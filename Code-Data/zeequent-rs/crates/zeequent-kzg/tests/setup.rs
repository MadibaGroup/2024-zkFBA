//! The ceremony importer and setup serialization.

use ark_bn254::{Bn254, Fr};
use ark_ec::pairing::Pairing;
use ark_ec::{AffineRepr, CurveGroup};
use ark_ff::UniformRand;
use ark_poly::DenseUVPolynomial;
use ark_std::test_rng;
use zeequent_kzg::ptau::{write_insecure_ptau, PtauError};
use zeequent_kzg::{
    batch_open, batch_verify, commit, setup_from_bytes, setup_to_bytes, Commitment, DomainKey,
    OpeningClaim, Poly, Srs,
};

fn temp_file(name: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!("zeequent-{}-{name}", std::process::id()))
}

#[test]
fn domain_key_from_points_matches_the_one_built_from_tau() {
    let rng = &mut test_rng();
    let n = 16;
    let (srs, expected) = Srs::insecure_setup_for_domain(n + 8, n, rng).unwrap();
    // [tau^n]_2 = [Z_H(tau)]_2 + [1]_2.
    let tau_n_h = (expected.vanishing_h.into_group() + srs.h.into_group()).into_affine();
    let derived = DomainKey::from_powers(n, &srs, tau_n_h).unwrap();
    assert_eq!(derived.lagrange_g, expected.lagrange_g);
    assert_eq!(derived.vanishing_h, expected.vanishing_h);
    assert_eq!(derived.blinding_basis, expected.blinding_basis);
}

#[test]
fn ceremony_file_round_trip() {
    let rng = &mut test_rng();
    let path = temp_file("ceremony.ptau");
    write_insecure_ptau(&path, 5, rng).unwrap();

    let n = 16;
    let (srs, key) = Srs::from_ptau(&path, n + 8, n).unwrap();
    assert_eq!(srs.g.len(), n + 9);
    assert_eq!(key.lagrange_g.len(), n);

    // The powers are consistent: e([tau^(i+1)]_1, [1]_2) = e([tau^i]_1, [tau]_2).
    for i in [0, 1, n + 7] {
        assert_eq!(Bn254::pairing(srs.g[i + 1], srs.h), Bn254::pairing(srs.g[i], srs.tau_h));
    }
    // The hiding basis shares tau: e([g tau]_1, [1]_2) = e([g]_1, [tau]_2).
    assert_eq!(
        Bn254::pairing(srs.gamma_g[1], srs.h),
        Bn254::pairing(srs.gamma_g[0], srs.tau_h)
    );
    // The Lagrange points sum to [1]_1, since the basis sums to the constant 1.
    let sum: ark_bn254::G1Projective = key.lagrange_g.iter().map(|p| p.into_group()).sum();
    assert_eq!(sum.into_affine(), srs.g[0]);
    // [Z_H(tau)]_2: e([tau^n]_1 - [1]_1, [1]_2) = e([1]_1, vanishing_h).
    let vanishing_g1 = (srs.g[n].into_group() - srs.g[0].into_group()).into_affine();
    assert_eq!(Bn254::pairing(vanishing_g1, srs.h), Bn254::pairing(srs.g[0], key.vanishing_h));

    // Committing and opening works with the imported SRS.
    let poly = Poly::from_coefficients_vec((0..n).map(|_| Fr::rand(rng)).collect());
    let (commitment, blinding) = commit(&srs, &poly, 2, rng).unwrap();
    let (point, gamma) = (Fr::rand(rng), Fr::rand(rng));
    let opening = batch_open(&srs, &[&poly], &[&blinding], point, gamma).unwrap();
    let commitments: [Commitment; 1] = [commitment];
    let claim = OpeningClaim { commitments: &commitments, opening: &opening, gamma };
    assert!(batch_verify(&srs.verifier_key(), &[claim], Fr::rand(rng)));

    // The serialized setup reads back identically.
    let (srs_again, key_again) = setup_from_bytes(&setup_to_bytes(&srs, &key)).unwrap();
    assert_eq!(srs_again.g, srs.g);
    assert_eq!(srs_again.gamma_g, srs.gamma_g);
    assert_eq!((srs_again.h, srs_again.tau_h), (srs.h, srs.tau_h));
    assert_eq!(key_again.lagrange_g, key.lagrange_g);
    assert_eq!(key_again.vanishing_h, key.vanishing_h);
    assert!(setup_from_bytes(&setup_to_bytes(&srs, &key)[1..]).is_none());

    // A ceremony too small for the domain is refused.
    assert!(matches!(Srs::from_ptau(&path, 40, 32), Err(PtauError::TooSmall { .. })));

    // A file that is not a ceremony is refused.
    let junk = temp_file("junk.ptau");
    std::fs::write(&junk, b"not a ceremony").unwrap();
    assert!(matches!(Srs::from_ptau(&junk, n + 8, n), Err(PtauError::Format(_))));

    let _ = std::fs::remove_file(path);
    let _ = std::fs::remove_file(junk);
}

#[test]
fn a_setup_is_checked_for_internal_consistency() {
    use ark_ec::AffineRepr;
    let rng = &mut test_rng();
    let n = 16;
    let (srs, key) = Srs::insecure_setup_for_domain(2 * n + 8, n, rng).unwrap();
    assert!(srs.is_consistent(&key, rng));

    // One power replaced: no longer a geometric sequence.
    let mut bad = srs.clone();
    bad.g[5] = bad.g[6];
    assert!(!bad.is_consistent(&key, rng));
    // The hiding basis replaced by unrelated points.
    let mut bad = srs.clone();
    bad.gamma_g[2] = bad.g[2];
    assert!(!bad.is_consistent(&key, rng));
    // [tau]_2 from another setup.
    let (other, other_key) = Srs::insecure_setup_for_domain(2 * n + 8, n, rng).unwrap();
    let mut bad = srs.clone();
    bad.tau_h = other.tau_h;
    assert!(!bad.is_consistent(&key, rng));
    // A domain key from another setup, a tampered Lagrange point, and a tampered vanishing point.
    assert!(!srs.is_consistent(&other_key, rng));
    let mut bad_key = key.clone();
    bad_key.lagrange_g[3] = (bad_key.lagrange_g[3].into_group() + srs.g[0]).into();
    assert!(!srs.is_consistent(&bad_key, rng));
    let mut bad_key = key.clone();
    bad_key.vanishing_h = srs.tau_h;
    assert!(!srs.is_consistent(&bad_key, rng));
}
