//! Range checks by lookup (logUp).

use ark_bn254::Fr;
use ark_ff::{batch_inversion, One, Zero};

/// The lookup entries that range-check one quantity of `bits` bits.
pub fn range_entries(limb_bits: usize, start: usize, bits: usize) -> Vec<(usize, u64)> {
    let limbs = bits.div_ceil(limb_bits);
    let mut entries: Vec<(usize, u64)> = (0..limbs).map(|j| (start + j, 1)).collect();
    let rest = bits % limb_bits;
    if rest != 0 {
        entries.push((start + limbs - 1, 1 << (limb_bits - rest)));
    }
    entries
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Lookup {
    /// `w`: bits per limb.
    pub limb_bits: usize,
    /// The looked-up values: a column index and a power-of-two scale.
    pub entries: Vec<(usize, u64)>,
    /// Index of the multiplicity column `m`; the last first-round column.
    pub mult: usize,
}

impl Lookup {
    /// Limb width for a domain of size `n`.
    pub fn limb_bits_for(n: usize) -> usize {
        (n.trailing_zeros() as usize).clamp(1, 8)
    }

    pub fn table_size(&self) -> usize {
        1 << self.limb_bits
    }

    /// Number of first-round columns.
    pub fn phase1(&self) -> usize {
        self.mult + 1
    }

    pub fn num_helpers(&self) -> usize {
        self.entries.len().div_ceil(2)
    }

    pub fn helper(&self, i: usize) -> usize {
        self.phase1() + i
    }

    /// The table helper `u`.
    pub fn table_helper(&self) -> usize {
        self.phase1() + self.num_helpers()
    }

    /// The running sum `S`.
    pub fn running_sum(&self) -> usize {
        self.table_helper() + 1
    }

    /// Total number of columns.
    pub fn total(&self) -> usize {
        self.running_sum() + 1
    }

    /// The table as a public polynomial: value `i` at domain point `i`.
    pub fn table(&self) -> Vec<(usize, Fr)> {
        (1..self.table_size()).map(|i| (i, Fr::from(i as u64))).collect()
    }

    /// Splits each value into `count` limbs, least significant first.
    pub fn limbs(&self, values: &[u64], count: usize) -> Vec<Vec<u64>> {
        let mask = (1u64 << self.limb_bits) - 1;
        (0..count)
            .map(|j| {
                let shift = self.limb_bits * j;
                values
                    .iter()
                    .map(|v| if shift < 64 { (v >> shift) & mask } else { 0 })
                    .collect()
            })
            .collect()
    }

    /// `m[v]`: how many entry cells equal `v`.
    pub fn multiplicities(&self, columns: &[Vec<u64>], n: usize) -> Vec<u64> {
        let size = self.table_size() as u64;
        let mut m = vec![0u64; n];
        for (column, scale) in &self.entries {
            for value in &columns[*column] {
                let cell = value.wrapping_mul(*scale);
                if cell < size {
                    m[cell as usize] += 1;
                }
            }
        }
        m
    }

    /// The second-round columns: the helpers, then `u`, then `S`.
    pub fn phase2(&self, beta: Fr, phase1: &[Vec<Fr>]) -> Vec<Vec<Fr>> {
        let n = phase1[0].len();
        let inverses: Vec<Vec<Fr>> = self
            .entries
            .iter()
            .map(|(column, scale)| {
                let scale = Fr::from(*scale);
                let mut values: Vec<Fr> =
                    phase1[*column].iter().map(|v| beta - scale * v).collect();
                batch_inversion(&mut values);
                values
            })
            .collect();
        let mut columns: Vec<Vec<Fr>> = inverses
            .chunks(2)
            .map(|pair| (0..n).map(|i| pair.iter().map(|column| column[i]).sum()).collect())
            .collect();

        let size = self.table_size();
        let mut table_inverse: Vec<Fr> = (0..n)
            .map(|i| beta - if i < size { Fr::from(i as u64) } else { Fr::zero() })
            .collect();
        batch_inversion(&mut table_inverse);
        let u: Vec<Fr> = phase1[self.mult]
            .iter()
            .zip(&table_inverse)
            .map(|(m, inverse)| *m * inverse)
            .collect();

        let mut sum = Vec::with_capacity(n);
        let mut running = Fr::zero();
        for i in 0..n {
            sum.push(running);
            running += columns.iter().map(|h| h[i]).sum::<Fr>() - u[i];
        }
        columns.push(u);
        columns.push(sum);
        columns
    }

    /// Appends the lookup's constraint terms at one point.
    pub fn terms(&self, cols: &[Fr], sum_next: Fr, table: Fr, beta: Fr, out: &mut Vec<Fr>) {
        let denominator = |entry: &(usize, u64)| beta - Fr::from(entry.1) * cols[entry.0];
        let mut helper_total = Fr::zero();
        for (i, pair) in self.entries.chunks(2).enumerate() {
            let h = cols[self.helper(i)];
            helper_total += h;
            let d1 = denominator(&pair[0]);
            match pair.get(1) {
                Some(second) => {
                    let d2 = denominator(second);
                    out.push(h * d1 * d2 - d1 - d2);
                }
                None => out.push(h * d1 - Fr::one()),
            }
        }
        let u = cols[self.table_helper()];
        out.push(u * (beta - table) - cols[self.mult]);
        out.push(sum_next - cols[self.running_sum()] - (helper_total - u));
    }

    /// `sum_j 2^(w j) * limb_j` for a run of limb values.
    pub fn recompose(&self, limbs: &[Fr]) -> Fr {
        let base = Fr::from(1u64 << self.limb_bits);
        limbs.iter().rev().fold(Fr::zero(), |acc, limb| acc * base + limb)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ark_ff::UniformRand;
    use ark_std::test_rng;

    /// One 8-bit quantity in 3-bit limbs on a domain of size 8: columns 0..3 are the limbs, column 3 the multiplicities.
    fn gadget() -> Lookup {
        Lookup {
            limb_bits: 3,
            entries: range_entries(3, 0, 8),
            mult: 3,
        }
    }

    fn columns(lookup: &Lookup, values: &[u64]) -> Vec<Vec<u64>> {
        let mut columns = lookup.limbs(values, 3);
        let multiplicities = lookup.multiplicities(&columns, values.len());
        columns.push(multiplicities);
        columns
    }

    /// Whether every lookup term vanishes at every point of the domain.
    fn holds(lookup: &Lookup, phase1: &[Vec<u64>], beta: Fr) -> bool {
        let n = phase1[0].len();
        let mut all: Vec<Vec<Fr>> = phase1
            .iter()
            .map(|column| column.iter().map(|v| Fr::from(*v)).collect())
            .collect();
        let helpers = lookup.phase2(beta, &all);
        all.extend(helpers);
        assert_eq!(all.len(), lookup.total());
        (0..n).all(|i| {
            let cols: Vec<Fr> = all.iter().map(|column| column[i]).collect();
            let sum_next = all[lookup.running_sum()][(i + 1) % n];
            let table = if i < lookup.table_size() { Fr::from(i as u64) } else { Fr::zero() };
            let mut terms = Vec::new();
            lookup.terms(&cols, sum_next, table, beta, &mut terms);
            terms.iter().all(|term| term.is_zero())
        })
    }

    #[test]
    fn entries_cover_the_exact_range() {
        // The top limb (2 bits) is looked up again, scaled by 2.
        assert_eq!(range_entries(3, 0, 8), vec![(0, 1), (1, 1), (2, 1), (2, 2)]);
        // A multiple of the limb width needs no second entry.
        assert_eq!(range_entries(8, 5, 16), vec![(5, 1), (6, 1)]);
    }

    #[test]
    fn values_in_range_pass_and_values_out_of_range_fail() {
        let lookup = gadget();
        let beta = Fr::rand(&mut test_rng());
        let honest = columns(&lookup, &[0, 1, 255, 200, 7, 64, 100, 0]);
        assert!(holds(&lookup, &honest, beta));

        // 256 does not fit 8 bits: the scaled top limb is outside the table.
        let too_large = columns(&lookup, &[0, 1, 256, 200, 7, 64, 100, 0]);
        assert!(!holds(&lookup, &too_large, beta));

        // A limb cell outside the table fails whatever the multiplicities.
        let mut bad_limb = honest.clone();
        bad_limb[0][4] = 9;
        assert!(!holds(&lookup, &bad_limb, beta));
        let mut bad_count = honest.clone();
        bad_count[3][0] += 1;
        assert!(!holds(&lookup, &bad_count, beta));
    }
}
