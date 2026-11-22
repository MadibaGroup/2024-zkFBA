//! Fiat-Shamir transcript.

use ark_bn254::{Fr, G1Affine};
use ark_ff::PrimeField;
use ark_serialize::CanonicalSerialize;
use sha2::{Digest, Sha256};

#[derive(Clone, Debug)]
pub struct Transcript {
    state: [u8; 32],
}

impl Transcript {
    /// Starts a transcript bound to a protocol name, e.g. `b"zeequent-clearing-v1"`.
    pub fn new(protocol: &[u8]) -> Self {
        let mut hasher = Sha256::new();
        hasher.update(b"zeequent-transcript");
        hasher.update((protocol.len() as u64).to_le_bytes());
        hasher.update(protocol);
        Self {
            state: hasher.finalize().into(),
        }
    }

    pub fn absorb_bytes(&mut self, label: &[u8], data: &[u8]) {
        let mut hasher = Sha256::new();
        hasher.update(self.state);
        hasher.update(b"absorb");
        hasher.update((label.len() as u64).to_le_bytes());
        hasher.update(label);
        hasher.update((data.len() as u64).to_le_bytes());
        hasher.update(data);
        self.state = hasher.finalize().into();
    }

    pub fn absorb_u64(&mut self, label: &[u8], value: u64) {
        self.absorb_bytes(label, &value.to_le_bytes());
    }

    pub fn absorb_fr(&mut self, label: &[u8], value: &Fr) {
        let mut buf = Vec::with_capacity(32);
        value
            .serialize_compressed(&mut buf)
            .expect("field element serializes");
        self.absorb_bytes(label, &buf);
    }

    pub fn absorb_frs(&mut self, label: &[u8], values: &[Fr]) {
        let mut buf = Vec::with_capacity(32 * values.len());
        for value in values {
            value
                .serialize_compressed(&mut buf)
                .expect("field element serializes");
        }
        self.absorb_bytes(label, &buf);
    }

    pub fn absorb_g1(&mut self, label: &[u8], point: &G1Affine) {
        let mut buf = Vec::with_capacity(32);
        point
            .serialize_compressed(&mut buf)
            .expect("group element serializes");
        self.absorb_bytes(label, &buf);
    }

    /// Derives a challenge and ratchets the state.
    pub fn challenge(&mut self, label: &[u8]) -> Fr {
        let mut wide = [0u8; 64];
        for (index, chunk) in wide.chunks_mut(32).enumerate() {
            let mut hasher = Sha256::new();
            hasher.update(self.state);
            hasher.update(b"challenge");
            hasher.update((label.len() as u64).to_le_bytes());
            hasher.update(label);
            hasher.update([index as u8]);
            chunk.copy_from_slice(&hasher.finalize());
        }
        let mut hasher = Sha256::new();
        hasher.update(self.state);
        hasher.update(b"ratchet");
        hasher.update(wide);
        self.state = hasher.finalize().into();
        Fr::from_le_bytes_mod_order(&wide)
    }
}
