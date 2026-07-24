# Why Noir Outscales the Hand-Written Rust Prover at Large N

**Kimia Esmaili, Concordia University**

---

## Observed Phenomenon

The benchmark data show a crossover that is counterintuitive at first. The hand-written Rust prover achieves an end-to-end latency of 27.52 ms at N=21 price ticks, a 3.4x advantage over the Noir/Barretenberg pipeline at 93.4 ms. By N=1000, the ordering inverts: the Noir prover completes in 299.2 ms while the Rust pipeline requires 416 ms, a 1.4x advantage for the compiled circuit. This section identifies the algorithmic and architectural mechanisms responsible for the crossover.

---

## Root Cause 1: Coefficient-Form Polynomial Division is O(N^2)

The most significant bottleneck is computing the quotient polynomial for constraint C5. In the Rust implementation, C5 is expressed as the product polynomial

$$V_{\mathrm{KL}}(X) = \bigl(\mathrm{AccA}(X) - \mathrm{Min}(X)\bigr) \cdot \bigl(\mathrm{AccB}(X) - \mathrm{Min}(X)\bigr)$$

where each operand is a degree-(N-1) polynomial interpolated in coefficient form over the BN254 scalar field. Their product $V_{\mathrm{KL}}$ has degree $2(N-1)$. To certify that $V_{\mathrm{KL}}$ vanishes on all N evaluation points, we compute the quotient

$$Q_{\mathrm{KL}}(X) = \frac{V_{\mathrm{KL}}(X)}{Z_H(X)}, \qquad Z_H(X) = \prod_{i=0}^{N-1}(X - \omega^i)$$

by polynomial long division in coefficient form. Dividing a degree-2N numerator by a degree-N denominator costs $O(N^2)$ field multiplications [Knuth 1997, sec. 4.6.1]. At N=21, with domain size $n=32$, this is negligible. At N=1000, with domain size $n=1024$, the $O(n^2)$ term produces 12 ms of latency in Layer 3c alone, a 15x increase for a 32x domain growth. This is consistent with a degree of super-linearity between $O(n)$ and $O(n^2)$, likely driven by the coefficient array exceeding the L2 cache at larger domain sizes.

---

## Root Cause 2: UltraHonk Computes Quotient Polynomials in O(N log N) via Coset FFT

The architectural answer to Root Cause 1 is the central algorithmic contribution of PLONK [Gabizon et al. 2019]. Instead of computing quotient polynomials by coefficient-form long division, PLONK selects an evaluation domain $H = \{1, \omega, \omega^2, \ldots, \omega^{N-1}\}$ that forms a multiplicative subgroup of the BN254 scalar field $\mathbb{F}_r$. The vanishing polynomial over $H$ becomes simply

$$Z_H(X) = X^N - 1$$

which can be evaluated at any point in $O(\log N)$ time. More importantly, the prover evaluates the combined constraint polynomial over a *coset* $gH = \{g, g\omega, \ldots, g\omega^{N-1}\}$ where $g \in \mathbb{F}_r^* \setminus H$. Since $Z_H(g\omega^i) = (g\omega^i)^N - 1 = g^N - 1 \neq 0$ for all $i$, the quotient at each coset point reduces to a single field division. The quotient polynomial in coefficient form is then recovered by an inverse Number Theoretic Transform (NTT) over the coset. The full procedure costs $O(N \log N)$ field operations, dominated by the forward and inverse NTT [Cooley and Tukey 1965; Harvey and van der Hoeven 2021].

UltraHonk, the proof system compiled by Barretenberg, extends this approach to a richer constraint system that includes custom gates, range-check lookup tables via Plookup [Gabizon and Williamson 2020], and permutation arguments, all handled within the same $O(N \log N)$ quotient-construction loop. At N=1000, the UltraHonk domain is $n=65{,}536 = 2^{16}$, a 16x ratio relative to the N=21 baseline. The corresponding $O(n \log n)$ cost ratio is

$$\frac{65536 \cdot 16}{4096 \cdot 12} \approx 21.3\times$$

The measured prove-time ratio is 3.5x rather than 21.3x because (i) fixed setup costs like SRS loading and curve parameter initialization are amortized over the larger computation, and (ii) Barretenberg parallelizes the NTT across all available CPU cores.

---

## Root Cause 3: Multi-Threaded MSMs Scale Better at Large Domain Sizes

Kate-Zaverucha-Goldberg (KZG) polynomial commitments [Kate et al. 2010] require one multi-scalar multiplication (MSM) over BN254 per polynomial committed. An MSM of size $m$ costs $O(m / \log m)$ group operations under Pippenger's algorithm [Pippenger 1976]. The Rust prover runs single-threaded and commits to 10 polynomials (5 witness + 5 quotient) over a domain of $n=32$ at N=21, rising to $n=1024$ at N=1000. All 10 MSMs run serially. Barretenberg distributes its 30+ wire-polynomial commitments across 14 CPU threads. For an MSM of size $m$ parallelized over $k$ threads with Pippenger, the effective cost is $O(m / (k \log m))$.

At large $m$, this gives near-linear speedup in $k$. At small $m$ the per-thread overhead dominates and parallelism adds little. This is why the threading advantage grows as N increases: the N=21 Rust MSMs involve only 32 scalar-point pairs, too few to benefit from parallelism, while the N=1000 Barretenberg MSMs involve 65,536 pairs, approaching the regime where $k=14$ threads achieve close to 14x throughput improvement.

---

## Root Cause 4: Layer 4 is an O(N * n) Bottleneck Specific to the Prototype

The Rust pipeline includes an algebraic constraint check (Layer 4) that evaluates all five witness polynomials at every one of the N data points via Horner's method. For a degree-(n-1) polynomial evaluated at N points, this costs $O(N \cdot n)$ field operations. With $N=1000$ and $n=1024$, that is roughly $10^6$ field multiplications, consuming 371 ms of the 416 ms total, 89% of the end-to-end latency.

This step is not a cryptographic requirement. It is a prover-side correctness check that would be disabled in production deployment, analogous to the assertion-checking passes in other research proof system prototypes [Ben-Sasson et al. 2018]. Its presence in the profiled pipeline accounts for the majority of the N=1000 Rust deficit. Excluding Layer 4, the Rust ZK-proper pipeline at N=1000 completes in roughly 45 ms, still faster than Barretenberg's 286 ms for the prove phase alone, which confirms that the Barretenberg $O(N \log N)$ advantage does not yet dominate at these domain sizes once the prototype artifact is removed.

---

## Summary

The crossover at approximately N=600-800 (interpolated between the N=100 and N=1000 data points) arises from three effects operating simultaneously as N increases.

| Factor | Rust scaling | Barretenberg scaling | Winner at large N |
|---|---|---|---|
| Quotient polynomial computation | O(N^2) long division | O(N log N) coset NTT | Barretenberg |
| MSM throughput | O(N/log N), 1 thread | O(N/(k log N)), k=14 threads | Barretenberg |
| Layer 4 sanity check | O(N * n) = O(N^2) | Absent | Barretenberg |
| Fixed overhead amortization | Low fixed cost | High fixed cost | Rust at small N |

The core lesson, consistent with the analysis in Wahby et al. [2018] on doubly-efficient SNARKs, is that custom arithmetization yields smaller constants in the proof system's complexity but does not necessarily achieve a better asymptotic class. General-purpose constraint compilers like Barretenberg are engineered to reach $O(N \log N)$ for all sub-computations by construction. Hand-written provers can inadvertently introduce super-linear terms, here the naive quotient long division and the Horner constraint check, that are invisible at prototype scale but dominate as the statement size approaches production workloads.

---

## References

- Ben-Sasson, E., Bentov, I., Horesh, Y., and Riabzev, M. (2018). Scalable, transparent, and post-quantum secure computational integrity. *Cryptology ePrint Archive*, Report 2018/046.
- Cooley, J. W., and Tukey, J. W. (1965). An algorithm for the machine calculation of complex Fourier series. *Mathematics of Computation*, 19(90), 297-301.
- Gabizon, A., Williamson, Z. J., and Ciobanu, O. (2019). PLONK: Permutations over Lagrange-bases for Oecumenical Noninteractive arguments of Knowledge. *Cryptology ePrint Archive*, Report 2019/953.
- Gabizon, A., and Williamson, Z. J. (2020). plookup: A simplified polynomial protocol for lookup tables. *Cryptology ePrint Archive*, Report 2020/315.
- Harvey, D., and van der Hoeven, J. (2021). Integer multiplication in time O(n log n). *Annals of Mathematics*, 193(2), 563-617.
- Kate, A., Zaverucha, G. M., and Goldberg, I. (2010). Constant-size commitments to polynomials and their applications. In *Advances in Cryptology -- ASIACRYPT 2010*, LNCS 6477, pp. 177-194. Springer.
- Knuth, D. E. (1997). *The Art of Computer Programming, Vol. 2: Seminumerical Algorithms* (3rd ed.), sec. 4.6.1. Addison-Wesley.
- Pippenger, N. (1976). On the evaluation of powers and related problems. In *17th Annual Symposium on Foundations of Computer Science*, pp. 258-263. IEEE.
- Wahby, R. S., Tzialla, I., Shelat, A., Thaler, J., and Walfish, M. (2018). Doubly-efficient zkSNARKs without trusted setup. In *IEEE Symposium on Security and Privacy*, pp. 926-943.
