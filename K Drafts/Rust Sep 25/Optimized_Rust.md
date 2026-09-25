Here's the head-to-head at the two sizes where Noir has real numbers, using the "core proof" definition already established in RATIONALE_AND_RESULTS.md (KZG setup + witness commits + quotients + quotient commits + Fiat-Shamir + batch open/verify + range proofs — i.e. the actual cryptographic work, excluding the Layer 4/4b self-check layers that Noir has no equivalent of):

Metric	                     Noir, N=100	Rust, N=100	Noir,         N=100       Rust, N=1000
Core proof (prove+verify)	 150ms.  	    59.0ms	                  460ms	      100.8ms
Rust's margin.   	           —	        2.5x faster	              —	          4.6x faster


What changed vs. before my fixes: at N=100, barely anything — the old O(n²) quotient division was cheap at this size anyway (9.6ms even before the fix), so the ratio is basically unchanged (was ~2.8x, now 2.5x, noise-level difference). At N=1000, though, the margin widened substantially: previously Rust's core proof was 141.1ms (3.3x faster than Noir); now it's 100.8ms (4.6x faster) — that ~40ms drop is entirely the coset-FFT quotient fix (Layer 3c: 70.3ms → 26.6ms), which was the single most expensive line item in the old core-proof breakdown.
Complexity picture, which is really the more important answer here: the RATIONALE doc's whole basis for predicting Noir would eventually catch up was that Rust's quotient construction was O(n²) (coefficient-form long division) against UltraHonk's O(n log n) coset-FFT approach — meaning Rust's asymptotic class was worse even though its constant factor was better at N≤1000. That gap is now closed: Rust's quotient construction is O(n log n), matching Noir's own mechanism exactly, not just beating it on constants. Same story for the verify_all bug I found and fixed this session — that was quietly O(n²) too, and is now O(n log n).
The one asymptotic gap that's still real is single-threaded MSM (Rust) vs. Barretenberg's multi-threaded MSM — but that's an engineering gap (both are the same O(m/log m) Pippenger complexity class, just one uses more cores), and per the doc's own reasoning it doesn't start to matter until n climbs into the tens of thousands, well past N=100/1000.
Net effect: at N=100 and N=1000, there's no longer a documented reason to expect Noir to close the gap as N grows toward these values — the specific mechanism that predicted a crossover "somewhere past N=1000" is gone. Whether a crossover still exists at all now depends entirely on the threading gap, which only bites at much larger N than these two measured points — which is exactly why I flagged wanting real Noir numbers at 5000+ to check that, rather than guessing.
Already confirmed twice — no new data since. Quick recap for completeness: the 20000-tick re-run (post verify_all fix) finished with all_pass=true at 5.9s total, down from ~110.7s before that fix (~18.8x improvement), and the 5000/10000-tick points came in at 2.2s/3.4s with the same clean pass.
Where things stand overall (Phases 2–4, all verified):
Coset-FFT quotients (Phase 2) + coset-FFT bit-gadget quotients (Phase 3) + the verify_all O(n²)→O(n log n) fix (bonus find) together took 1000-tick full pipeline from 2363ms → 739ms, and 20000-tick from ~110.7s → 5.9s.
Growth per domain-doubling is now ~1.55–1.74x, matching or beating O(n log n).
ALL PASS: YES at every size, negative control still correctly fails, no protocol/privacy structure changed.
At the two sizes with real Noir numbers (N=100, N=1000), Rust's core-proof margin over Noir widened from ~2.8x/3.3x to ~2.5x/4.6x, and the specific complexity-class gap the RATIONALE doc worried about is now closed.
Next-step options, still open:
Run ~/zk_fba_noir's own benchmark at N=5000/10000/20000 (read-only, no edits) to get a real crossover point — the only way to check whether the remaining single-threaded-MSM gap actually bites at these sizes.
Rewrite RATIONALE_AND_RESULTS.md with today's numbers and the corrected complexity analysis.
Thread the MSM commits with rayon (constant-factor win, addresses the one gap that's still real).
Stop here — this is already a complete, verified, and fairly large piece of work.

