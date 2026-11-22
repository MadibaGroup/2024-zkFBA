//! The two constraint systems.

use crate::clearing::Witness;
use crate::columns::*;
use crate::engine::System;
use crate::lookup::{range_entries, Lookup};
use crate::public_polys::*;
use ark_bn254::Fr;
use ark_ff::{One, Zero};
use zeequent_kzg::Transcript;
use zeequent_types::{Params, Receipt};

fn absorb_params(t: &mut Transcript, params: &Params) {
    t.absorb_u64(b"n", params.n as u64);
    t.absorb_u64(b"n_ticks", params.n_ticks as u64);
    t.absorb_u64(b"k_in", params.k_in as u64);
    t.absorb_u64(b"k", params.k as u64);
}

/// `sum_j alpha^j * terms[j]`.
fn fold(terms: &[Fr], alpha: Fr) -> Fr {
    terms.iter().rev().fold(Fr::zero(), |acc, term| acc * alpha + term)
}

/// F1-F33 for a crossing batch.
pub(crate) struct ClearingSystem {
    params: Params,
    receipt: Receipt,
    layout: Layout,
    lookup: Lookup,
    v_max: Fr,
    /// `V_minDelta`
    v: Fr,
    /// Side bit `s`.
    s: Fr,
    /// `P`
    p_at_price: Fr,
    /// `J`
    j_io: Fr,
    /// `nu = [V_minDelta > 0]`
    nu: Fr,
}

impl ClearingSystem {
    pub(crate) fn new(params: &Params, receipt: &Receipt) -> Self {
        Self {
            params: *params,
            receipt: *receipt,
            layout: Layout::new(params),
            lookup: Layout::new(params).lookup(),
            v_max: Fr::from(receipt.v_max),
            v: Fr::from(receipt.v_min_delta),
            s: Fr::from(receipt.side.bit()),
            p_at_price: Fr::from(receipt.p_at_price),
            j_io: Fr::from(receipt.j_io),
            nu: Fr::from(receipt.has_imbalance() as u64),
        }
    }
}

impl System for ClearingSystem {
    fn protocol(&self) -> &'static [u8] {
        b"zeequent-clearing-v2"
    }

    fn absorb_statement(&self, t: &mut Transcript) {
        let r = &self.receipt;
        absorb_params(t, &self.params);
        t.absorb_u64(b"v_max", r.v_max);
        t.absorb_u64(b"c", r.c as u64);
        t.absorb_u64(b"d", r.d as u64);
        t.absorb_u64(b"p_star", r.p_star as u64);
        t.absorb_u64(b"v_min_delta", r.v_min_delta);
        t.absorb_u64(b"side", r.side.bit());
        t.absorb_u64(b"p_at_price", r.p_at_price);
        t.absorb_u64(b"j_io", r.j_io);
    }

    fn n(&self) -> usize {
        self.params.n
    }

    fn phase1_columns(&self) -> usize {
        self.lookup.phase1()
    }

    fn num_columns(&self) -> usize {
        self.lookup.total()
    }

    fn shifted(&self) -> Vec<usize> {
        let mut shifted = SHIFTED.to_vec();
        shifted.push(self.lookup.running_sum());
        shifted
    }

    fn phase2(&self, beta: Fr, phase1: &[Vec<Fr>]) -> Vec<Vec<Fr>> {
        self.lookup.phase2(beta, phase1)
    }

    fn public_polys(&self) -> Vec<PublicPoly> {
        let mut polys = clearing_sets(&self.params, &self.receipt);
        polys.push(self.lookup.table());
        polys
    }

    fn combine(
        &self,
        cols: &[Fr],
        shifted: &[Fr],
        p: &[Fr],
        gate: Fr,
        beta: Fr,
        alpha: Fr,
    ) -> Fr {
        let one = Fr::one();
        let layout = &self.layout;
        let (v_max, v, s) = (self.v_max, self.v, self.s);
        // The quantity rebuilt from its limbs.
        let bits = |group: usize| {
            let start = layout.group_start(group);
            self.lookup.recompose(&cols[start..start + layout.limbs(group)])
        };

        let mut terms = vec![
            // Inputs.
            cols[V_B] - bits(G_B),                                             // F1
            cols[V_A] - bits(G_A),                                             // F2
            p[P_PAD] * cols[V_B],                                              // F3
            p[P_PAD] * cols[V_A],                                              // F4
            // Depth.
            p[P_L_LAST] * (cols[V_ACC_B] - cols[V_B]),                         // F5
            p[P_L_FIRST] * (cols[V_ACC_A] - cols[V_A]),                        // F6
            gate * (cols[V_ACC_B] - shifted[S_ACC_B] - cols[V_B]),             // F7
            gate * (shifted[S_ACC_A] - cols[V_ACC_A] - shifted[S_A]),          // F8
            // Executable volume.
            cols[V_SURP_B] - (cols[V_ACC_B] - cols[V_MIN]),                    // F9
            cols[V_SURP_A] - (cols[V_ACC_A] - cols[V_MIN]),                    // F10
            cols[V_SURP_A] * cols[V_SURP_B],                                   // F11
            cols[V_SURP_B] - bits(G_SURP_B),                                   // F12
            cols[V_SURP_A] - bits(G_SURP_A),                                   // F13
            (v_max - cols[V_MIN]) - bits(G_CEIL),                              // F14
            // Plateau.
            p[P_L_C] * (cols[V_MIN] - v_max),                                  // F15
            p[P_L_D] * (cols[V_MIN] - v_max),                                  // F16
            p[P_MASK_C] * (v_max - cols[V_MIN] - one - cols[V_SLACK]),         // F17
            // Clearing tick.
            cols[V_DELTA] - cols[V_SURP_A] - cols[V_SURP_B],                   // F18
            p[P_MASK_P] * ((cols[V_DELTA] - v) - bits(G_FLR)),                 // F19
            p[P_L_P] * (cols[V_SURP_B] - (one - s) * v),                       // F20a
            p[P_L_P] * (cols[V_SURP_A] - s * v),                               // F20b
            p[P_MASK_T] * (cols[V_DELTA] - v - one - cols[V_SLACK]),           // F21
            cols[V_SLACK] - bits(G_SLK),                                       // F22
            // Phase 3.
            cols[V_IO_B] - bits(G_IO_B),                                       // F24
            cols[V_IO_A] - bits(G_IO_A),                                       // F25
            p[P_PAD] * cols[V_IO_B],                                           // F26
            p[P_PAD] * cols[V_IO_A],                                           // F27
            p[P_L_LAST] * (cols[V_IO_ACC_B] - cols[V_IO_B]),                   // F28
            gate * (cols[V_IO_ACC_B] - shifted[S_IO_ACC_B] - cols[V_IO_B]),    // F29
            p[P_L_FIRST] * (cols[V_IO_ACC_A] - cols[V_IO_A]),                  // F30
            gate * (shifted[S_IO_ACC_A] - cols[V_IO_ACC_A] - shifted[S_IO_A]), // F31
            self.nu * p[P_L_P]
                * ((one - s) * cols[V_B] + s * cols[V_A] - self.p_at_price),   // F32
            self.nu * p[P_L_P]
                * ((one - s) * cols[V_IO_ACC_A] + s * cols[V_IO_ACC_B] - self.j_io), // F33
        ];
        // F23: every limb is in the table.
        self.lookup.terms(cols, shifted[S_LOOKUP_SUM], p[P_TABLE], beta, &mut terms);
        fold(&terms, alpha)
    }
}

// Columns of the no-cross proof.
pub(crate) const NC_B: usize = 0;
pub(crate) const NC_A: usize = 1;
const NC_ACC_B: usize = 2;
const NC_ACC_A: usize = 3;
const NC_VALUE: usize = 4;
/// `AccB`, `AccA`, `A` at `omega * zeta`; the lookup's running sum follows.
const NC_SHIFTED: [usize; 3] = [NC_ACC_B, NC_ACC_A, NC_A];
/// Position of the lookup table among the public polynomials.
const NC_P_TABLE: usize = 3;

/// The no-cross statement.
pub(crate) struct NoCrossSystem {
    params: Params,
    /// Limbs of a single-tick volume.
    limbs: usize,
    lookup: Lookup,
}

impl NoCrossSystem {
    pub(crate) fn new(params: &Params) -> Self {
        let limb_bits = Lookup::limb_bits_for(params.n);
        let bits = params.k_in as usize;
        let limbs = bits.div_ceil(limb_bits);
        let mut entries = range_entries(limb_bits, NC_VALUE, bits);
        entries.extend(range_entries(limb_bits, NC_VALUE + limbs, bits));
        Self {
            params: *params,
            limbs,
            lookup: Lookup {
                limb_bits,
                entries,
                mult: NC_VALUE + 2 * limbs,
            },
        }
    }

    /// The first-round columns: `B`, `A`, `AccB`, `AccA`, the limbs of `B` and of `A`, and the lookup multiplicities.
    pub(crate) fn columns(&self, w: &Witness) -> Vec<Vec<u64>> {
        let mut columns = vec![w.b.clone(), w.a.clone(), w.acc_b.clone(), w.acc_a.clone()];
        for source in [&w.b, &w.a] {
            columns.extend(self.lookup.limbs(source, self.limbs));
        }
        let multiplicities = self.lookup.multiplicities(&columns, self.params.n);
        columns.push(multiplicities);
        columns
    }
}

impl System for NoCrossSystem {
    fn protocol(&self) -> &'static [u8] {
        b"zeequent-no-cross-v2"
    }

    fn absorb_statement(&self, t: &mut Transcript) {
        absorb_params(t, &self.params);
    }

    fn n(&self) -> usize {
        self.params.n
    }

    fn phase1_columns(&self) -> usize {
        self.lookup.phase1()
    }

    fn num_columns(&self) -> usize {
        self.lookup.total()
    }

    fn shifted(&self) -> Vec<usize> {
        let mut shifted = NC_SHIFTED.to_vec();
        shifted.push(self.lookup.running_sum());
        shifted
    }

    fn phase2(&self, beta: Fr, phase1: &[Vec<Fr>]) -> Vec<Vec<Fr>> {
        self.lookup.phase2(beta, phase1)
    }

    fn public_polys(&self) -> Vec<PublicPoly> {
        let mut polys = no_cross_sets(&self.params);
        polys.push(self.lookup.table());
        polys
    }

    fn combine(
        &self,
        cols: &[Fr],
        shifted: &[Fr],
        p: &[Fr],
        gate: Fr,
        beta: Fr,
        alpha: Fr,
    ) -> Fr {
        let width = self.limbs;
        let (l_first, l_last, pad) = (p[0], p[1], p[2]);
        let (acc_b_next, acc_a_next, a_next) = (shifted[0], shifted[1], shifted[2]);
        let recompose = |limbs: &[Fr]| self.lookup.recompose(limbs);
        let mut terms = vec![
            cols[NC_B] - recompose(&cols[NC_VALUE..NC_VALUE + width]),
            cols[NC_A] - recompose(&cols[NC_VALUE + width..NC_VALUE + 2 * width]),
            pad * cols[NC_B],
            pad * cols[NC_A],
            l_last * (cols[NC_ACC_B] - cols[NC_B]),
            l_first * (cols[NC_ACC_A] - cols[NC_A]),
            gate * (cols[NC_ACC_B] - acc_b_next - cols[NC_B]),
            gate * (acc_a_next - cols[NC_ACC_A] - a_next),
            cols[NC_ACC_A] * cols[NC_ACC_B],
        ];
        self.lookup.terms(cols, shifted[3], p[NC_P_TABLE], beta, &mut terms);
        fold(&terms, alpha)
    }
}
