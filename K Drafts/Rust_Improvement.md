### Assumption: What are the things that can be done to beat Barretenberg (other than the FFT fix):
---

- More efficient lookup arguments (e.g., improving on Plookup or LogUp trade-offs)
- Faster polynomial commitment schemes
- Better batching strategies for KZG openings
- Improved MSM algorithms (Feasible?)
- Memory-efficient prover architectures for very large circuits
- Specialized compilers that exploit circuit structure to reduce the number of constraints before proving (Idea?)

For example, if our compiler emits 20% fewer constraints than Barretenberg's input for the same computation, 
then every downstream O(nlogn) step also becomes cheaper. 
Reducing n itself is often one of the most effective ways to improve prover performance.

----------
### After FFT fix:

The one thing that's a genuine, if narrow, finding: verify_range16 in the Madiba gadgets crate checks that some value under 2^16 was proven, but never binds it to the specific scalar the caller cares about. That's a real soundness gap in a third-party dependency, not a bug in your the code. verify_range16_bound fixes it, but only as a local wrapper in the codebase.

So, did we solve a problem in the Rust or ZKP ecosystem? No, not in the sense of fixing something broken in arkworks, ark-poly-commit, Noir, or Barretenberg themselves. What got solved was a bug in this codebase (the O(n^2) quotient division, the O(n^2) verify_all, the unbound bit-gadget commitments). The one exception is the gadgets crate range-check binding gap, and that's still parked locally rather than reported upstream.

If there's a real research contribution to point to, it's more likely the protocol itself, and the Rust-vs-Noir comparison is supporting evaluation for that claim, not the claim itself.

### What else can be done to optimize the Rust code

1. Turn on arkworks' own parallel feature: This is the highest-value, lowest-effort thing left: We checked ark-ec 0.4.2, ark-poly 0.4.2, ark-ff 0.4.2, and ark-std 0.4.0 in our local registry cache: all four ship a parallel feature gated on rayon, and it's currently off in Cargo.toml (no features = [...] specified anywhere). We confirmed the MSM code (ark-ec::scalar_mul::variable_base) and the FFT code (ark-poly::domain::radix2::fft) are both behind #[cfg(feature = "parallel")]. That means the exact gap both docs flagged as "the one row Noir still legitimately wins" (single-threaded MSM vs Barretenberg's multi-threaded MSM) has a one-line fix sitting unused: add features = ["parallel"] to those four crates. It would also parallelize every FFT call, meaning Layer 2, the new coset-FFT quotients, and the fixed verify_all all get faster too, not just the MSMs. It's worth doing before spending effort on anything else on this list.

2. Stop rebuilding the coset setup from scratch on every quotient call: Reading through compute_quotients, it calls divide_by_vanishing_coset 9 separate times in a single pass, and each call independently reconstructs the coset domain, recomputes Z_H evaluated at every coset point via exponentiation, and re-runs batch inversion over that vector. Several of these calls share the same padded degree m (the linear-times-something quotients round up to one power of two, the product quotients round up to another), so the coset and inverted Z_H values are identical across several calls and are being recomputed anyway. Memoizing that pair keyed by m would cut redundant domain construction, exponentiation, and inversion out of Layer 3c without changing its complexity class, a constant-factor win on top of the FFT fix, not a replacement for it.

3. Adding a size threshold instead of always using coset-FFT: we already have the data for this: at 21 and 100 ticks the new coset-FFT division is slower than the old coefficient-form long division (FFT setup overhead isn't paid off yet), and it only wins starting somewhere before 1000 ticks. A simple if degree < threshold { poly_div_rem } else { divide_by_vanishing_coset } would recover the small-N losses reported in the optimized doc without giving up the large-N win. Worth picking the threshold empirically rather than guessing as a benchmark sweep.

4. Finishing Phase 3: Not a performance item, a soundness one, but it's still open: the six bit-gadget instances are bound into the Fiat-Shamir challenge now but not yet opened through batch_check. This is still the most important unfinished piece.

5. Measuring real proof size instead of estimating it: Once Phase 3 lands, running CanonicalSerialize on the actual BatchCheckProof/OpenEval types instead of counting elements by hand. The current ~15,264-byte figure is arithmetic, not a measurement.

6. Re-benchmarking both systems at N=5,000 to N=20,000 and real data: With items 1-3 in place, this becomes the real question left in the complexity story: does the MSM-threading gap actually produce a crossover against Noir at realistic-to-large batch sizes, or does it stay academic. Right now that's still an open question, not a measured one, at either size.

