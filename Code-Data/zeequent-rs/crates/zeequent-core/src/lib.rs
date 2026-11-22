//! Zeequent core.

pub mod batch;
pub mod book;
pub mod check;
pub mod clearing;
pub mod columns;
mod constraints;
mod engine;
pub mod lookup;
pub mod proof;
pub mod public_polys;
pub mod updates;

pub use book::{
    commitments_from_bytes, commitments_to_bytes, verify_book_opening, Board, BoardEntry, verify_empty_column, verify_update, BookOpening, BookState,
    BookStateError, ColumnSecret, EmptyColumnProof, UpdateReceipt, UpdateStep,
};
pub use batch::{verify_batch, BatchError, PublishedBatch, VerifiedBatch};
pub use check::{check, check_receipt_scalars, Violation};
pub use updates::{
    prove_updates, prove_updates_with, required_srs_degree_updates, update_domain_size,
    verify_updates, UpdateProof,
};
pub use clearing::{clear, fill, ClearError, Witness};
pub use proof::{
    prove, prove_no_cross, prove_no_cross_with, prove_with, required_srs_degree, verify,
    verify_no_cross, Proof, ProveError, ProverOptions, VerifyError,
};
