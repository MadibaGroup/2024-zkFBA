//! The committed order book: opening, updates, receipts and the commitment log.

use ark_bn254::{Bn254, Fr, G1Affine, G1Projective};
use ark_ec::{pairing::Pairing, AffineRepr, CurveGroup, VariableBaseMSM};
use ark_ff::UniformRand;
use ark_poly::DenseUVPolynomial;
use ark_serialize::CanonicalSerialize;
use ark_std::rand::{rngs::StdRng, RngCore, SeedableRng};
use zeequent_kzg::{
    commit_blinding, commit_mask, commit_with, Blinding, Commitment, DomainKey, KzgError, Poly,
    Srs, Transcript, VerifierKey, BOOK_BLINDING_DEGREE,
};
use zeequent_types::{Book, BookError, Column, Order, Outcome, Params};

use crate::clearing::{clear, ClearError, Witness};

/// Degree of `rho_0` in a book column's off-domain mask.
pub const BOOK_MASK_DEGREE: usize = 3;

/// The exchange's private data for one committed book column.
#[derive(Clone, Debug)]
pub struct ColumnSecret {
    pub commitment: Commitment,
    /// `r`: `r_0` plus the sum of every update's `rho` in the constant term.
    pub blinding: Blinding,
    /// Coefficients of `rho_0`.
    pub mask: Vec<Fr>,
}

/// Public proof that a freshly opened column commitment is empty.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EmptyColumnProof {
    /// `D = [rho_0(tau)]_1`
    pub d: G1Affine,
    /// `E = [gamma * r_0(tau)]_1`
    pub e: G1Affine,
    /// Schnorr commitment for the knowledge of `r_0`.
    pub schnorr_commitment: G1Affine,
    /// Schnorr responses, one per coefficient of `r_0`.
    pub schnorr_response: Vec<Fr>,
}

/// What the exchange publishes when a batch opens.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BookOpening {
    pub commitments: [Commitment; 4],
    pub proofs: [EmptyColumnProof; 4],
}

/// Given to the trader whose order caused an update.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UpdateReceipt {
    pub column: Column,
    pub tick: usize,
    pub qty: u64,
    /// Blinding of this update.
    pub rho: Fr,
    /// The column's commitment before and after the update.
    pub before: Commitment,
    pub after: Commitment,
}

/// The exchange's private record of one update.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UpdateStep {
    pub column: Column,
    pub tick: usize,
    pub qty: u64,
    pub rho: Fr,
    /// The public change of the column's commitment, `after - before`.
    pub delta: G1Affine,
}

fn schnorr_challenge(commitment: &Commitment, d: &G1Affine, e: &G1Affine, a: &G1Affine) -> Fr {
    let mut t = Transcript::new(b"zeequent-empty-book-v1");
    t.absorb_g1(b"commitment", &commitment.0);
    t.absorb_g1(b"d", d);
    t.absorb_g1(b"e", e);
    t.absorb_g1(b"schnorr", a);
    t.challenge(b"challenge")
}

pub(crate) fn open_column<R: RngCore>(
    n: usize,
    srs: &Srs,
    rng: &mut R,
) -> Result<(ColumnSecret, EmptyColumnProof), KzgError> {
    let mask: Vec<Fr> = (0..=BOOK_MASK_DEGREE).map(|_| Fr::rand(rng)).collect();
    let blinding = Blinding::random(BOOK_BLINDING_DEGREE, rng);

    let d = commit_with(srs, &Poly::from_coefficients_vec(mask.clone()), &Blinding::none())?.0;
    let e_point = commit_blinding(srs, &blinding)?;
    let e = e_point.into_affine();
    let commitment = Commitment((commit_mask(srs, n, &mask)? + e_point).into_affine());

    // Schnorr proof of knowledge of r_0 with E = sum r_0[i] * [gamma tau^i].
    let nonce = Blinding::random(BOOK_BLINDING_DEGREE, rng);
    let schnorr_commitment = commit_blinding(srs, &nonce)?.into_affine();
    let challenge = schnorr_challenge(&commitment, &d, &e, &schnorr_commitment);
    let schnorr_response = nonce
        .0
        .coeffs
        .iter()
        .zip(&blinding.0.coeffs)
        .map(|(k, r)| *k + challenge * r)
        .collect();

    Ok((
        ColumnSecret {
            commitment,
            blinding,
            mask,
        },
        EmptyColumnProof {
            d,
            e,
            schnorr_commitment,
            schnorr_response,
        },
    ))
}

/// Checks that a column commitment published at batch open is empty.
pub fn verify_empty_column(
    commitment: &Commitment,
    proof: &EmptyColumnProof,
    vk: &VerifierKey,
    key: &DomainKey,
) -> bool {
    if proof.schnorr_response.len() != key.blinding_basis.len() {
        return false;
    }
    // E is a pure blinding term.
    let challenge = schnorr_challenge(commitment, &proof.d, &proof.e, &proof.schnorr_commitment);
    let lhs = G1Projective::msm_unchecked(&key.blinding_basis, &proof.schnorr_response);
    if lhs != proof.e * challenge + proof.schnorr_commitment {
        return false;
    }
    // What remains is a multiple of Z_H, so it is zero on the whole domain.
    let remainder = (commitment.0 - proof.e).into_affine();
    Bn254::pairing(remainder, vk.h) == Bn254::pairing(proof.d, key.vanishing_h)
}

/// Checks all four columns of a batch opening.
pub fn verify_book_opening(opening: &BookOpening, vk: &VerifierKey, key: &DomainKey) -> bool {
    opening
        .commitments
        .iter()
        .zip(&opening.proofs)
        .all(|(commitment, proof)| verify_empty_column(commitment, proof, vk, key))
}

/// Checks the receipt equation. The caller compares the encoded order fields and checks log membership separately.
pub fn verify_update(receipt: &UpdateReceipt, vk: &VerifierKey, key: &DomainKey) -> bool {
    if receipt.tick >= key.lagrange_g.len() {
        return false;
    }
    let expected = key.lagrange_g[receipt.tick] * Fr::from(receipt.qty)
        + vk.gamma_g * receipt.rho
        + receipt.before.0;
    expected.into_affine() == receipt.after.0
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BookStateError {
    Book(BookError),
    Kzg(KzgError),
    /// The tick's volume would no longer fit in `k_in` bits.
    VolumeTooLarge,
    /// The domain key is for another domain size.
    Domain,
}

/// The exchange's state for one batch: the plaintext book and the secrets behind its four running commitments.
#[derive(Clone, Debug)]
pub struct BookState {
    pub params: Params,
    pub book: Book,
    /// Indexed by `Column::index()`.
    pub columns: [ColumnSecret; 4],
    /// Every update of the batch, in log order.
    pub history: Vec<UpdateStep>,
}

impl BookState {
    /// Opens an empty book and returns the public opening.
    pub fn open<R: RngCore>(
        params: &Params,
        srs: &Srs,
        key: &DomainKey,
        rng: &mut R,
    ) -> Result<(Self, BookOpening), BookStateError> {
        if key.n != params.n {
            return Err(BookStateError::Domain);
        }
        let mut secrets = Vec::with_capacity(4);
        let mut proofs = Vec::with_capacity(4);
        for _ in 0..4 {
            let (secret, proof) = open_column(params.n, srs, rng).map_err(BookStateError::Kzg)?;
            secrets.push(secret);
            proofs.push(proof);
        }
        let columns: [ColumnSecret; 4] = secrets.try_into().expect("four columns");
        let opening = BookOpening {
            commitments: core::array::from_fn(|i| columns[i].commitment),
            proofs: proofs.try_into().expect("four proofs"),
        };
        let state = Self {
            params: *params,
            book: Book::new(params.n_ticks),
            columns,
            history: Vec::new(),
        };
        Ok((state, opening))
    }

    /// The four public running commitments.
    pub fn commitments(&self) -> [Commitment; 4] {
        core::array::from_fn(|i| self.columns[i].commitment)
    }

    /// Records an order and returns the trader's receipt.
    pub fn apply<R: RngCore>(
        &mut self,
        order: &Order,
        vk: &VerifierKey,
        key: &DomainKey,
        rng: &mut R,
    ) -> Result<UpdateReceipt, BookStateError> {
        let (column, tick) = self.book.slot(order).map_err(BookStateError::Book)?;
        let current = self.book.column(column)[tick];
        match current.checked_add(order.qty) {
            Some(total) if total < self.params.input_bound() => {}
            _ => return Err(BookStateError::VolumeTooLarge),
        }
        self.book.add(order).map_err(BookStateError::Book)?;

        // Every order moves all four commitments; only its own column gets the quantity.
        let mut receipt = None;
        for index in 0..4 {
            let rho = Fr::rand(rng);
            let real = index == column.index();
            let secret = &mut self.columns[index];
            let before = secret.commitment;
            let mut point = vk.gamma_g * rho + before.0;
            if real {
                point += key.lagrange_g[tick] * Fr::from(order.qty);
            }
            let after = Commitment(point.into_affine());
            secret.commitment = after;
            secret.blinding.0.coeffs[0] += rho;
            self.history.push(UpdateStep {
                column: Column::from_index(index).expect("four columns"),
                tick: if real { tick } else { 0 },
                qty: if real { order.qty } else { 0 },
                rho,
                delta: (after.0.into_group() - before.0).into_affine(),
            });
            if real {
                receipt = Some(UpdateReceipt {
                    column,
                    tick,
                    qty: order.qty,
                    rho,
                    before,
                    after,
                });
            }
        }
        Ok(receipt.expect("the order's column is one of the four"))
    }
    /// Prover randomness derived from this exact state.
    pub fn prover_rng(&self, purpose: &[u8]) -> StdRng {
        let mut t = Transcript::new(b"zeequent-prover-randomness-v1");
        t.absorb_u64(b"n_ticks", self.params.n_ticks as u64);
        t.absorb_u64(b"k_in", self.params.k_in as u64);
        t.absorb_bytes(b"state", &self.to_bytes());
        t.absorb_bytes(b"purpose", purpose);
        let mut seed = [0u8; 32];
        t.challenge(b"seed")
            .serialize_compressed(&mut seed[..])
            .expect("a field element is 32 bytes");
        StdRng::from_seed(seed)
    }

    /// Runs the clearing rule on the current book.
    pub fn clear(&self) -> Result<(Outcome, Witness), ClearError> {
        clear(&self.book, &self.params)
    }
}

// File formats.

use zeequent_kzg::codec::{Reader, Writer};

const STATE_MAGIC: &[u8; 8] = b"ZQSTATE2";
const OPENING_MAGIC: &[u8; 8] = b"ZQOPEN01";
const BOOK_MAGIC: &[u8; 8] = b"ZQBOOK01";
const UPDATE_MAGIC: &[u8; 8] = b"ZQUPDATE";

/// The four public book commitments.
pub fn commitments_to_bytes(commitments: &[Commitment; 4]) -> Vec<u8> {
    let mut w = Writer::new(BOOK_MAGIC);
    for commitment in commitments {
        w.g1(&commitment.0);
    }
    w.finish()
}

pub fn commitments_from_bytes(bytes: &[u8]) -> Option<[Commitment; 4]> {
    let mut r = Reader::new(bytes, BOOK_MAGIC)?;
    let commitments = [
        Commitment(r.g1()?),
        Commitment(r.g1()?),
        Commitment(r.g1()?),
        Commitment(r.g1()?),
    ];
    r.finish()?;
    Some(commitments)
}

impl BookOpening {
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut w = Writer::new(OPENING_MAGIC);
        for (commitment, proof) in self.commitments.iter().zip(&self.proofs) {
            w.g1(&commitment.0);
            w.g1(&proof.d);
            w.g1(&proof.e);
            w.g1(&proof.schnorr_commitment);
            w.frs(&proof.schnorr_response);
        }
        w.finish()
    }

    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        let mut r = Reader::new(bytes, OPENING_MAGIC)?;
        let mut commitments = Vec::with_capacity(4);
        let mut proofs = Vec::with_capacity(4);
        for _ in 0..4 {
            commitments.push(Commitment(r.g1()?));
            proofs.push(EmptyColumnProof {
                d: r.g1()?,
                e: r.g1()?,
                schnorr_commitment: r.g1()?,
                schnorr_response: r.frs()?,
            });
        }
        r.finish()?;
        Some(Self {
            commitments: commitments.try_into().ok()?,
            proofs: proofs.try_into().ok()?,
        })
    }
}

impl UpdateReceipt {
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut w = Writer::new(UPDATE_MAGIC);
        w.u64(self.column.index() as u64);
        w.u64(self.tick as u64);
        w.u64(self.qty);
        w.fr(&self.rho);
        w.g1(&self.before.0);
        w.g1(&self.after.0);
        w.finish()
    }

    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        let mut r = Reader::new(bytes, UPDATE_MAGIC)?;
        let receipt = Self {
            column: Column::from_index(usize::try_from(r.u64()?).ok()?)?,
            tick: usize::try_from(r.u64()?).ok()?,
            qty: r.u64()?,
            rho: r.fr()?,
            before: Commitment(r.g1()?),
            after: Commitment(r.g1()?),
        };
        r.finish()?;
        Some(receipt)
    }
}

impl BookState {
    /// The exchange's private state.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut w = Writer::new(STATE_MAGIC);
        for column in [&self.book.bids, &self.book.asks, &self.book.io_bids, &self.book.io_asks] {
            w.u64s(column);
        }
        for secret in &self.columns {
            w.g1(&secret.commitment.0);
            w.frs(&secret.blinding.0.coeffs);
            w.frs(&secret.mask);
        }
        w.u64(self.history.len() as u64);
        for step in &self.history {
            w.u64(step.column.index() as u64);
            w.u64(step.tick as u64);
            w.u64(step.qty);
            w.fr(&step.rho);
            w.g1(&step.delta);
        }
        w.finish()
    }

    pub fn from_bytes(params: &Params, bytes: &[u8]) -> Option<Self> {
        let mut r = Reader::new(bytes, STATE_MAGIC)?;
        let book = Book {
            bids: r.u64s()?,
            asks: r.u64s()?,
            io_bids: r.u64s()?,
            io_asks: r.u64s()?,
        };
        let lengths = [&book.bids, &book.asks, &book.io_bids, &book.io_asks];
        if lengths.iter().any(|column| column.len() != params.n_ticks) {
            return None;
        }
        let mut columns = Vec::with_capacity(4);
        for _ in 0..4 {
            columns.push(ColumnSecret {
                commitment: Commitment(r.g1()?),
                blinding: Blinding(Poly::from_coefficients_vec(r.frs()?)),
                mask: r.frs()?,
            });
        }
        let count = usize::try_from(r.u64()?).ok()?;
        // Reject counts the input cannot hold.
        if count.checked_mul(120)? > bytes.len() {
            return None;
        }
        let mut history = Vec::with_capacity(count);
        for _ in 0..count {
            history.push(UpdateStep {
                column: Column::from_index(usize::try_from(r.u64()?).ok()?)?,
                tick: usize::try_from(r.u64()?).ok()?,
                qty: r.u64()?,
                rho: r.fr()?,
                delta: r.g1()?,
            });
        }
        r.finish()?;
        Some(Self {
            params: *params,
            book,
            columns: columns.try_into().ok()?,
            history,
        })
    }
}

// The commitment log ("bulletin board").

const BOARD_MAGIC: &[u8; 8] = b"ZQBOARD1";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BoardEntry {
    pub column: Column,
    /// The column's commitment after this update.
    pub commitment: Commitment,
    /// Digest of the opening and of every entry up to and including this one.
    pub digest: Fr,
}

/// Hash-chained commitment history for one Zeequent batch. Entries have no order-arrival timestamp.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Board {
    pub entries: Vec<BoardEntry>,
}

fn opening_digest(opening: &BookOpening) -> Fr {
    let mut t = Transcript::new(b"zeequent-board-v1");
    t.absorb_bytes(b"opening", &opening.to_bytes());
    t.challenge(b"digest")
}

fn next_digest(previous: &Fr, index: usize, column: Column, commitment: &Commitment) -> Fr {
    let mut t = Transcript::new(b"zeequent-board-v1");
    t.absorb_fr(b"previous", previous);
    t.absorb_u64(b"index", index as u64);
    t.absorb_u64(b"column", column.index() as u64);
    t.absorb_g1(b"commitment", &commitment.0);
    t.challenge(b"digest")
}

impl Board {
    pub fn new() -> Self {
        Self { entries: Vec::new() }
    }

    /// The digest that pins the whole history so far.
    pub fn head_digest(&self, opening: &BookOpening) -> Fr {
        self.entries.last().map(|e| e.digest).unwrap_or_else(|| opening_digest(opening))
    }

    /// Appends a single step.
    pub fn append(&mut self, opening: &BookOpening, receipt: &UpdateReceipt) {
        let previous = self.head_digest(opening);
        let digest = next_digest(&previous, self.entries.len(), receipt.column, &receipt.after);
        self.entries.push(BoardEntry {
            column: receipt.column,
            commitment: receipt.after,
            digest,
        });
    }

    /// Appends the four steps of one order: the new commitment of every column, in column order.
    pub fn append_order(&mut self, opening: &BookOpening, after: &[Commitment; 4]) {
        for (index, commitment) in after.iter().enumerate() {
            let previous = self.head_digest(opening);
            let column = Column::from_index(index).expect("four columns");
            let digest = next_digest(&previous, self.entries.len(), column, commitment);
            self.entries.push(BoardEntry { column, commitment: *commitment, digest });
        }
    }

    /// Recomputes every digest from the opening.
    pub fn verify(&self, opening: &BookOpening) -> Option<[Commitment; 4]> {
        let mut current = opening.commitments;
        let mut digest = opening_digest(opening);
        for (index, entry) in self.entries.iter().enumerate() {
            digest = next_digest(&digest, index, entry.column, &entry.commitment);
            if digest != entry.digest {
                return None;
            }
            current[entry.column.index()] = entry.commitment;
        }
        Some(current)
    }

    /// The change each logged step made to its column's commitment, in log order.
    pub fn deltas(&self, opening: &BookOpening) -> Vec<G1Affine> {
        let mut current = opening.commitments;
        let deltas: Vec<G1Projective> = self
            .entries
            .iter()
            .map(|entry| {
                let slot = &mut current[entry.column.index()];
                let delta = entry.commitment.0.into_group() - slot.0;
                *slot = entry.commitment;
                delta
            })
            .collect();
        G1Projective::normalize_batch(&deltas)
    }

    /// Whether the receipt's `before -> after` step appears in the log as consecutive states of its column.
    pub fn contains(&self, opening: &BookOpening, receipt: &UpdateReceipt) -> bool {
        let mut state = opening.commitments[receipt.column.index()];
        for entry in &self.entries {
            if entry.column != receipt.column {
                continue;
            }
            if state == receipt.before && entry.commitment == receipt.after {
                return true;
            }
            state = entry.commitment;
        }
        false
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut w = Writer::new(BOARD_MAGIC);
        w.u64(self.entries.len() as u64);
        for entry in &self.entries {
            w.u64(entry.column.index() as u64);
            w.g1(&entry.commitment.0);
            w.fr(&entry.digest);
        }
        w.finish()
    }

    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        let mut r = Reader::new(bytes, BOARD_MAGIC)?;
        let count = usize::try_from(r.u64()?).ok()?;
        // Reject counts the input cannot hold.
        if count.checked_mul(104)? > bytes.len() {
            return None;
        }
        let mut entries = Vec::with_capacity(count);
        for _ in 0..count {
            entries.push(BoardEntry {
                column: Column::from_index(usize::try_from(r.u64()?).ok()?)?,
                commitment: Commitment(r.g1()?),
                digest: r.fr()?,
            });
        }
        r.finish()?;
        Some(Self { entries })
    }
}

impl Default for Board {
    fn default() -> Self {
        Self::new()
    }
}
