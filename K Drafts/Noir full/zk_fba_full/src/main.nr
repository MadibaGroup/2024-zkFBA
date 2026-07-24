// ZK-FBA  --  Noir circuit for Frequent Batch Auction arithmetic
// Author  :  Kimia Esmaili, Concordia University
//
// Noir translation of the hand-rolled arkworks/BN254 KZG polynomial proof
// in ../zk_fba.  This file is a thin orchestration shell: the two
// constraint families live in separate modules that can also be compiled
// and timed in isolation via acc_circuit/ and mcv_circuit/.
//
//   fba_accumulators.nr  --  C1-C4: ask/bid depth correctness
//   fba_mcv.nr           --  C5 + MCV: min-exclusivity + global maximum
//
// The Barretenberg backend (bb) handles automatically:
//   * ACIR -> UltraHonk constraint system compilation
//   * KZG polynomial commitments  (Ignition ceremony SRS)
//   * Fiat-Shamir transcript hashing  (Poseidon-based)
//   * KZG opening proofs
//   * Pairing verification
//
// Auction data: 21 price ticks, index 0..20 -> price 0, 10, 20, ..., 110
//   BID_RAW = [100,100,100,200,200,500,1000,1500,2000,700,
//              100,100,100,0,0,0,500,500,1000,1000,2000]
//   ASK_RAW = [0,20,30,50,100,200,400,700,1000,1500,1000,
//              0,0,0,0,0,100,100,200,200,200]

mod fba_accumulators;
mod fba_mcv;

fn main(
    // Private witness (hidden from verifier)
    bid:     [u64; 21],  // raw bid volume at each price tick
    ask:     [u64; 21],  // raw ask volume at each price tick
    acc_b:   [u64; 21],  // bid depth  = backward cumulative sum of bid
    acc_a:   [u64; 21],  // ask depth  = forward  cumulative sum of ask
    min_arr: [u64; 21],  // min(acc_b[i], acc_a[i]) -- MCV candidate at tick i

    // Public output (known to verifier)
    mcv: pub u64,        // Market Clearing Volume = max(min_arr)
) {
    // C1-C4: accumulator recurrences (ask depth + bid depth)
    fba_accumulators::check_accumulators(bid, ask, acc_b, acc_a);

    // C5 + MCV: min-exclusivity and global maximum
    fba_mcv::check_mcv(acc_a, acc_b, min_arr, mcv);
}
