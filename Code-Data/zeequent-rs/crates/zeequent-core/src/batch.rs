//! Checks a batch against the supplied opening and history. The caller compares the returned digest with the published head.

use crate::book::{verify_book_opening, Board, BookOpening};
use crate::engine::{Proof, VerifyError};
use crate::proof::{verify, verify_no_cross};
use crate::updates::{verify_updates, UpdateProof};
use ark_bn254::Fr;
use zeequent_kzg::{Commitment, DomainKey, VerifierKey};
use zeequent_types::{Outcome, Params};

/// Artifacts used to verify one batch against a local copy of the log.
#[derive(Clone, Debug)]
pub struct PublishedBatch<'a> {
    pub opening: &'a BookOpening,
    pub board: &'a Board,
    pub outcome: &'a Outcome,
    /// The clearing proof, or the no-cross proof.
    pub proof: &'a Proof,
    pub updates: &'a UpdateProof,
}

/// Which check failed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BatchError {
    /// A book commitment was not opened empty.
    Opening,
    /// The log's digests do not chain from the opening.
    Log,
    /// The update-validity proof is invalid.
    Updates(VerifyError),
    /// The clearing or no-cross proof is invalid.
    Outcome(VerifyError),
}

/// What a successful check establishes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VerifiedBatch {
    /// Final log heads. A crossing proof uses all four; a no-cross proof uses the ordinary bid and ask heads.
    pub book: [Commitment; 4],
    /// The digest that pins the whole log.
    pub log_digest: Fr,
    /// Number of verified log steps. An honest order submission writes four.
    pub steps: usize,
}

/// Verifies a published batch.
pub fn verify_batch(
    params: &Params,
    update_size: usize,
    batch: &PublishedBatch,
    vk: &VerifierKey,
    key: &DomainKey,
) -> Result<VerifiedBatch, BatchError> {
    if !verify_book_opening(batch.opening, vk, key) {
        return Err(BatchError::Opening);
    }
    let book = batch.board.verify(batch.opening).ok_or(BatchError::Log)?;
    let deltas = batch.board.deltas(batch.opening);
    verify_updates(params, &deltas, update_size, batch.updates, vk, key)
        .map_err(BatchError::Updates)?;
    match batch.outcome {
        Outcome::Cross(receipt) => verify(params, receipt, &book, batch.proof, vk),
        Outcome::NoCross => verify_no_cross(params, &book, batch.proof, vk),
    }
    .map_err(BatchError::Outcome)?;
    Ok(VerifiedBatch {
        book,
        log_digest: batch.board.head_digest(batch.opening),
        steps: batch.board.entries.len(),
    })
}
