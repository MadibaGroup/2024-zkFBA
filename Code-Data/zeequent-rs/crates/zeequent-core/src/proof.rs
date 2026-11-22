//! The two batch-close proofs, bound to the published book commitments.

use crate::book::ColumnSecret;
use crate::check::{check, check_receipt_scalars};
use crate::clearing::Witness;
use crate::columns::{build_columns, to_field, V_A, V_B, V_IO_A, V_IO_B};
use ark_bn254::Fr;
use crate::constraints::{ClearingSystem, NoCrossSystem, NC_A, NC_B};
use crate::engine::{prove_system, verify_system, FixedColumn, QUOTIENT_EXTRA};
pub use crate::engine::{Proof, ProveError, VerifyError};
use ark_std::rand::RngCore;
use zeequent_kzg::{Commitment, DomainKey, Srs, VerifierKey};
use zeequent_types::{Outcome, Params, Receipt};

/// The SRS degree a prover needs for these parameters.
pub fn required_srs_degree(params: &Params) -> usize {
    2 * params.n + QUOTIENT_EXTRA
}

#[derive(Clone, Copy, Debug, Default)]
pub struct ProverOptions {
    /// Skip the pre-flight check and the divisibility check and emit a proof anyway.
    pub skip_checks: bool,
}

fn fixed<'a>(column: usize, secret: &'a ColumnSecret) -> FixedColumn<'a> {
    FixedColumn {
        column,
        commitment: secret.commitment,
        blinding: &secret.blinding,
        mask: &secret.mask,
    }
}

/// Proves that `receipt` is the clearing of the committed book.
pub fn prove<R: RngCore>(
    params: &Params,
    receipt: &Receipt,
    witness: &Witness,
    book: &[ColumnSecret; 4],
    srs: &Srs,
    key: &DomainKey,
    rng: &mut R,
) -> Result<Proof, ProveError> {
    prove_with(params, receipt, witness, book, srs, key, rng, ProverOptions::default())
}

#[allow(clippy::too_many_arguments)]
pub fn prove_with<R: RngCore>(
    params: &Params,
    receipt: &Receipt,
    witness: &Witness,
    book: &[ColumnSecret; 4],
    srs: &Srs,
    key: &DomainKey,
    rng: &mut R,
    options: ProverOptions,
) -> Result<Proof, ProveError> {
    if !options.skip_checks {
        check(params, &Outcome::Cross(*receipt), witness).map_err(ProveError::Violation)?;
    }
    let system = ClearingSystem::new(params, receipt);
    let columns: Vec<Vec<Fr>> =
        build_columns(params, receipt, witness).iter().map(|c| to_field(c)).collect();
    let fixed_columns = [
        fixed(V_B, &book[0]),
        fixed(V_A, &book[1]),
        fixed(V_IO_B, &book[2]),
        fixed(V_IO_A, &book[3]),
    ];
    prove_system(&system, &columns, &fixed_columns, srs, key, rng, options.skip_checks)
}

/// Verifies a clearing proof against the receipt and the published book commitments.
pub fn verify(
    params: &Params,
    receipt: &Receipt,
    book: &[Commitment; 4],
    proof: &Proof,
    vk: &VerifierKey,
) -> Result<(), VerifyError> {
    check_receipt_scalars(params, receipt).map_err(|_| VerifyError::Receipt)?;
    let system = ClearingSystem::new(params, receipt);
    let expected = [(V_B, book[0]), (V_A, book[1]), (V_IO_B, book[2]), (V_IO_A, book[3])];
    check_book(proof, &expected)?;
    verify_system(&system, proof, vk)
}

fn check_book(proof: &Proof, expected: &[(usize, Commitment)]) -> Result<(), VerifyError> {
    for (column, commitment) in expected {
        match proof.commitments.get(*column) {
            Some(found) if found == commitment => {}
            Some(_) => return Err(VerifyError::Book),
            None => return Err(VerifyError::Shape),
        }
    }
    Ok(())
}

/// Proves that the committed book does not cross at any price.
pub fn prove_no_cross<R: RngCore>(
    params: &Params,
    witness: &Witness,
    book: &[ColumnSecret; 4],
    srs: &Srs,
    key: &DomainKey,
    rng: &mut R,
) -> Result<Proof, ProveError> {
    prove_no_cross_with(params, witness, book, srs, key, rng, ProverOptions::default())
}

pub fn prove_no_cross_with<R: RngCore>(
    params: &Params,
    witness: &Witness,
    book: &[ColumnSecret; 4],
    srs: &Srs,
    key: &DomainKey,
    rng: &mut R,
    options: ProverOptions,
) -> Result<Proof, ProveError> {
    if !options.skip_checks {
        check(params, &Outcome::NoCross, witness).map_err(ProveError::Violation)?;
    }
    let system = NoCrossSystem::new(params);
    let columns: Vec<Vec<Fr>> = system.columns(witness).iter().map(|c| to_field(c)).collect();
    let fixed_columns = [fixed(NC_B, &book[0]), fixed(NC_A, &book[1])];
    prove_system(&system, &columns, &fixed_columns, srs, key, rng, options.skip_checks)
}

/// Verifies zero ordinary executable volume against the final bid and ask commitments.
pub fn verify_no_cross(
    params: &Params,
    book: &[Commitment; 4],
    proof: &Proof,
    vk: &VerifierKey,
) -> Result<(), VerifyError> {
    let system = NoCrossSystem::new(params);
    check_book(proof, &[(NC_B, book[0]), (NC_A, book[1])])?;
    verify_system(&system, proof, vk)
}
