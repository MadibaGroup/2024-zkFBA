
### **Abstract**
Frequent batch auctions (FBAs) have been proposed as an alternative to traditional limit order books for trading securities. The motivation is to mitigate the predatory advantages of high-frequency traders (HFTs). With FBAs, a double-sided auction is held over a short interval (e.g., 1 second). All marketable orders submitted during the time window are executed at the same price, and arrival time is not a factor. FBAs are significantly less transparent than continuous-time orderbooks and rely on fully trusted specialists or exchanges to execute orders at the fairest price. In this research, we apply the cryptographic concept of zero-knowledge proofs (ZKPs) to develop a zk-FBA which enables the specialist to prove trades are executed fairly without revealing any of the orders directly. Our zk-FBA is implemented using modern ZKP techniques: as a custom zk-SNARK.


## Introduction (The Mechanical Constant of the HFT Arms Race)


The efficiency of modern financial markets is often characterized by the speed at which information is incorporated into prices. [1] However, the predominant market design, the continuous limit order book (CLOB), has introduced a structural flaw: the sniping of stale quotes [2]. In a CLOB, time is treated as a continuous variable, and orders are processed serially [1]. This creates a socially wasteful arms race where high-frequency traders (HFTs) compete to capture arbitrage rents from public information that is symmetrically observable to all participants [3]. Empirical evidence suggests that these latency-arbitrage races occur approximately once per minute for many symbols and account for a remarkably large portion (up to 20%) of overall trading volume[1].  Because the continuous design rewards the first party to react to a signal, firms are compelled to invest heavily in microwave links and transoceanic cables [1].  Recent research estimates that the size of the prize in this arms race is approximately $5 billion per year in global equities [1].  This expenditure does not improve price discovery; it merely redistributes wealth from fundamental investors to the fastest intermediaries through wider spreads and reduced market depth [4].  Frequent batch auctions (FBAs) offer a structural remedy by moving from continuous to discrete time [2].

## 2. Frequent Batch Auctions and Market Clearing


An FBA is a uniform-price, sealed-bid double auction conducted at frequent but discrete intervals, such as every 100 milliseconds [2]. By batching orders that arrive within the same interval, the FBA eliminates the outsized importance of microsecond speed advantages. If multiple participants observe the same news, they must compete on price rather than arrival time, thereby restoring the focus to fundamental valuation [2]. 



*Algorithm for Finding The Clearing Price:*

The objective of the auction is to identify the market-clearing price ($P^*$) that maximizes the volume of executed trades. The process involves aggregating bids and asks into Cumulative Demand and Supply Arrays. Demand Depth is the total quantity participants are willing to buy at or above a given price, and Supply Depth is the total quantity participants are willing to sell at or below a given price. At each price tick, the cleared volume is defined as the minimum of the cumulative supply and demand, forming a Minimum Array. The auctioneer identifies the global maximum of this Minimum Array to establish the clearing volume. 

*Tie-Breaking: A Design Choice:*

In many liquid markets, the maximum execution volume occurs across a range of prices rather than at a single price, creating a Volume Plateau. Identifying a specific price within this range requires a tie-breaking rule. It is critical to recognize that tie-breaking is a design choice, not uniquely dictated by economic theory [5]. Different rules, such as pro-rata allocation on the margin or random selection, reflect different market philosophies and can impact participant incentives [6]. Our research adopts the Surplus Minimization rule [10]. This mechanism identifies the price within the plateau at which the absolute difference (imbalance) between supply and demand is at its global minimum, a point in the Surplus Valley. The Plateau and the Valley are shown in Fig. 1, which is based on synthetic market data. This approach provides an economically intuitive clearing point that minimizes unfulfilled interest while maximizing trades [9].


## 3. The Research Gap: Verifiability in the Decentralization Era

Despite the economic advantages of FBAs, a significant research gap exists regarding the verifiability of auction integrity in opaque environments. Early foundational work on decentralizing financial infrastructure, most notably by Clark et al. (2014) [7], established the feasibility of using distributed ledgers to maintain order books and prediction market logs. While they successfully addressed concerns regarding censorship resistance and availability, their model, and much of the related literature [8], assumed a fundamental trade-off between transparency and privacy. In practice, the transition from a transparent CLOB to a sealed-bid FBA introduces a Transparency Paradox [15]. To prevent last-look arbitrage, orders must remain confidential until the auction clears [12]. This opacity creates a vulnerability where a malicious auctioneer could under-match orders to favor certain participants or manipulate the clearing price [12]. Current regulatory frameworks rely on reactive, disclosure-based auditing, which is often insufficient for high-frequency environments where historical records can be obfuscated. There is a critical need for a protocol that provides proactive, mathematically guaranteed fair play without requiring the disclosure of sensitive order data [8].



## 4. Zero-Knowledge Proofs: Practical Cryptographic Integrity

Zero-knowledge proofs (ZKPs), allow a "prover" to convince a "verifier" that a statement is true without revealing any secret inputs [11]. Modern iterations, known as zk-SNARKs (Succinct Non-Interactive Arguments of Knowledge), possess attributes essential for financial infrastructure [13]:

*Zero-Knowledge:* No private input, such as an order price or size, is exposed during verification.

*Succinctness:* The proof is small (often $\approx 1$ KB) and can be verified near-instantaneously, regardless of the number of orders.

*Knowledge Soundness:* It is computationally impossible for a prover to generate a valid proof for a false statement [13].


Our protocol leverages PLONK (Permutations over Lagrange-bases for Oecumenical Noninteractive arguments of Knowledge), a type of zk-SNARK proof system [14]. PLONK provides a Universal Trusted Setup, allowing a single ceremony to generate parameters that support any circuit up to a certain size bound. This flexibility is vital for dynamic financial markets where auction parameters and asset classes may change frequently [14].


## 5. Protocol Specification: An Off-Chain Verifiable FBA

Zeequent implements the FBA matching process to maintain the required low-latency performance. The specialist computes the clearing price on private infrastructure and then generates a zk-SNARK proof of correctness. The protocol represents the order book as arrays (prices, bids, asks, and depths) interpolated into polynomials over an evaluation domain $H = \{1, \omega, \dots, \omega^{n-1}\}$. 

*Accumulator and Range Constraints:*

To prevent the specialist from inventing volume or entering negative orders, the protocol first performs range checks. Using a Half-Field Range Check (see PLONKbook?), the protocol ensures that all values lie in the first half of the modular interval $[0, (q-1)/2]$, thereby guaranteeing that all inputs are positive. Supply and demand accumulators are verified via recursive summation vanishing equations, where Supply Sum ($V_{Acc_A,2}$) is $Acc_A(\omega X) = Acc_A(X) + Ask(X)$ and Demand Sum ($V_{Acc_B,2}$) is $Acc_B(X) = Acc_B(\omega X) + Bid(X)$.

*Maximum Volume and Plateau Isolation:*

The system proves the matched volume at any tick is the lesser of supply and demand using a mutual exclusivity constraint:

$$V_{Plateau}(X) = (Acc_A(X) - Min(X)) \cdot (Acc_B(X) - Min(X)) = 0$$

To prove that the matched volume $V_{max}$ is the global maximum, the protocol uses bit-decomposition to show that $(V_{max} - Min(X)) \geqslant 0$. The Volume Plateau $[c, d]$ is locked by proving the volume is exactly $V_{max}$ inside this range and strictly lower outside, enforced by Cliff Proofs that require a drop of at least one unit at the boundaries.


<img width="963" height="437" alt="Screenshot 2026-04-27 at 10 11 56 PM" src="https://github.com/user-attachments/assets/fecc9876-2c9f-4a37-81a6-48a319dcf376" />









*Verifiable Tie-Breaking via the Valley Proof:*

Once the plateau is established, the protocol identifies the unique clearing price by minimizing market surplus. The surplus is the absolute difference between supply and demand. The Valley Proof is obtained using the same method but in reverse, demonstrating that the chosen price corresponds to the global minimum of this surplus valley within the plateau interval. This provides a verifiable, non-arbitrary tie-breaking mechanism, auditable by any regulator or participant.

## 6. Economic and Regulatory Synthesis

The transition to a zk-FBA model represents a paradigm shift in financial regulation from reactive, disclosure-based auditing to proactive, proof-based verification. In traditional markets, regulators detect abuse by analyzing historical records after the fact [15]. In the Zeequent model, the exchange provides a cryptographic proof of correctness at the time of clearing [16]. This rational privacy allows institutions to trade large blocks without revealing their strategies to predatory algorithms, while providing regulators with a mathematical guarantee that the exchange acted as a neutral intermediary. By solving the transparency paradox, zk-FBAs restore financial markets' focus to price discovery, effectively ending the microsecond arms race [2].





## Refrences:
intro:
1 Quantifying the High-Frequency Trading “Arms Race”

2 The High-Frequency Trading Arms Race: Frequent Batch Auctions as a Market Design Response (OG)

3 Mechanism Design with Information Leakage

4 A Theory of Stock Exchange Competition and Innovation Will the Market Fix the Market

5 Supplementary Appendix to “Strategy-proofness in the Large”

6 Existence of Equilibrium in Auctions and Discontinuous Bayesian Games

7 On Decentralizing Prediction Markets and Order Books

8 Preserving Capital Markets Efficiency in the High-Frequency Trading Era

9 Implementation Details for Frequent Batch Auctions:
Slowing Down Markets to the Blink of an Eye†
By Eric Budish, Peter Cramton, and John Shim*

10 Optimal auction duration: A price formation viewpoint
Paul Jusselin, Thibaut Mastrolia, Mathieu Rosenbaum

11 THE KNOWLEDGE COMPLEXITY OF
INTERACTIVE PROOF SYSTEMS*
SHAFI GOLDWASSER+, SILVIO MICALI+, AND CHARLES RACKOFF (OG)

12 Achieving Trust without Disclosure:
Dark Pools and a Role for Secrecy-Preserving Verification

13 Performance E of zk-SNARK Protocols for Privacy-Preserving Sensor Data Verification: A Systematic Benchmarking Study

14 PLONK: Permutations over Lagrange-bases for Oecumenical Noninteractive arguments of Knowledge (OG)

15 The Transparency Paradox: Why the EU AI Act Cannot Be Enforced Through Existing Supervisory Instruments

16 The Knowledge Complexity of Interactive Proof Systems

--------
-------
# backup material for methodology:

The Zeequent algorithm formalizes the auction matching process into an arithmetic circuit compatible with modern Polynomial Interactive Oracle Proof (PIOP) frameworks. By representing the auction state as a system of polynomials, the protocol enables the off-chain specialist to prove that the clearing outcome is mathematically correct without revealing individual bids.

**Algebraic Representation: Interpolation over the Domain $H$**

The system represents the auction ecosystem as five distinguished private arrays: prices, bid volumes, ask volumes, demand depth, and supply depth. These arrays are interpolated into polynomials defined over the evaluation domain $H=\{\omega^{0},...,\omega^{n-1}\}$, where $\omega$ is the generator of a multiplicative subgroup in a finite field $\mathbb{F}_{q}$. The backbone of the verification logic is the vanishing polynomial $Z_{H}(X)=X^{n}-1$, which ensures that all enforced constraints evaluate to zero at every point in the domain $H$.

**Transition Logic and Recursive Summations:**

To enforce the economic laws of supply and demand, Zeequent utilizes transition constraints to handle recurrence relations across price ticks without wrap-around contradictions.

Accumulator Initialization: Enforces that the supply starts at the first bid at the lowest price tick and demand starts at the highest price tick. For the supply accumulator $Acc_{A}$, initialization is verified via: $V_{Acc\_A,1}(X)=(Acc_{A}(X)-Arr_{A}(X))\cdot\frac{Z_{H}(X)}{X-\omega^{n-1}}=0$.

Recursive Summations: Proves that the cumulative depth is the sum of previous volumes and current orders. The demand sum recurrence, $Acc_{B}(X)=Acc_{B}(\omega X)+Bid(X)$, is enforced as a vanishing equation over the truncated domain.


**Volume Plateau and Cliff Proofs:**

The protocol proves that the clearing volume $V_{max}$ is the global maximum of the executable volume (the minimum of supply and demand at each tick).

Mutual Exclusivity: The system proves that the cleared volume $Min(X)$ at any price tick is exactly equal to either the supply or the demand using the constraint:

$V_{Plateau}(X)=(Acc_{A}(X)-Min(X))\cdot(Acc_{B}(X)-Min(X))=0$.

Plateau Isolation: To lock the interval $[c, d]$ where the maximum volume occurs, the prover uses Cliff vanishing equations. These prove that at $c-1$ and $d+1$, the volume is strictly less than $V_{max}$ by at least 1 unit, using bit-decomposition and slack variables to verify the inequality in zero-knowledge.


**The Valley Proof and Tie-Breaking**

The tie-breaker logic identifies the optimal clearing price within the volume plateau by minimizing market imbalance. The protocol defines the surplus polynomial as the absolute difference between cumulative supply and demand:
$V_{surp}(X)=Surplus(X)-(Acc_{A}(X)-Acc_{B}(X))=0$.

The Valley Proof then demonstrates that the clearing price corresponds to the global minimum (the floor) of this surplus vector within the plateau. This ensures a verifiable, non-arbitrary clearing point.


**Constraint Batching and Probabilistic Verification**

To maintain efficiency, the numerous vanishing equations ($V_{i}$) are batched into a single provable statement. The prover creates a combination of all constraints using powers of a verifier-provided challenge $\alpha$:
$Batch(\alpha)=\sum_{i=1}^{m}\alpha^{i}(V_{i}(X)-Q_{i}(X)Z_{H}(X))$.

The verifier then performs an algebraic check at a random evaluation point $\zeta \notin H$ to confirm that the entire system evaluates to zero, providing a high-probability guarantee that all auction rules were followed.


-------
------



# notes to self:

-------

----------------------------

###### *Points to consider for the finance-heavy approach:???????There is no space laft*

Current frequent batch auctions require traders to take the exchange's word for it that the clearing price was calculated correctly and that the auction wasn't front-run by the exchange operator (how ZKPs shift the burden of trust from people/institutions to mathematics). 

Regulatory alignment (compliance by design): Financial regulators struggle to audit black-box algorithms inside high-frequency trading platforms (Regulatory Technology)?

The Latency Budget (If a batch auction occurs every 100ms, how much of that is available for proof generation? Describing how our implementation allows for modularity or potential parallelization (the engineering story rather than the math).



###### Outline (too much?):

1. Introduction: defining the trust gap. Trust the operator’s clearing logic (!), institutional participation (?)
2. System Model: visual diagram: participants -> order submission -> (black box ZK) -> public clearing Price.
3. The Value Proposition (too far from the actual scope?): Market Fairness. Contrast with dark pool opaque clearing (?) processes
4. Performance & Feasibility (would they even care?) Is this fast enough for real-time trading? (latency, throughput)
5. Regulatory & Adoption: helping compliance? (Automated auditing, proof of fairness to regulators)



](https://web.stanford.edu/~jacksonm/exist32.pdf)

](https://eduardomazevedo.github.io/papers/azevedo-budish-spl-supplementary-material.pdf)


---------------------
