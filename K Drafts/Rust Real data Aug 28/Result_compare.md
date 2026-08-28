### Cross-Size Comparison
Dataset                ||        	n	        ||          domain	      ||           MCV         ||     	Total ||
21-tick hardcoded	    ||        21	      ||            32	           ||          5,000     ||     	31.4 ms ||
100-tick log-normal	     ||       100	       ||           128	           ||          5,849	     ||       54.0 ms ||
1000-tick log-normal      ||   	1000        ||       	  1,024	            ||         55,940	      ||      416 ms ||


### Key Observations
**The verifier is O(1) — it doesn't change with n:**

Layer 3f (batch_check): always 4 pairings, ~1.9 ms regardless of whether n=21 or n=1000

Layer 3g (range proof verify): always ~1.6 ms

This is the core ZK property: a verifier checks a fixed-size proof, not the raw order book

**Prover cost scales, but sub-linearly:**

KZG setup: 2ms -> 2ms -> 3.6ms (scales with SRS degree = domain_size−1)

Witness commits (5 MSMs): 3.1ms -> 3.5ms -> 6.2ms

Quotient polynomials (Layer 3c): 0.8ms -> 1ms -> 12ms — V_KL = (AccA−Min)·(AccB−Min) involves polynomial multiplication followed by naive long division: O(domain_size^2)

**The bottleneck is Layer 4 (algebraic constraint check), which is NOT ZK:**

4ms -> 31ms -> 371ms — this checks the witness directly by evaluating the polynomial at all n data points (O(n · domain_size) Horner evaluations)

In a real deployment, the prover runs Layer 4 as a sanity check only; the verifier never does this

**MCV range proof stays fixed:**

Layer 3g always proves MCV element of [0, 65536) with a 16-element domain regardless of auction size — the range proof domain is completely decoupled from the FBA domain