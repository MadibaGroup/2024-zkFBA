//! Column layout of the clearing proof.

use crate::clearing::Witness;
use crate::lookup::{range_entries, Lookup};
use ark_bn254::Fr;
use zeequent_types::{Params, Receipt};

// Value columns, in `Witness` field order.
pub const V_B: usize = 0;
pub const V_A: usize = 1;
pub const V_ACC_B: usize = 2;
pub const V_ACC_A: usize = 3;
pub const V_MIN: usize = 4;
pub const V_SURP_B: usize = 5;
pub const V_SURP_A: usize = 6;
pub const V_DELTA: usize = 7;
pub const V_SLACK: usize = 8;
pub const V_IO_B: usize = 9;
pub const V_IO_A: usize = 10;
pub const V_IO_ACC_B: usize = 11;
pub const V_IO_ACC_A: usize = 12;
pub const NUM_VALUE: usize = 13;

// Range-checked quantities.
pub const G_B: usize = 0;
pub const G_A: usize = 1;
pub const G_IO_B: usize = 2;
pub const G_IO_A: usize = 3;
pub const G_SURP_B: usize = 4;
pub const G_SURP_A: usize = 5;
/// `V_max - Min` (F14).
pub const G_CEIL: usize = 6;
/// `Delta - V_minDelta` on the plateau, 0 elsewhere (F19).
pub const G_FLR: usize = 7;
/// `Slack` (F22).
pub const G_SLK: usize = 8;
pub const NUM_GROUPS: usize = 9;

/// Value columns also opened at `omega * zeta`.
pub const SHIFTED: [usize; 6] = [V_ACC_B, V_ACC_A, V_A, V_IO_ACC_B, V_IO_ACC_A, V_IO_A];
// Positions inside the shifted opening.
pub const S_ACC_B: usize = 0;
pub const S_ACC_A: usize = 1;
pub const S_A: usize = 2;
pub const S_IO_ACC_B: usize = 3;
pub const S_IO_ACC_A: usize = 4;
pub const S_IO_A: usize = 5;
pub const S_LOOKUP_SUM: usize = 6;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Layout {
    /// `w`: bits per limb.
    pub limb_bits: usize,
    /// Limbs of a single-tick volume, `ceil(k_in / w)`.
    pub limbs_in: usize,
    /// Limbs of an accumulated quantity, `ceil(k / w)`.
    pub limbs_acc: usize,
    /// `k_in`
    pub bits_in: usize,
    /// `k`
    pub bits_acc: usize,
}

impl Layout {
    pub fn new(params: &Params) -> Self {
        let limb_bits = Lookup::limb_bits_for(params.n);
        Self {
            limb_bits,
            limbs_in: (params.k_in as usize).div_ceil(limb_bits),
            limbs_acc: (params.k as usize).div_ceil(limb_bits),
            bits_in: params.k_in as usize,
            bits_acc: params.k as usize,
        }
    }

    /// Number of limbs of a range-checked quantity.
    pub fn limbs(&self, group: usize) -> usize {
        if group < 4 {
            self.limbs_in
        } else {
            self.limbs_acc
        }
    }

    /// Index of the first limb column of a quantity.
    pub fn group_start(&self, group: usize) -> usize {
        NUM_VALUE + (0..group).map(|g| self.limbs(g)).sum::<usize>()
    }

    /// Bit length of a range-checked quantity.
    pub fn bits(&self, group: usize) -> usize {
        if group < 4 {
            self.bits_in
        } else {
            self.bits_acc
        }
    }

    pub fn lookup(&self) -> Lookup {
        Lookup {
            limb_bits: self.limb_bits,
            entries: (0..NUM_GROUPS)
                .flat_map(|g| range_entries(self.limb_bits, self.group_start(g), self.bits(g)))
                .collect(),
            mult: self.group_start(NUM_GROUPS),
        }
    }

    /// Number of first-round columns.
    pub fn phase1(&self) -> usize {
        self.lookup().phase1()
    }

    /// Total number of committed columns, excluding the quotient.
    pub fn total(&self) -> usize {
        self.lookup().total()
    }
}

/// The first-round columns in layout order: values, limbs, multiplicities.
pub fn build_columns(params: &Params, receipt: &Receipt, w: &Witness) -> Vec<Vec<u64>> {
    let layout = Layout::new(params);
    let lookup = layout.lookup();
    let ceil: Vec<u64> = w.min.iter().map(|m| receipt.v_max.wrapping_sub(*m)).collect();
    let flr: Vec<u64> = w
        .delta
        .iter()
        .enumerate()
        .map(|(i, delta)| {
            if (receipt.c..=receipt.d).contains(&i) {
                delta.wrapping_sub(receipt.v_min_delta)
            } else {
                0
            }
        })
        .collect();

    let mut columns: Vec<Vec<u64>> = vec![
        w.b.clone(),
        w.a.clone(),
        w.acc_b.clone(),
        w.acc_a.clone(),
        w.min.clone(),
        w.surp_b.clone(),
        w.surp_a.clone(),
        w.delta.clone(),
        w.slack.clone(),
        w.io_b.clone(),
        w.io_a.clone(),
        w.io_acc_b.clone(),
        w.io_acc_a.clone(),
    ];
    let sources: [&Vec<u64>; NUM_GROUPS] = [
        &w.b, &w.a, &w.io_b, &w.io_a, &w.surp_b, &w.surp_a, &ceil, &flr, &w.slack,
    ];
    for (group, source) in sources.iter().enumerate() {
        columns.extend(lookup.limbs(source, layout.limbs(group)));
    }
    let multiplicities = lookup.multiplicities(&columns, params.n);
    columns.push(multiplicities);
    debug_assert_eq!(columns.len(), layout.phase1());
    columns
}

pub fn to_field(values: &[u64]) -> Vec<Fr> {
    values.iter().map(|v| Fr::from(*v)).collect()
}
