//! Public polynomials.

use ark_bn254::Fr;
use ark_ff::{batch_inversion, Field, One, Zero};
use ark_poly::{EvaluationDomain, Radix2EvaluationDomain};
use zeequent_types::{Params, Receipt, Side};

/// A public polynomial: its non-zero values on the domain.
pub type PublicPoly = Vec<(usize, Fr)>;

// Public polynomials of the clearing proof, in order.
pub const P_L_FIRST: usize = 0;
pub const P_L_LAST: usize = 1;
pub const P_PAD: usize = 2;
pub const P_MASK_P: usize = 3;
pub const P_MASK_C: usize = 4;
pub const P_MASK_T: usize = 5;
pub const P_L_C: usize = 6;
pub const P_L_D: usize = 7;
pub const P_L_P: usize = 8;
pub const P_TABLE: usize = 9;

/// The polynomial that is 1 on the given ticks and 0 on the rest.
pub fn indicator(ticks: impl IntoIterator<Item = usize>) -> PublicPoly {
    ticks.into_iter().map(|tick| (tick, Fr::one())).collect()
}

/// The nine indicator polynomials of the clearing proof.
pub fn clearing_sets(params: &Params, r: &Receipt) -> Vec<PublicPoly> {
    let n = params.n;
    let mut cliffs = Vec::new();
    if r.c >= 1 {
        cliffs.push(r.c - 1);
    }
    if r.d + 1 < n {
        cliffs.push(r.d + 1);
    }
    let tie_break_active =
        r.p_star > r.c && (r.side == Side::SupplyLong || r.v_min_delta == 0);
    let tie = if tie_break_active {
        vec![r.p_star - 1]
    } else {
        vec![]
    };
    vec![
        indicator([0]),
        indicator([n - 1]),
        indicator(params.n_ticks..n),
        indicator(r.c..=r.d),
        indicator(cliffs),
        indicator(tie),
        indicator([r.c]),
        indicator([r.d]),
        indicator([r.p_star]),
    ]
}

/// The indicator polynomials of the no-cross proof: `L_0`, `L_{n-1}`, `Pad`.
pub fn no_cross_sets(params: &Params) -> Vec<PublicPoly> {
    vec![
        indicator([0]),
        indicator([params.n - 1]),
        indicator(params.n_ticks..params.n),
    ]
}

/// Evaluation vectors over the domain, for the prover.
pub fn on_domain(polys: &[PublicPoly], n: usize) -> Vec<Vec<Fr>> {
    polys
        .iter()
        .map(|poly| {
            let mut evals = vec![Fr::zero(); n];
            for (tick, value) in poly {
                evals[*tick] = *value;
            }
            evals
        })
        .collect()
}

/// `sum_i value_i * L_i(x)` with `L_i(x) = omega^i (x^n - 1) / (n (x - omega^i))`.
fn evaluate(domain: &Radix2EvaluationDomain<Fr>, x: Fr, poly: &PublicPoly) -> Fr {
    if poly.is_empty() {
        return Fr::zero();
    }
    let roots: Vec<Fr> = poly.iter().map(|(tick, _)| domain.element(*tick)).collect();
    let mut denominators: Vec<Fr> = roots.iter().map(|root| x - root).collect();
    batch_inversion(&mut denominators);
    let sum: Fr = poly
        .iter()
        .zip(roots.iter().zip(&denominators))
        .map(|((_, value), (root, inverse))| *value * root * inverse)
        .sum();
    let n = Fr::from(domain.size() as u64);
    sum * (x.pow([domain.size() as u64]) - Fr::one()) * n.inverse().expect("n is non-zero")
}

/// Evaluates every public polynomial at `x`, for the verifier.
pub fn eval_at(domain: &Radix2EvaluationDomain<Fr>, polys: &[PublicPoly], x: Fr) -> Vec<Fr> {
    polys.iter().map(|poly| evaluate(domain, x, poly)).collect()
}
