//! Binary codec for files.

use ark_bn254::{Fr, G1Affine, G2Affine};
use ark_serialize::{CanonicalDeserialize, CanonicalSerialize};

pub struct Writer(Vec<u8>);

impl Writer {
    pub fn new(magic: &[u8; 8]) -> Self {
        Self(magic.to_vec())
    }

    pub fn finish(self) -> Vec<u8> {
        self.0
    }

    pub fn u64(&mut self, value: u64) {
        self.0.extend_from_slice(&value.to_le_bytes());
    }

    pub fn bytes(&mut self, data: &[u8]) {
        self.u64(data.len() as u64);
        self.0.extend_from_slice(data);
    }

    pub fn u64s(&mut self, values: &[u64]) {
        self.u64(values.len() as u64);
        for value in values {
            self.u64(*value);
        }
    }

    fn put<T: CanonicalSerialize>(&mut self, value: &T) {
        value
            .serialize_uncompressed(&mut self.0)
            .expect("writing to a Vec cannot fail");
    }

    pub fn fr(&mut self, value: &Fr) {
        self.put(value);
    }

    pub fn g1(&mut self, value: &G1Affine) {
        self.put(value);
    }

    pub fn g2(&mut self, value: &G2Affine) {
        self.put(value);
    }

    pub fn frs(&mut self, values: &[Fr]) {
        self.u64(values.len() as u64);
        for value in values {
            self.put(value);
        }
    }

    pub fn g1s(&mut self, values: &[G1Affine]) {
        self.u64(values.len() as u64);
        for value in values {
            self.put(value);
        }
    }
}

pub struct Reader<'a>(&'a [u8]);

impl<'a> Reader<'a> {
    /// Starts reading and checks the magic tag.
    pub fn new(bytes: &'a [u8], magic: &[u8; 8]) -> Option<Self> {
        let rest = bytes.strip_prefix(magic.as_slice())?;
        Some(Self(rest))
    }

    /// Succeeds only if every byte was consumed.
    pub fn finish(self) -> Option<()> {
        self.0.is_empty().then_some(())
    }

    pub fn u64(&mut self) -> Option<u64> {
        let (head, rest) = self.0.split_first_chunk::<8>()?;
        self.0 = rest;
        Some(u64::from_le_bytes(*head))
    }

    /// A length prefix, bounded by the remaining input.
    fn len(&mut self, item_size: usize) -> Option<usize> {
        let len = usize::try_from(self.u64()?).ok()?;
        (len.checked_mul(item_size)? <= self.0.len()).then_some(len)
    }

    pub fn bytes(&mut self) -> Option<&'a [u8]> {
        let len = self.len(1)?;
        let (head, rest) = self.0.split_at(len);
        self.0 = rest;
        Some(head)
    }

    pub fn u64s(&mut self) -> Option<Vec<u64>> {
        let len = self.len(8)?;
        (0..len).map(|_| self.u64()).collect()
    }

    fn take<T: CanonicalDeserialize>(&mut self) -> Option<T> {
        T::deserialize_uncompressed(&mut self.0).ok()
    }

    fn take_unchecked<T: CanonicalDeserialize>(&mut self) -> Option<T> {
        T::deserialize_uncompressed_unchecked(&mut self.0).ok()
    }

    pub fn fr(&mut self) -> Option<Fr> {
        self.take()
    }

    pub fn g1(&mut self) -> Option<G1Affine> {
        self.take()
    }

    pub fn g2(&mut self) -> Option<G2Affine> {
        self.take()
    }

    pub fn frs(&mut self) -> Option<Vec<Fr>> {
        let len = self.len(32)?;
        (0..len).map(|_| self.take()).collect()
    }

    pub fn g1s(&mut self) -> Option<Vec<G1Affine>> {
        let len = self.len(64)?;
        (0..len).map(|_| self.take()).collect()
    }

    /// As [`Reader::g1s`] without curve checks.
    pub fn g1s_unchecked(&mut self) -> Option<Vec<G1Affine>> {
        let len = self.len(64)?;
        (0..len).map(|_| self.take_unchecked()).collect()
    }
}
