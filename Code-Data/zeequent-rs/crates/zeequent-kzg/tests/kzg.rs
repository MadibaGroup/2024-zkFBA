use ark_bn254::Fr;
use ark_ec::CurveGroup;
use ark_ff::{One, UniformRand};
use ark_poly::{DenseUVPolynomial, Polynomial};
use ark_std::test_rng;
use zeequent_kzg::{
    batch_open, batch_verify, commit, commit_with, evaluate_all, Blinding, Commitment, KzgError,
    OpeningClaim, Poly, Srs, Transcript,
};

fn random_poly(degree: usize, rng: &mut impl ark_std::rand::RngCore) -> Poly {
    Poly::from_coefficients_vec((0..=degree).map(|_| Fr::rand(rng)).collect())
}

#[test]
fn open_and_verify_one_point() {
    let rng = &mut test_rng();
    let srs = Srs::insecure_setup(32, rng);
    let vk = srs.verifier_key();

    let polys: Vec<Poly> = (0..5).map(|i| random_poly(10 + i, rng)).collect();
    let committed: Vec<(Commitment, Blinding)> =
        polys.iter().map(|p| commit(&srs, p, 2, rng).unwrap()).collect();
    let commitments: Vec<Commitment> = committed.iter().map(|(c, _)| *c).collect();
    let poly_refs: Vec<&Poly> = polys.iter().collect();
    let blinding_refs: Vec<&Blinding> = committed.iter().map(|(_, b)| b).collect();

    let point = Fr::rand(rng);
    let gamma = Fr::rand(rng);
    let opening = batch_open(&srs, &poly_refs, &blinding_refs, point, gamma).unwrap();
    assert_eq!(opening.evals, evaluate_all(&poly_refs, point));

    let claim = OpeningClaim { commitments: &commitments, opening: &opening, gamma };
    assert!(batch_verify(&vk, &[claim], Fr::rand(rng)));

    // A wrong evaluation is rejected.
    let mut wrong = opening.clone();
    wrong.evals[3] += Fr::one();
    let claim = OpeningClaim { commitments: &commitments, opening: &wrong, gamma };
    assert!(!batch_verify(&vk, &[claim], Fr::rand(rng)));

    // A wrong combined blinding evaluation is rejected.
    let mut wrong = opening.clone();
    wrong.blinding_eval += Fr::one();
    let claim = OpeningClaim { commitments: &commitments, opening: &wrong, gamma };
    assert!(!batch_verify(&vk, &[claim], Fr::rand(rng)));

    // A different combining challenge is rejected.
    let claim = OpeningClaim { commitments: &commitments, opening: &opening, gamma: gamma + Fr::one() };
    assert!(!batch_verify(&vk, &[claim], Fr::rand(rng)));

    // Commitments in the wrong order are rejected.
    let mut swapped = commitments.clone();
    swapped.swap(0, 1);
    let claim = OpeningClaim { commitments: &swapped, opening: &opening, gamma };
    assert!(!batch_verify(&vk, &[claim], Fr::rand(rng)));
}

#[test]
fn verify_two_points_together() {
    let rng = &mut test_rng();
    let srs = Srs::insecure_setup(16, rng);
    let vk = srs.verifier_key();

    let polys: Vec<Poly> = (0..4).map(|_| random_poly(12, rng)).collect();
    let committed: Vec<(Commitment, Blinding)> =
        polys.iter().map(|p| commit(&srs, p, 2, rng).unwrap()).collect();
    let commitments: Vec<Commitment> = committed.iter().map(|(c, _)| *c).collect();

    // All four at zeta; the first two also at a second point.
    let zeta = Fr::rand(rng);
    let shifted = zeta * Fr::from(7u64);
    let (gamma_a, gamma_b) = (Fr::rand(rng), Fr::rand(rng));

    let all_polys: Vec<&Poly> = polys.iter().collect();
    let all_blindings: Vec<&Blinding> = committed.iter().map(|(_, b)| b).collect();
    let opening_a = batch_open(&srs, &all_polys, &all_blindings, zeta, gamma_a).unwrap();
    let opening_b =
        batch_open(&srs, &all_polys[..2], &all_blindings[..2], shifted, gamma_b).unwrap();

    let claims = [
        OpeningClaim { commitments: &commitments, opening: &opening_a, gamma: gamma_a },
        OpeningClaim { commitments: &commitments[..2], opening: &opening_b, gamma: gamma_b },
    ];
    assert!(batch_verify(&vk, &claims, Fr::rand(rng)));

    // Corrupting the second opening fails the combined check.
    let mut wrong_b = opening_b.clone();
    wrong_b.evals[0] += Fr::one();
    let claims = [
        OpeningClaim { commitments: &commitments, opening: &opening_a, gamma: gamma_a },
        OpeningClaim { commitments: &commitments[..2], opening: &wrong_b, gamma: gamma_b },
    ];
    assert!(!batch_verify(&vk, &claims, Fr::rand(rng)));
}

#[test]
fn commitments_hide_and_add() {
    let rng = &mut test_rng();
    let srs = Srs::insecure_setup(16, rng);

    // Two commitments to the same polynomial differ.
    let p = random_poly(8, rng);
    let (c1, r1) = commit(&srs, &p, 2, rng).unwrap();
    let (c2, _) = commit(&srs, &p, 2, rng).unwrap();
    assert_ne!(c1, c2);

    // Commitments add: cm(p, r1) + cm(q, r2) = cm(p + q, r1 + r2).
    let q = random_poly(5, rng);
    let (cq, r2) = commit(&srs, &q, 2, rng).unwrap();
    let sum = Commitment((c1.0 + cq.0).into_affine());
    let expected = commit_with(&srs, &(&p + &q), &Blinding(&r1.0 + &r2.0)).unwrap();
    assert_eq!(sum, expected);

    // An unblinded commitment is deterministic.
    assert_eq!(
        commit_with(&srs, &p, &Blinding::none()).unwrap(),
        commit_with(&srs, &p, &Blinding::none()).unwrap()
    );
}

#[test]
fn srs_bounds_are_enforced() {
    let rng = &mut test_rng();
    let srs = Srs::insecure_setup(8, rng);
    assert_eq!(srs.max_degree(), 8);
    assert!(commit(&srs, &random_poly(8, rng), 2, rng).is_ok());
    assert_eq!(
        commit(&srs, &random_poly(9, rng), 2, rng).unwrap_err(),
        KzgError::SrsTooSmall { needed: 10, available: 9 }
    );
}

#[test]
fn opening_a_constant_and_a_zero_polynomial() {
    let rng = &mut test_rng();
    let srs = Srs::insecure_setup(8, rng);
    let vk = srs.verifier_key();

    let polys = [
        Poly::from_coefficients_vec(vec![Fr::from(5u64)]),
        Poly::from_coefficients_vec(vec![]),
    ];
    let blindings = [Blinding::none(), Blinding::random(1, rng)];
    let commitments: Vec<Commitment> = polys
        .iter()
        .zip(&blindings)
        .map(|(p, b)| commit_with(&srs, p, b).unwrap())
        .collect();

    let (point, gamma) = (Fr::rand(rng), Fr::rand(rng));
    let opening = batch_open(
        &srs,
        &[&polys[0], &polys[1]],
        &[&blindings[0], &blindings[1]],
        point,
        gamma,
    )
    .unwrap();
    assert_eq!(opening.evals, vec![Fr::from(5u64), Fr::from(0u64)]);
    assert_eq!(polys[0].evaluate(&point), Fr::from(5u64));
    let claim = OpeningClaim { commitments: &commitments, opening: &opening, gamma };
    assert!(batch_verify(&vk, &[claim], Fr::rand(rng)));
}

#[test]
fn transcript_is_deterministic_and_chained() {
    let run = |first: u64, second: u64| {
        let mut t = Transcript::new(b"test");
        t.absorb_u64(b"a", first);
        let c1 = t.challenge(b"alpha");
        t.absorb_u64(b"b", second);
        let c2 = t.challenge(b"zeta");
        (c1, c2)
    };

    assert_eq!(run(1, 2), run(1, 2));
    // Changing an early input changes every later challenge.
    assert_ne!(run(1, 2).0, run(9, 2).0);
    assert_ne!(run(1, 2).1, run(9, 2).1);
    // Changing a late input leaves earlier challenges alone.
    assert_eq!(run(1, 2).0, run(1, 9).0);
    assert_ne!(run(1, 2).1, run(1, 9).1);

    // Two challenges in a row differ, even with the same label.
    let mut t = Transcript::new(b"test");
    assert_ne!(t.challenge(b"x"), t.challenge(b"x"));
    // The protocol name and the labels are bound.
    let mut a = Transcript::new(b"one");
    let mut b = Transcript::new(b"two");
    assert_ne!(a.challenge(b"x"), b.challenge(b"x"));
    let mut a = Transcript::new(b"test");
    let mut b = Transcript::new(b"test");
    a.absorb_u64(b"left", 1);
    b.absorb_u64(b"right", 1);
    assert_ne!(a.challenge(b"x"), b.challenge(b"x"));
}
