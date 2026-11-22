//! End-to-end tests: open a book, apply orders, clear, prove, verify.

use ark_bn254::Fr;
use ark_ec::{AffineRepr, CurveGroup};
use ark_ff::{One, UniformRand};
use ark_poly::{DenseUVPolynomial, EvaluationDomain, Polynomial, Radix2EvaluationDomain};
use ark_std::rand::{rngs::StdRng, SeedableRng};
use zeequent_core::columns::Layout;
use zeequent_core::public_polys::{clearing_sets, eval_at, on_domain};
use zeequent_core::{
    prove, prove_no_cross, prove_no_cross_with, prove_with, required_srs_degree,
    verify, verify_book_opening, verify_no_cross, verify_update, BookState, ProveError,
    ProverOptions, VerifyError, Witness,
};
use zeequent_kzg::{Commitment, DomainKey, Poly, Srs, VerifierKey};
use zeequent_types::{Order, OrderKind, OrderSide, Outcome, Params, Receipt, Side};

const BUY: OrderSide = OrderSide::Buy;
const SELL: OrderSide = OrderSide::Sell;
const CHEAT: ProverOptions = ProverOptions { skip_checks: true };

fn limit(side: OrderSide, tick: usize, qty: u64) -> Order {
    Order { side, kind: OrderKind::Limit { tick }, qty }
}

fn io(side: OrderSide, tick: usize, qty: u64) -> Order {
    Order { side, kind: OrderKind::Io { tick }, qty }
}

struct Setup {
    params: Params,
    srs: Srs,
    key: DomainKey,
    vk: VerifierKey,
    rng: StdRng,
}

fn setup(n_ticks: usize, k_in: u32) -> Setup {
    let mut rng = StdRng::seed_from_u64(42);
    let params = Params::new(n_ticks, k_in).unwrap();
    let (srs, key) =
        Srs::insecure_setup_for_domain(required_srs_degree(&params), params.n, &mut rng).unwrap();
    let vk = srs.verifier_key();
    Setup { params, srs, key, vk, rng }
}

/// Opens a book, applies the orders, and clears it.
fn batch(s: &mut Setup, orders: &[Order]) -> (BookState, Outcome, Witness) {
    let (mut state, opening) = BookState::open(&s.params, &s.srs, &s.key, &mut s.rng).unwrap();
    assert!(verify_book_opening(&opening, &s.vk, &s.key));
    for order in orders {
        let receipt = state.apply(order, &s.vk, &s.key, &mut s.rng).unwrap();
        assert!(verify_update(&receipt, &s.vk, &s.key));
    }
    let (outcome, witness) = state.clear().unwrap();
    (state, outcome, witness)
}

fn crossing(s: &mut Setup, orders: &[Order]) -> (BookState, Receipt, Witness) {
    match batch(s, orders) {
        (state, Outcome::Cross(receipt), witness) => (state, receipt, witness),
        _ => panic!("expected a cross"),
    }
}

/// Bids 10 @ 5, asks 6 @ 2, plus IO orders: the running example.
fn example(s: &mut Setup) -> (BookState, Receipt, Witness) {
    crossing(s, &[limit(BUY, 5, 10), limit(SELL, 2, 6), io(SELL, 4, 3), io(BUY, 6, 9)])
}

#[test]
fn public_polynomials_match_their_interpolation() {
    let mut s = setup(8, 8);
    let (_, receipt, _) = example(&mut s);
    let domain = Radix2EvaluationDomain::<Fr>::new(s.params.n).unwrap();
    let x = Fr::rand(&mut s.rng);
    // The nine indicator polynomials and the lookup table.
    let mut sets = clearing_sets(&s.params, &receipt);
    sets.push(Layout::new(&s.params).lookup().table());
    assert_eq!(sets.len(), 10);
    let by_formula = eval_at(&domain, &sets, x);
    for (j, evals) in on_domain(&sets, s.params.n).iter().enumerate() {
        let poly = Poly::from_coefficients_vec(domain.ifft(evals));
        assert_eq!(poly.evaluate(&x), by_formula[j], "public polynomial {j}");
    }
}

#[test]
fn honest_proof_verifies() {
    let mut s = setup(8, 8);
    let (state, receipt, witness) = example(&mut s);
    let book = state.commitments();

    let proof =
        prove(&s.params, &receipt, &witness, &state.columns, &s.srs, &s.key, &mut s.rng).unwrap();
    assert_eq!(verify(&s.params, &receipt, &book, &proof, &s.vk), Ok(()));

    // n = 8, so limbs have 3 bits: 3 limbs for k_in = 8, 4 limbs for k = 11.
    let layout = Layout::new(&s.params);
    assert_eq!((layout.limb_bits, layout.limbs_in, layout.limbs_acc), (3, 3, 4));
    assert_eq!(layout.lookup().entries.len(), 41);
    assert_eq!(layout.phase1(), 46);
    assert_eq!(layout.total(), 69);
    assert_eq!(proof.num_commitments(), 70);
    // 72 points, 77 evaluations, 2 blinding evaluations, 2 opening points, three lengths.
    let bytes = proof.to_bytes();
    assert_eq!(bytes.len(), 32 * (72 + 77 + 2 + 2) + 12);
    assert_eq!(proof.size_bytes(), bytes.len());
    let decoded = zeequent_core::Proof::from_bytes(&bytes).unwrap();
    assert_eq!(decoded, proof);
    assert_eq!(verify(&s.params, &receipt, &book, &decoded, &s.vk), Ok(()));
    // Truncated, extended and corrupted encodings are rejected or fail to verify.
    assert!(zeequent_core::Proof::from_bytes(&bytes[..bytes.len() - 1]).is_none());
    let mut longer = bytes.clone();
    longer.push(0);
    assert!(zeequent_core::Proof::from_bytes(&longer).is_none());
    let mut flipped = bytes.clone();
    flipped[bytes.len() - 5] ^= 1;
    let accepted = zeequent_core::Proof::from_bytes(&flipped)
        .map(|p| verify(&s.params, &receipt, &book, &p, &s.vk).is_ok())
        .unwrap_or(false);
    assert!(!accepted);

    // Two proofs of the same statement differ: the prover is randomised.
    let again =
        prove(&s.params, &receipt, &witness, &state.columns, &s.srs, &s.key, &mut s.rng).unwrap();
    assert_ne!(proof, again);
    assert_eq!(verify(&s.params, &receipt, &book, &again, &s.vk), Ok(()));
}

#[test]
fn every_tie_break_case_proves() {
    let mut s = setup(8, 8);
    let books: Vec<Vec<Order>> = vec![
        // Unique maximizer.
        vec![limit(BUY, 6, 5), limit(BUY, 4, 5), limit(SELL, 2, 4), limit(SELL, 4, 4)],
        // Demand-long plateau.
        vec![limit(BUY, 5, 10), limit(SELL, 2, 6)],
        // Supply-long plateau.
        vec![limit(BUY, 5, 6), limit(SELL, 2, 10)],
        // Supply-long valley inside the plateau (tie-break active).
        vec![limit(BUY, 5, 6), limit(BUY, 3, 5), limit(SELL, 2, 6), limit(SELL, 4, 4)],
        // Balanced plateau.
        vec![limit(BUY, 5, 5), limit(SELL, 2, 5)],
        // Plateau touching both ends of the book: no cliffs.
        vec![limit(BUY, 7, 4), limit(SELL, 0, 4)],
        // Market orders and IO on the buy side.
        vec![
            Order { side: BUY, kind: OrderKind::Market, qty: 3 },
            limit(SELL, 3, 9),
            io(BUY, 5, 4),
        ],
        // Several orders at one tick.
        vec![limit(BUY, 5, 4), limit(BUY, 5, 6), limit(SELL, 2, 1), limit(SELL, 2, 5)],
    ];
    for orders in &books {
        let (state, receipt, witness) = crossing(&mut s, orders);
        let proof =
            prove(&s.params, &receipt, &witness, &state.columns, &s.srs, &s.key, &mut s.rng)
                .unwrap();
        assert_eq!(
            verify(&s.params, &receipt, &state.commitments(), &proof, &s.vk),
            Ok(()),
            "book {orders:?}"
        );
    }
}

#[test]
fn padded_domain_proves() {
    // 100 real ticks in a domain of 128, 16-bit volumes.
    let mut s = setup(100, 16);
    assert_eq!((s.params.n, s.params.k), (128, 23));
    let (state, receipt, witness) = crossing(
        &mut s,
        &[
            limit(BUY, 99, 40_000),
            limit(BUY, 60, 1_500),
            limit(SELL, 10, 30_000),
            limit(SELL, 55, 20_000),
            io(SELL, 70, 700),
        ],
    );
    let proof =
        prove(&s.params, &receipt, &witness, &state.columns, &s.srs, &s.key, &mut s.rng).unwrap();
    assert_eq!(verify(&s.params, &receipt, &state.commitments(), &proof, &s.vk), Ok(()));
}

#[test]
fn a_proof_does_not_verify_for_another_receipt_or_book() {
    let mut s = setup(8, 8);
    let (state, receipt, witness) = example(&mut s);
    let book = state.commitments();
    let proof =
        prove(&s.params, &receipt, &witness, &state.columns, &s.srs, &s.key, &mut s.rng).unwrap();

    let variants = [
        Receipt { v_max: receipt.v_max + 1, ..receipt },
        Receipt { c: receipt.c + 1, ..receipt },
        Receipt { d: receipt.d - 1, p_star: receipt.d - 1, ..receipt },
        Receipt { p_star: receipt.p_star - 1, ..receipt },
        Receipt { v_min_delta: receipt.v_min_delta - 1, ..receipt },
        Receipt { side: Side::SupplyLong, ..receipt },
        Receipt { p_at_price: receipt.p_at_price + 1, ..receipt },
        Receipt { j_io: receipt.j_io + 1, ..receipt },
    ];
    for variant in variants {
        assert!(verify(&s.params, &variant, &book, &proof, &s.vk).is_err(), "{variant:?}");
    }
    // A receipt that fails a scalar check is rejected before any cryptography.
    let outside = Receipt { p_star: receipt.d + 1, ..receipt };
    assert_eq!(verify(&s.params, &outside, &book, &proof, &s.vk), Err(VerifyError::Receipt));

    // One more order changes the book, and the old proof no longer verifies.
    let mut later = state.clone();
    later.apply(&limit(BUY, 5, 1), &s.vk, &s.key, &mut s.rng).unwrap();
    assert_eq!(
        verify(&s.params, &receipt, &later.commitments(), &proof, &s.vk),
        Err(VerifyError::Book)
    );
}

#[test]
fn tampered_proofs_are_rejected() {
    let mut s = setup(8, 8);
    let (state, receipt, witness) = example(&mut s);
    let book = state.commitments();
    let proof =
        prove(&s.params, &receipt, &witness, &state.columns, &s.srs, &s.key, &mut s.rng).unwrap();
    let check = |bad: &zeequent_core::Proof| verify(&s.params, &receipt, &book, bad, &s.vk);

    let mut bad = proof.clone();
    bad.at_zeta.evals[4] += Fr::one();
    assert!(check(&bad).is_err());

    let mut bad = proof.clone();
    bad.at_zeta_omega.evals[0] += Fr::one();
    assert!(check(&bad).is_err());

    // Swapping two prover-chosen commitments (columns 2 and 3).
    let mut bad = proof.clone();
    bad.commitments.swap(2, 3);
    assert!(check(&bad).is_err());

    // Replacing a book commitment.
    let mut bad = proof.clone();
    bad.commitments[0] = bad.commitments[5];
    assert_eq!(check(&bad), Err(VerifyError::Book));

    let mut bad = proof.clone();
    bad.at_zeta.blinding_eval += Fr::one();
    assert_eq!(check(&bad), Err(VerifyError::Opening));

    let mut bad = proof.clone();
    bad.commitments.pop();
    assert_eq!(check(&bad), Err(VerifyError::Shape));

    let mut bad = proof.clone();
    bad.at_zeta.point += Fr::one();
    assert_eq!(check(&bad), Err(VerifyError::Point));
}

#[test]
fn the_prover_refuses_a_false_statement() {
    let mut s = setup(8, 8);
    let (state, receipt, witness) = example(&mut s);
    let lowest_tick = Receipt { p_star: receipt.c, ..receipt };
    assert!(matches!(
        prove(&s.params, &lowest_tick, &witness, &state.columns, &s.srs, &s.key, &mut s.rng),
        Err(ProveError::Violation(_))
    ));
}

#[test]
fn a_cheating_prover_is_caught_by_the_verifier() {
    let mut s = setup(8, 8);
    let (state, receipt, witness) = example(&mut s);
    let book = state.commitments();

    // False receipts, proved by a prover that ignores its own checks.
    let false_receipts = [
        // "Lowest tick" instead of the tie-break rule.
        Receipt { p_star: receipt.c, ..receipt },
        // Understated volume.
        Receipt { v_max: receipt.v_max - 1, ..receipt },
        // A plateau reported one tick short.
        Receipt { c: receipt.c + 1, ..receipt },
        // Inflated eligible IO volume.
        Receipt { j_io: receipt.j_io + 5, ..receipt },
        // Overstated at-price volume.
        Receipt { p_at_price: receipt.p_at_price + 1, ..receipt },
    ];
    for false_receipt in false_receipts {
        let proof = prove_with(
            &s.params, &false_receipt, &witness, &state.columns, &s.srs, &s.key, &mut s.rng, CHEAT,
        )
        .unwrap();
        assert_eq!(
            verify(&s.params, &false_receipt, &book, &proof, &s.vk),
            Err(VerifyError::Identity),
            "{false_receipt:?}"
        );
    }

    // A true receipt with a corrupted witness.
    let mut shifted = witness.clone();
    for i in 0..shifted.a.len() - 1 {
        shifted.acc_a[i + 1] = shifted.acc_a[i] + shifted.a[i];
    }
    let proof = prove_with(
        &s.params, &receipt, &shifted, &state.columns, &s.srs, &s.key, &mut s.rng, CHEAT,
    )
    .unwrap();
    assert_eq!(verify(&s.params, &receipt, &book, &proof, &s.vk), Err(VerifyError::Identity));

    let mut moved = witness.clone();
    moved.b[5] -= 1;
    moved.b[6] += 1;
    let proof = prove_with(
        &s.params, &receipt, &moved, &state.columns, &s.srs, &s.key, &mut s.rng, CHEAT,
    )
    .unwrap();
    assert!(verify(&s.params, &receipt, &book, &proof, &s.vk).is_err());
}

#[test]
fn no_cross_batch_proves() {
    let mut s = setup(8, 8);
    let (state, outcome, witness) =
        batch(&mut s, &[limit(BUY, 2, 5), limit(SELL, 5, 7), io(SELL, 1, 3)]);
    assert_eq!(outcome, Outcome::NoCross);
    let book = state.commitments();

    let proof =
        prove_no_cross(&s.params, &witness, &state.columns, &s.srs, &s.key, &mut s.rng).unwrap();
    assert_eq!(verify_no_cross(&s.params, &book, &proof, &s.vk), Ok(()));
    // 4 value columns, 6 limb columns, multiplicities, 4 helpers, table helper, running sum, quotient.
    assert_eq!(proof.num_commitments(), 18);

    // An empty book does not cross either.
    let (empty, outcome, witness) = batch(&mut s, &[]);
    assert_eq!(outcome, Outcome::NoCross);
    let proof =
        prove_no_cross(&s.params, &witness, &empty.columns, &s.srs, &s.key, &mut s.rng).unwrap();
    assert_eq!(verify_no_cross(&s.params, &empty.commitments(), &proof, &s.vk), Ok(()));
    // A no-cross proof is not a clearing proof, and is bound to its own book.
    assert_eq!(verify_no_cross(&s.params, &book, &proof, &s.vk), Err(VerifyError::Book));
}

#[test]
fn a_crossing_book_cannot_be_declared_no_cross() {
    let mut s = setup(8, 8);
    let (state, _, witness) = example(&mut s);
    assert!(matches!(
        prove_no_cross(&s.params, &witness, &state.columns, &s.srs, &s.key, &mut s.rng),
        Err(ProveError::Violation(_))
    ));
    let proof = prove_no_cross_with(
        &s.params, &witness, &state.columns, &s.srs, &s.key, &mut s.rng, CHEAT,
    )
    .unwrap();
    assert_eq!(
        verify_no_cross(&s.params, &state.commitments(), &proof, &s.vk),
        Err(VerifyError::Identity)
    );
}

#[test]
fn book_opening_and_update_receipts() {
    let mut s = setup(8, 8);
    let (mut state, opening) = BookState::open(&s.params, &s.srs, &s.key, &mut s.rng).unwrap();
    assert!(verify_book_opening(&opening, &s.vk, &s.key));

    // An opening that already contains volume is rejected.
    let mut loaded = opening.clone();
    loaded.commitments[0] =
        Commitment((s.key.lagrange_g[3] * Fr::from(7u64) + loaded.commitments[0].0).into_affine());
    assert!(!verify_book_opening(&loaded, &s.vk, &s.key));
    // So is one whose blinding term E hides volume: the Schnorr proof fails.
    let mut smuggled = opening.clone();
    smuggled.proofs[0].e =
        (s.key.lagrange_g[3] * Fr::from(7u64) + smuggled.proofs[0].e).into_affine();
    assert!(!verify_book_opening(&smuggled, &s.vk, &s.key));

    // A trader's receipt verifies, and pins quantity, tick and blinding.
    let order = limit(BUY, 5, 10);
    let receipt = state.apply(&order, &s.vk, &s.key, &mut s.rng).unwrap();
    assert!(verify_update(&receipt, &s.vk, &s.key));
    assert_eq!((receipt.tick, receipt.qty), (5, 10));
    assert_eq!(receipt.before, opening.commitments[0]);
    assert_eq!(receipt.after, state.commitments()[0]);
    let mut wrong = receipt;
    wrong.qty = 9;
    assert!(!verify_update(&wrong, &s.vk, &s.key));
    let mut wrong = receipt;
    wrong.tick = 4;
    assert!(!verify_update(&wrong, &s.vk, &s.key));
    let mut wrong = receipt;
    wrong.rho += Fr::one();
    assert!(!verify_update(&wrong, &s.vk, &s.key));

    // Without rho, the true (qty, tick) cannot be confirmed from the commitments.
    let guess = (s.key.lagrange_g[5] * Fr::from(10u64) + receipt.before.0).into_affine();
    assert_ne!(guess, receipt.after.0);

    // Two identical orders give different updates.
    let second = state.apply(&order, &s.vk, &s.key, &mut s.rng).unwrap();
    assert_ne!(
        (second.after.0 - second.before.0).into_affine(),
        (receipt.after.0 - receipt.before.0).into_affine()
    );

    // Every order moves all four commitments.
    let before = state.commitments();
    let market = Order { side: SELL, kind: OrderKind::Market, qty: 2 };
    let receipt = state.apply(&market, &s.vk, &s.key, &mut s.rng).unwrap();
    let after = state.commitments();
    assert_eq!((receipt.column.index(), receipt.tick), (1, 0));
    for index in 0..4 {
        assert_ne!(before[index], after[index]);
    }
    let steps = &state.history[state.history.len() - 4..];
    for (index, step) in steps.iter().enumerate() {
        assert_eq!(step.column.index(), index);
        assert_eq!(step.qty, if index == 1 { 2 } else { 0 });
        assert_eq!(step.delta, (after[index].0.into_group() - before[index].0).into_affine());
        if index != 1 {
            assert_eq!(step.delta, (s.vk.gamma_g * step.rho).into_affine());
        }
    }

    // Out-of-range orders are refused and change nothing.
    let steps = state.history.len();
    assert!(state.apply(&limit(BUY, 1, 300), &s.vk, &s.key, &mut s.rng).is_err());
    assert!(state.apply(&limit(BUY, 8, 1), &s.vk, &s.key, &mut s.rng).is_err());
    assert_eq!((state.history.len(), state.commitments()), (steps, after));
}

#[test]
fn a_batch_runs_on_a_ceremony_imported_setup_and_survives_serialization() {
    use zeequent_core::{commitments_from_bytes, commitments_to_bytes, BookOpening, UpdateReceipt};
    use zeequent_kzg::ptau::write_insecure_ptau;
    use zeequent_kzg::{setup_from_bytes, setup_to_bytes};

    let mut rng = StdRng::seed_from_u64(99);
    let params = Params::new(8, 8).unwrap();
    let path = std::env::temp_dir().join(format!("zeequent-core-{}.ptau", std::process::id()));
    write_insecure_ptau(&path, 5, &mut rng).unwrap();
    let (srs, key) = Srs::from_ptau(&path, required_srs_degree(&params), params.n).unwrap();
    let _ = std::fs::remove_file(&path);
    // Everything below uses the setup as read back from its file format.
    let (srs, key) = setup_from_bytes(&setup_to_bytes(&srs, &key)).unwrap();
    let vk = srs.verifier_key();

    let (mut state, opening) = BookState::open(&params, &srs, &key, &mut rng).unwrap();
    let opening = BookOpening::from_bytes(&opening.to_bytes()).unwrap();
    assert!(verify_book_opening(&opening, &vk, &key));

    for order in [limit(BUY, 5, 10), limit(SELL, 2, 6), io(SELL, 4, 3)] {
        // The exchange's state is saved and reloaded between orders.
        state = BookState::from_bytes(&params, &state.to_bytes()).unwrap();
        let receipt = state.apply(&order, &vk, &key, &mut rng).unwrap();
        let receipt = UpdateReceipt::from_bytes(&receipt.to_bytes()).unwrap();
        assert!(verify_update(&receipt, &vk, &key));
    }
    let state = BookState::from_bytes(&params, &state.to_bytes()).unwrap();
    let book = commitments_from_bytes(&commitments_to_bytes(&state.commitments())).unwrap();

    let (outcome, witness) = state.clear().unwrap();
    let outcome = Outcome::from_text(&outcome.to_text()).unwrap();
    let Outcome::Cross(receipt) = outcome else { panic!("expected a cross") };
    let proof = prove(&params, &receipt, &witness, &state.columns, &srs, &key, &mut rng).unwrap();
    let proof = zeequent_core::Proof::from_bytes(&proof.to_bytes()).unwrap();
    assert_eq!(verify(&params, &receipt, &book, &proof, &vk), Ok(()));

    // Malformed files are rejected, not misread.
    assert!(BookState::from_bytes(&params, &state.to_bytes()[..40]).is_none());
    assert!(BookState::from_bytes(&Params::new(16, 8).unwrap(), &state.to_bytes()).is_none());
    assert!(BookOpening::from_bytes(&commitments_to_bytes(&book)).is_none());
    assert!(Outcome::from_text("outcome=cross\nv_max=3\n").is_none());
    assert_eq!(Outcome::from_text(&Outcome::NoCross.to_text()), Some(Outcome::NoCross));
}

#[test]
fn the_commitment_log_pins_the_history() {
    use zeequent_core::Board;

    let mut s = setup(8, 8);
    let (mut state, opening) = BookState::open(&s.params, &s.srs, &s.key, &mut s.rng).unwrap();
    let mut board = Board::new();
    assert_eq!(board.verify(&opening), Some(opening.commitments));

    let orders = [limit(BUY, 5, 10), limit(SELL, 2, 6), limit(BUY, 5, 3), io(SELL, 4, 3)];
    let mut receipts = Vec::new();
    for order in &orders {
        let receipt = state.apply(order, &s.vk, &s.key, &mut s.rng).unwrap();
        board.append_order(&opening, &state.commitments());
        receipts.push(receipt);
    }

    // The log reproduces the current commitments and survives its file format.
    let board = Board::from_bytes(&board.to_bytes()).unwrap();
    assert_eq!(board.verify(&opening), Some(state.commitments()));
    // Every trader finds their own update in it.
    for receipt in &receipts {
        assert!(verify_update(receipt, &s.vk, &s.key));
        assert!(board.contains(&opening, receipt));
    }
    // A receipt for an update that is not in the log is not found.
    let mut other = state.clone();
    let stray = other.apply(&limit(BUY, 1, 1), &s.vk, &s.key, &mut s.rng).unwrap();
    assert!(!board.contains(&opening, &stray));

    // The clearing proof verifies against the head of the log.
    let (outcome, witness) = state.clear().unwrap();
    let Outcome::Cross(receipt) = outcome else { panic!("expected a cross") };
    let proof =
        prove(&s.params, &receipt, &witness, &state.columns, &s.srs, &s.key, &mut s.rng).unwrap();
    let head = board.verify(&opening).unwrap();
    assert_eq!(verify(&s.params, &receipt, &head, &proof, &s.vk), Ok(()));

    // Any rewrite of history breaks the chain.
    let mut dropped = board.clone();
    dropped.entries.remove(1);
    assert_eq!(dropped.verify(&opening), None);
    let mut swapped = board.clone();
    swapped.entries.swap(0, 1);
    assert_eq!(swapped.verify(&opening), None);
    let mut replaced = board.clone();
    replaced.entries[2].commitment = replaced.entries[0].commitment;
    assert_eq!(replaced.verify(&opening), None);
    let (_, another_opening) = BookState::open(&s.params, &s.srs, &s.key, &mut s.rng).unwrap();
    assert_eq!(board.verify(&another_opening), None);
    // A truncated log is valid but has another digest.
    let mut shorter = board.clone();
    shorter.entries.pop();
    assert!(shorter.verify(&opening).is_some());
    assert_ne!(shorter.head_digest(&opening), board.head_digest(&opening));
    assert!(Board::from_bytes(&board.to_bytes()[..50]).is_none());
}

/// The verifier agrees with the integer specification.
#[test]
fn the_verifier_agrees_with_the_integer_checker() {
    use ark_std::rand::Rng;
    use zeequent_core::check;

    let mut s = setup(6, 4);
    let n = s.params.n;
    let (mut accepted, mut rejected) = (0, 0);
    for _ in 0..12 {
        let orders: Vec<Order> = (0..s.rng.gen_range(2..7))
            .map(|_| {
                let side = if s.rng.gen_bool(0.5) { BUY } else { SELL };
                let tick = s.rng.gen_range(0..6);
                let qty = s.rng.gen_range(1..4);
                if s.rng.gen_bool(0.25) { io(side, tick, qty) } else { limit(side, tick, qty) }
            })
            .collect();
        let (state, outcome, witness) = batch(&mut s, &orders);
        let book = state.commitments();
        let Outcome::Cross(receipt) = outcome else {
            let proof = prove_no_cross(
                &s.params, &witness, &state.columns, &s.srs, &s.key, &mut s.rng,
            )
            .unwrap();
            assert_eq!(verify_no_cross(&s.params, &book, &proof, &s.vk), Ok(()));
            continue;
        };

        let mut candidates = vec![receipt];
        for _ in 0..4 {
            let mut r = receipt;
            match s.rng.gen_range(0..8) {
                0 => r.v_max += 1,
                1 => r.v_max -= 1,
                2 => r.c = s.rng.gen_range(0..=r.p_star),
                3 => r.d = s.rng.gen_range(r.p_star..6),
                4 => { r.p_star = s.rng.gen_range(r.c..=r.d) }
                5 => r.v_min_delta = s.rng.gen_range(0..=r.v_min_delta + 2),
                6 => r.side = if r.side == Side::DemandLong { Side::SupplyLong } else { Side::DemandLong },
                _ => { r.p_at_price += s.rng.gen_range(0..3); r.j_io += s.rng.gen_range(0..3) }
            }
            candidates.push(r);
        }
        for r in candidates {
            // The cheating witness: slack refitted to the claimed receipt.
            let mut w = witness.clone();
            w.slack = vec![0; n];
            let cliff = |i: usize| r.v_max.wrapping_sub(w.min[i]).wrapping_sub(1);
            let left = r.c.checked_sub(1).map(|i| (i, cliff(i)));
            let right = (r.d + 1 < n).then(|| (r.d + 1, cliff(r.d + 1)));
            for (i, value) in left.into_iter().chain(right) {
                w.slack[i] = value;
            }
            if r.p_star > r.c && (r.side == Side::SupplyLong || r.v_min_delta == 0) {
                w.slack[r.p_star - 1] =
                    w.delta[r.p_star - 1].wrapping_sub(r.v_min_delta).wrapping_sub(1);
            }
            let expected = check(&s.params, &Outcome::Cross(r), &w).is_ok();
            assert_eq!(expected, r == receipt, "{r:?} vs {receipt:?}");
            let proof = prove_with(
                &s.params, &r, &w, &state.columns, &s.srs, &s.key, &mut s.rng, CHEAT,
            )
            .unwrap();
            let verified = verify(&s.params, &r, &book, &proof, &s.vk).is_ok();
            assert_eq!(verified, expected, "{r:?} vs {receipt:?}");
            if verified { accepted += 1 } else { rejected += 1 }
        }
    }
    assert!(accepted >= 4 && rejected >= 12, "{accepted} accepted, {rejected} rejected");
}

/// A logged step that removes an order is caught by the update-validity proof.
#[test]
fn a_logged_step_cannot_cancel_an_order() {
    use ark_ec::CurveGroup;
    use zeequent_core::{prove_updates, prove_updates_with, verify_updates, Board, UpdateReceipt, UpdateStep};
    use zeequent_types::Column;

    // Thirteen steps need a 16-row step domain.
    let mut s = setup(8, 8);
    let size = 16;
    let (srs, key) = Srs::insecure_setup_for_domain(
        zeequent_core::required_srs_degree_updates(size), 8, &mut s.rng,
    )
    .unwrap();
    let update_key = DomainKey::lagrange_only(size, &srs).unwrap();
    s = Setup { vk: srs.verifier_key(), srs, key, ..s };
    let (mut state, opening) = BookState::open(&s.params, &s.srs, &s.key, &mut s.rng).unwrap();
    let mut board = Board::new();
    for order in [limit(BUY, 5, 10), limit(SELL, 2, 6)] {
        state.apply(&order, &s.vk, &s.key, &mut s.rng).unwrap();
        board.append_order(&opening, &state.commitments());
    }
    let without_the_order = state.clear().unwrap().0;

    // A trader sells 9 at tick 1 and checks the receipt against the log.
    let trader = state.apply(&limit(SELL, 1, 9), &s.vk, &s.key, &mut s.rng).unwrap();
    board.append_order(&opening, &state.commitments());
    assert!(verify_update(&trader, &s.vk, &s.key));
    assert!(board.contains(&opening, &trader));
    assert_ne!(state.clear().unwrap().0, without_the_order);

    // So far the log is honest and its update-validity proof verifies.
    let honest =
        prove_updates(&s.params, &state.history, &s.srs, &s.key, &update_key, &mut s.rng).unwrap();
    let deltas = board.deltas(&opening);
    assert_eq!(verify_updates(&s.params, &deltas, size, &honest, &s.vk, &s.key), Ok(()));

    // The exchange logs one more step on the same column: minus 9 at tick 1.
    let index = Column::Asks.index();
    let rho = Fr::rand(&mut s.rng);
    let before = state.columns[index].commitment;
    let after = Commitment(
        (s.vk.gamma_g * rho - s.key.lagrange_g[1] * Fr::from(9u64) + before.0).into_affine(),
    );
    let delta = (s.vk.gamma_g * rho - s.key.lagrange_g[1] * Fr::from(9u64)).into_affine();
    state.book.asks[1] -= 9;
    state.columns[index].commitment = after;
    state.columns[index].blinding.0.coeffs[0] += rho;
    let forged = UpdateReceipt { column: Column::Asks, tick: 1, qty: 0, rho, before, after };
    board.append(&opening, &forged);

    // The clearing proof alone still verifies, and clears without the order.
    let head = board.verify(&opening).unwrap();
    assert_eq!(head, state.commitments());
    assert!(board.contains(&opening, &trader));
    let (outcome, witness) = state.clear().unwrap();
    assert_eq!(outcome, without_the_order);
    let Outcome::Cross(receipt) = outcome else { panic!("expected a cross") };
    let proof =
        prove(&s.params, &receipt, &witness, &state.columns, &s.srs, &s.key, &mut s.rng).unwrap();
    assert_eq!(verify(&s.params, &receipt, &head, &proof, &s.vk), Ok(()));

    // But no update-validity proof exists for the extended log.
    let deltas = board.deltas(&opening);
    assert!(verify_updates(&s.params, &deltas, size, &honest, &s.vk, &s.key).is_err());
    for (tick, qty) in [(1, 0), (1, 9), (3, 2), (0, 255)] {
        let mut history = state.history.clone();
        history.push(UpdateStep { column: Column::Asks, tick, qty, rho, delta });
        assert!(matches!(
            prove_updates(&s.params, &history, &s.srs, &s.key, &update_key, &mut s.rng),
            Err(ProveError::Violation(_))
        ));
        let cheat =
            prove_updates_with(&s.params, &history, &s.srs, &s.key, &update_key, &mut s.rng, CHEAT)
                .unwrap();
        assert!(verify_updates(&s.params, &deltas, size, &cheat, &s.vk, &s.key).is_err());
    }
}

#[test]
fn update_validity_proofs() {
    use zeequent_core::{
        prove_updates, required_srs_degree_updates, update_domain_size, verify_updates, Board,
        UpdateProof,
    };

    // More steps than ticks: the step domain is larger than the book's.
    let params = Params::new(8, 8).unwrap();
    let mut rng = StdRng::seed_from_u64(5);
    // Twenty orders log eighty steps.
    let size = update_domain_size(params.n, 80);
    assert_eq!(size, 128);
    // One setup serves both domains.
    let (srs, key) =
        Srs::insecure_setup_for_domain(required_srs_degree_updates(size), params.n, &mut rng)
            .unwrap();
    let update_key = DomainKey::lagrange_only(size, &srs).unwrap();
    let vk = srs.verifier_key();

    let (mut state, opening) = BookState::open(&params, &srs, &key, &mut rng).unwrap();
    let mut board = Board::new();
    for i in 0..20u64 {
        let order = match i % 4 {
            0 => limit(BUY, (i % 8) as usize, 1 + i),
            1 => limit(SELL, (i % 5) as usize, 2 + i),
            2 => io(BUY, 7, 3),
            _ => io(SELL, 0, 1),
        };
        state.apply(&order, &vk, &key, &mut rng).unwrap();
        board.append_order(&opening, &state.commitments());
    }
    let deltas = board.deltas(&opening);
    assert_eq!(deltas.len(), 80);
    // The log's column tags are 0, 1, 2, 3 repeated.
    for (i, entry) in board.entries.iter().enumerate() {
        assert_eq!(entry.column.index(), i % 4);
    }
    assert_eq!(board.verify(&opening), Some(state.commitments()));
    assert_eq!(deltas, state.history.iter().map(|step| step.delta).collect::<Vec<_>>());

    let proof = prove_updates(&params, &state.history, &srs, &key, &update_key, &mut rng).unwrap();
    assert_eq!(verify_updates(&params, &deltas, size, &proof, &vk, &key), Ok(()));
    let decoded = UpdateProof::from_bytes(&proof.to_bytes()).unwrap();
    assert_eq!(decoded, proof);

    // Bound to the log: a shorter log, a reordered log, another domain size.
    assert!(verify_updates(&params, &deltas[..79], size, &proof, &vk, &key).is_err());
    let mut swapped = deltas.clone();
    swapped.swap(3, 4);
    assert!(verify_updates(&params, &swapped, size, &proof, &vk, &key).is_err());
    assert!(verify_updates(&params, &deltas, 256, &proof, &vk, &key).is_err());

    // The state file keeps the history.
    let reloaded = BookState::from_bytes(&params, &state.to_bytes()).unwrap();
    assert_eq!(reloaded.history, state.history);

    // An empty log has a proof too.
    let (empty, opening) = BookState::open(&params, &srs, &key, &mut rng).unwrap();
    let proof = prove_updates(&params, &empty.history, &srs, &key, &update_key, &mut rng).unwrap();
    let none = Board::new().deltas(&opening);
    assert_eq!(verify_updates(&params, &none, size, &proof, &vk, &key), Ok(()));

    // A step of 300 does not fit 8 bits: refused by the prover, rejected by the verifier.
    let mut history = state.history.clone();
    let rho = Fr::rand(&mut rng);
    let delta = (key.lagrange_g[2] * Fr::from(300u64) + vk.gamma_g * rho).into_affine();
    history.push(zeequent_core::UpdateStep {
        column: zeequent_types::Column::Bids, tick: 2, qty: 300, rho, delta,
    });
    let mut longer = deltas.clone();
    longer.push(delta);
    assert!(matches!(
        prove_updates(&params, &history, &srs, &key, &update_key, &mut rng),
        Err(ProveError::Violation(_))
    ));
    let cheat = zeequent_core::prove_updates_with(
        &params, &history, &srs, &key, &update_key, &mut rng, CHEAT,
    )
    .unwrap();
    assert!(verify_updates(&params, &longer, size, &cheat, &vk, &key).is_err());
}

/// Proving the same book twice gives the same proof.
#[test]
fn the_batch_close_proof_is_reproducible() {
    use zeequent_core::prove_updates;

    let mut s = setup(8, 8);
    let (state, receipt, witness) = example(&mut s);
    let book = state.commitments();
    let close = |state: &BookState| {
        prove(
            &s.params, &receipt, &witness, &state.columns, &s.srs, &s.key,
            &mut state.prover_rng(b"clearing"),
        )
        .unwrap()
    };
    let first = close(&state);
    assert_eq!(verify(&s.params, &receipt, &book, &first, &s.vk), Ok(()));
    assert_eq!(close(&state).to_bytes(), first.to_bytes());
    // Also after the state went through its file format.
    let reloaded = BookState::from_bytes(&s.params, &state.to_bytes()).unwrap();
    assert_eq!(close(&reloaded).to_bytes(), first.to_bytes());

    // Separate randomness per proof and per batch.
    use ark_std::rand::RngCore;
    let draw = |state: &BookState, purpose: &[u8]| state.prover_rng(purpose).next_u64();
    assert_ne!(draw(&state, b"clearing"), draw(&state, b"updates"));
    let (other, _, _) = example(&mut s);
    assert_ne!(draw(&state, b"clearing"), draw(&other, b"clearing"));

    // The update-validity proof is reproducible in the same way.
    let (small, _, _) = batch(&mut s, &[limit(BUY, 5, 10), limit(SELL, 2, 6)]);
    let reloaded = BookState::from_bytes(&s.params, &small.to_bytes()).unwrap();
    let updates = |state: &BookState| {
        prove_updates(
            &s.params, &state.history, &s.srs, &s.key, &s.key,
            &mut state.prover_rng(b"updates"),
        )
        .unwrap()
        .to_bytes()
    };
    assert_eq!(updates(&small), updates(&reloaded));
}
/// Every derived column is constrained.
#[test]
fn every_derived_column_is_constrained() {
    use zeequent_core::check;

    let mut s = setup(8, 8);
    let (state, receipt, witness) = example(&mut s);
    let book = state.commitments();
    let outcome = Outcome::Cross(receipt);
    let (mut accepted, mut rejected) = (0, 0);

    type Column = fn(&mut Witness) -> &mut Vec<u64>;
    let columns: [(&str, Column); 9] = [
        ("acc_b", |w| &mut w.acc_b),
        ("acc_a", |w| &mut w.acc_a),
        ("min", |w| &mut w.min),
        ("surp_b", |w| &mut w.surp_b),
        ("surp_a", |w| &mut w.surp_a),
        ("delta", |w| &mut w.delta),
        ("slack", |w| &mut w.slack),
        ("io_acc_b", |w| &mut w.io_acc_b),
        ("io_acc_a", |w| &mut w.io_acc_a),
    ];
    for (name, column) in columns {
        for tick in 0..s.params.n {
            let mut w = witness.clone();
            column(&mut w)[tick] += 1;
            let expected = check(&s.params, &outcome, &w).is_ok();
            // Every cell of every column but `slack` is pinned.
            assert!(!expected || name == "slack", "{name}[{tick}] is unconstrained");
            let proof = prove_with(
                &s.params, &receipt, &w, &state.columns, &s.srs, &s.key, &mut s.rng, CHEAT,
            )
            .unwrap();
            let verified = verify(&s.params, &receipt, &book, &proof, &s.vk).is_ok();
            assert_eq!(verified, expected, "{name}[{tick}]");
            if verified { accepted += 1 } else { rejected += 1 }
        }
    }
    // 8 columns of 8 pinned cells, and the pinned cells of `slack`.
    assert!(rejected >= 64 && accepted >= 1, "{accepted} accepted, {rejected} rejected");
}

/// `verify_batch` is the whole public check.
#[test]
fn the_whole_batch_is_checked_in_one_call() {
    use zeequent_core::{
        prove_updates, verify_batch, BatchError, Board, BookOpening, PublishedBatch, UpdateProof,
    };

    struct Published {
        opening: BookOpening,
        board: Board,
        outcome: Outcome,
        proof: zeequent_core::Proof,
        updates: UpdateProof,
    }
    let mut s = setup(8, 8);
    let mut publish = |orders: &[Order]| {
        let (mut state, opening) =
            BookState::open(&s.params, &s.srs, &s.key, &mut s.rng).unwrap();
        let mut board = Board::new();
        for order in orders {
            state.apply(order, &s.vk, &s.key, &mut s.rng).unwrap();
            board.append_order(&opening, &state.commitments());
        }
        let (outcome, witness) = state.clear().unwrap();
        let mut rng = state.prover_rng(b"clearing");
        let proof = match &outcome {
            Outcome::Cross(receipt) => {
                prove(&s.params, receipt, &witness, &state.columns, &s.srs, &s.key, &mut rng)
            }
            Outcome::NoCross => {
                prove_no_cross(&s.params, &witness, &state.columns, &s.srs, &s.key, &mut rng)
            }
        }
        .unwrap();
        let updates = prove_updates(
            &s.params, &state.history, &s.srs, &s.key, &s.key, &mut state.prover_rng(b"updates"),
        )
        .unwrap();
        Published { opening, board, outcome, proof, updates }
    };
    let a = publish(&[limit(BUY, 5, 10), limit(SELL, 2, 6)]);
    let b = publish(&[limit(BUY, 6, 4), limit(SELL, 1, 9)]);
    let quiet = publish(&[limit(BUY, 2, 5), limit(SELL, 5, 7)]);
    assert_eq!(quiet.outcome, Outcome::NoCross);

    let params = s.params;
    let (vk, key) = (s.vk, s.key.clone());
    let check = |opening: &BookOpening, board: &Board, outcome: &Outcome,
                 proof: &zeequent_core::Proof, updates: &UpdateProof| {
        let batch = PublishedBatch { opening, board, outcome, proof, updates };
        verify_batch(&params, 8, &batch, &vk, &key)
    };

    for batch in [&a, &b, &quiet] {
        let verified =
            check(&batch.opening, &batch.board, &batch.outcome, &batch.proof, &batch.updates)
                .unwrap();
        assert_eq!(verified.steps, 8);
        assert_eq!(verified.log_digest, batch.board.head_digest(&batch.opening));
    }

    // Another batch's opening: the log no longer chains from it.
    assert_eq!(
        check(&b.opening, &a.board, &a.outcome, &a.proof, &a.updates).unwrap_err(),
        BatchError::Log
    );
    // Another batch's update-validity proof.
    assert!(matches!(
        check(&a.opening, &a.board, &a.outcome, &a.proof, &b.updates),
        Err(BatchError::Updates(_))
    ));
    // Another batch's clearing proof, or another outcome.
    assert!(matches!(
        check(&a.opening, &a.board, &a.outcome, &b.proof, &a.updates),
        Err(BatchError::Outcome(_))
    ));
    assert!(matches!(
        check(&a.opening, &a.board, &b.outcome, &a.proof, &a.updates),
        Err(BatchError::Outcome(_))
    ));
    // A crossing batch declared no-cross, and the reverse.
    assert!(matches!(
        check(&a.opening, &a.board, &Outcome::NoCross, &quiet.proof, &a.updates),
        Err(BatchError::Outcome(_))
    ));
    assert!(matches!(
        check(&quiet.opening, &quiet.board, &a.outcome, &quiet.proof, &quiet.updates),
        Err(BatchError::Outcome(_))
    ));
    // A log with its last order dropped: its own proofs no longer fit.
    let mut shorter = a.board.clone();
    shorter.entries.truncate(4);
    assert!(check(&a.opening, &shorter, &a.outcome, &a.proof, &a.updates).is_err());
    // An opening whose emptiness proof is broken.
    let mut broken = a.opening.clone();
    broken.proofs[0].e = broken.proofs[1].e;
    assert_eq!(
        check(&broken, &a.board, &a.outcome, &a.proof, &a.updates).unwrap_err(),
        BatchError::Opening
    );
}
