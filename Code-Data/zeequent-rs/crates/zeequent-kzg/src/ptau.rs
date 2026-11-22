//! Import of a powers-of-tau ceremony file in the snarkjs `.ptau` format.

use std::fs::File;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::Path;

use ark_bn254::{Fq, Fq2, Fr, G1Affine, G1Projective, G2Affine, G2Projective};
use ark_ec::{AffineRepr, CurveGroup, Group};
use ark_ff::{BigInteger, Field, One, PrimeField, UniformRand};
use ark_std::rand::RngCore;

use crate::{DomainKey, Srs, BOOK_BLINDING_DEGREE};

#[derive(Debug)]
pub enum PtauError {
    Io(std::io::Error),
    /// Not a `.ptau` file, or a version or curve we do not handle.
    Format(&'static str),
    /// The ceremony is too small for the requested domain or degree.
    TooSmall { power: u32, needed_power: u32 },
    /// A point is not on the curve under any known encoding.
    BadPoint,
    /// The field has no evaluation domain of the requested size.
    Domain,
}

impl From<std::io::Error> for PtauError {
    fn from(e: std::io::Error) -> Self {
        PtauError::Io(e)
    }
}

#[derive(Clone, Copy, Debug)]
struct Encoding {
    little_endian: bool,
    montgomery: bool,
}

const ENCODINGS: [Encoding; 4] = [
    Encoding { little_endian: true, montgomery: true },
    Encoding { little_endian: true, montgomery: false },
    Encoding { little_endian: false, montgomery: true },
    Encoding { little_endian: false, montgomery: false },
];

/// `(2^256 mod p)^-1`: converts out of Montgomery form.
fn montgomery_inverse() -> Fq {
    Fq::from(2u64)
        .pow([256u64])
        .inverse()
        .expect("2^256 is non-zero mod p")
}

fn decode_fq(bytes: &[u8], encoding: Encoding, r_inverse: &Fq) -> Fq {
    let value = if encoding.little_endian {
        Fq::from_le_bytes_mod_order(bytes)
    } else {
        Fq::from_be_bytes_mod_order(bytes)
    };
    if encoding.montgomery {
        value * r_inverse
    } else {
        value
    }
}

fn decode_g1(bytes: &[u8], encoding: Encoding, r_inverse: &Fq) -> Option<G1Affine> {
    let point = G1Affine::new_unchecked(
        decode_fq(&bytes[..32], encoding, r_inverse),
        decode_fq(&bytes[32..64], encoding, r_inverse),
    );
    point.is_on_curve().then_some(point)
}

/// G2 coordinates are `x.c0, x.c1, y.c0, y.c1`; `swapped` reads `c1` first.
fn decode_g2(bytes: &[u8], encoding: Encoding, swapped: bool, r_inverse: &Fq) -> Option<G2Affine> {
    let field = |i: usize| decode_fq(&bytes[32 * i..32 * (i + 1)], encoding, r_inverse);
    let pair = |a: usize, b: usize| {
        if swapped {
            Fq2::new(field(b), field(a))
        } else {
            Fq2::new(field(a), field(b))
        }
    };
    let point = G2Affine::new_unchecked(pair(0, 1), pair(2, 3));
    (point.is_on_curve() && point.is_in_correct_subgroup_assuming_on_curve()).then_some(point)
}

struct Sections {
    file: File,
    /// `(id, offset, size)`
    table: Vec<(u32, u64, u64)>,
}

impl Sections {
    fn open(path: &Path) -> Result<Self, PtauError> {
        let mut file = File::open(path)?;
        let mut magic = [0u8; 4];
        file.read_exact(&mut magic)?;
        if &magic != b"ptau" {
            return Err(PtauError::Format("missing ptau magic"));
        }
        if read_u32(&mut file)? != 1 {
            return Err(PtauError::Format("unsupported version"));
        }
        let count = read_u32(&mut file)?;
        let mut table = Vec::new();
        for _ in 0..count {
            let id = read_u32(&mut file)?;
            let size = read_u64(&mut file)?;
            let offset = file.stream_position()?;
            table.push((id, offset, size));
            file.seek(SeekFrom::Start(offset + size))?;
        }
        Ok(Self { file, table })
    }

    fn section(&self, id: u32) -> Result<(u64, u64), PtauError> {
        self.table
            .iter()
            .find(|(section, _, _)| *section == id)
            .map(|(_, offset, size)| (*offset, *size))
            .ok_or(PtauError::Format("missing section"))
    }

    /// Reads `count` records of `record` bytes starting at record `first`.
    fn read(&mut self, id: u32, record: usize, first: usize, count: usize) -> Result<Vec<u8>, PtauError> {
        let (offset, size) = self.section(id)?;
        let end = (first + count) as u64 * record as u64;
        if end > size {
            return Err(PtauError::Format("section shorter than expected"));
        }
        self.file.seek(SeekFrom::Start(offset + (first * record) as u64))?;
        let mut buffer = vec![0u8; count * record];
        self.file.read_exact(&mut buffer)?;
        Ok(buffer)
    }
}

fn read_u32(file: &mut File) -> Result<u32, PtauError> {
    let mut buffer = [0u8; 4];
    file.read_exact(&mut buffer)?;
    Ok(u32::from_le_bytes(buffer))
}

fn read_u64(file: &mut File) -> Result<u64, PtauError> {
    let mut buffer = [0u8; 8];
    file.read_exact(&mut buffer)?;
    Ok(u64::from_le_bytes(buffer))
}

impl Srs {
    /// Builds the SRS and the domain key for a domain of size `n` from a ceremony file.
    pub fn from_ptau(
        path: &Path,
        max_degree: usize,
        n: usize,
    ) -> Result<(Self, DomainKey), PtauError> {
        let mut sections = Sections::open(path)?;

        // Header: field size, prime, power.
        let (header_offset, _) = sections.section(1)?;
        sections.file.seek(SeekFrom::Start(header_offset))?;
        if read_u32(&mut sections.file)? != 32 {
            return Err(PtauError::Format("field elements are not 32 bytes"));
        }
        let mut prime = [0u8; 32];
        sections.file.read_exact(&mut prime)?;
        if prime.as_slice() != Fq::MODULUS.to_bytes_le().as_slice() {
            return Err(PtauError::Format("not a BN254 ceremony"));
        }
        let power = read_u32(&mut sections.file)?;

        // The ceremony must cover the domain and the degree.
        let g1_count = max_degree.max(n) + 1;
        let needed_power = (n + 1).next_power_of_two().trailing_zeros();
        let g1_available = (1usize << (power + 1)) - 1;
        if (1usize << power) <= n || g1_count > g1_available {
            return Err(PtauError::TooSmall { power, needed_power });
        }

        // Detect the field-element encoding on the first G1 point.
        let r_inverse = montgomery_inverse();
        let g1_bytes = sections.read(2, 64, 0, g1_count)?;
        let encoding = ENCODINGS
            .into_iter()
            .find(|e| {
                decode_g1(&g1_bytes[..64], *e, &r_inverse)
                    .is_some_and(|p| p == G1Affine::generator())
            })
            .ok_or(PtauError::BadPoint)?;

        let g: Vec<G1Affine> = g1_bytes
            .chunks(64)
            .map(|chunk| decode_g1(chunk, encoding, &r_inverse))
            .collect::<Option<_>>()
            .ok_or(PtauError::BadPoint)?;

        let gamma_bytes = sections.read(4, 64, 0, BOOK_BLINDING_DEGREE + 1)?;
        let gamma_g: Vec<G1Affine> = gamma_bytes
            .chunks(64)
            .map(|chunk| decode_g1(chunk, encoding, &r_inverse))
            .collect::<Option<_>>()
            .ok_or(PtauError::BadPoint)?;

        // G2: [1]_2, [tau]_2 and [tau^n]_2.
        let first_g2 = sections.read(3, 128, 0, 2)?;
        let swapped = [false, true]
            .into_iter()
            .find(|s| {
                decode_g2(&first_g2[..128], encoding, *s, &r_inverse)
                    .is_some_and(|p| p == G2Affine::generator())
            })
            .ok_or(PtauError::BadPoint)?;
        let g2 = |bytes: &[u8]| decode_g2(bytes, encoding, swapped, &r_inverse).ok_or(PtauError::BadPoint);
        let h = g2(&first_g2[..128])?;
        let tau_h = g2(&first_g2[128..])?;
        let tau_n_h = g2(&sections.read(3, 128, n, 1)?)?;

        let srs = Srs { g, gamma_g, h, tau_h };
        let key = DomainKey::from_powers(n, &srs, tau_n_h).ok_or(PtauError::Domain)?;
        Ok((srs, key))
    }
}

/// Writes a `.ptau` file from a locally sampled secret, in the encoding snarkjs uses.
pub fn write_insecure_ptau<R: RngCore>(path: &Path, power: u32, rng: &mut R) -> std::io::Result<()> {
    let tau = Fr::rand(rng);
    let alpha = Fr::rand(rng);
    let size = 1usize << power;
    let r = Fq::from(2u64).pow([256u64]);
    let fq = |value: Fq| (value * r).into_bigint().to_bytes_le();

    let g1_section = |count: usize, scale: Fr| -> Vec<u8> {
        let mut points = Vec::with_capacity(count);
        let mut scalar = scale;
        for _ in 0..count {
            points.push(G1Projective::generator() * scalar);
            scalar *= tau;
        }
        let mut bytes = Vec::with_capacity(count * 64);
        for point in G1Projective::normalize_batch(&points) {
            bytes.extend(fq(point.x));
            bytes.extend(fq(point.y));
        }
        bytes
    };
    let mut g2_points = Vec::with_capacity(size);
    let mut scalar = Fr::one();
    for _ in 0..size {
        g2_points.push(G2Projective::generator() * scalar);
        scalar *= tau;
    }
    let mut g2_bytes = Vec::with_capacity(size * 128);
    for point in G2Projective::normalize_batch(&g2_points) {
        for coordinate in [point.x.c0, point.x.c1, point.y.c0, point.y.c1] {
            g2_bytes.extend(fq(coordinate));
        }
    }

    let mut header = Vec::new();
    header.extend(32u32.to_le_bytes());
    header.extend(Fq::MODULUS.to_bytes_le());
    header.extend(power.to_le_bytes());
    header.extend(power.to_le_bytes());

    let sections: [(u32, Vec<u8>); 4] = [
        (1, header),
        (2, g1_section(2 * size - 1, Fr::one())),
        (3, g2_bytes),
        (4, g1_section(size, alpha)),
    ];
    let mut file = File::create(path)?;
    file.write_all(b"ptau")?;
    file.write_all(&1u32.to_le_bytes())?;
    file.write_all(&(sections.len() as u32).to_le_bytes())?;
    for (id, payload) in &sections {
        file.write_all(&id.to_le_bytes())?;
        file.write_all(&(payload.len() as u64).to_le_bytes())?;
        file.write_all(payload)?;
    }
    Ok(())
}
