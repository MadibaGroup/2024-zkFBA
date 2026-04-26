
### **Abstract**
Frequent batch auctions (FBAs) have been proposed as an alternative to traditional limit order books for trading securities. The motivation is to mitigate the predatory advantages of high-frequency traders (HFTs). With FBAs, a double-sided auction is held over a short interval (e.g., 1 second). All marketable orders submitted during the time window are executed at the same price, and arrival time is not a factor. FBAs are significantly less transparent than continuous-time orderbooks and rely on fully trusted specialists or exchanges to execute orders at the fairest price. In this research, we apply the cryptographic concept of zero-knowledge proofs (ZKPs) to develop a zk-FBA which enables the specialist to prove trades are executed fairly without revealing any of the orders directly. Our zk-FBA is implemented using modern ZKP techniques: as a custom zk-SNARK.

----------------------------------

## Introduction (The Mechanical Constant of the HFT Arms Race)


The efficiency of modern financial markets is often characterized by the speed at which information is incorporated into prices. [1] However, the predominant market design, the continuous limit order book (CLOB), has introduced a structural flaw: the "sniping" of stale quotes [2]. In a CLOB, time is treated as a continuous variable, and orders are processed serially [1]. This creates a socially wasteful arms race where high-frequency traders (HFTs) compete to capture arbitrage rents from public information that is symmetrically observable to all participants [3]. Empirical evidence suggests that these latency-arbitrage races occur approximately once per minute for many symbols and account for a remarkably large portion (up to 20%) of overall trading volume[1].  Because the continuous design rewards the first party to react to a signal, firms are compelled to invest heavily in microwave links and trans-oceanic cables[1].  Recent research estimates that the size of the prize in this arms race is approximately $5 billion per year in global equities [1].  This expenditure does not improve price discovery; it merely redistributes wealth from fundamental investors to the fastest intermediaries through wider spreads and reduced market depth [4].  Frequent batch auctions (FBAs) offer a structural remedy by moving from continuous to discrete time [2].

## 2. Frequent Batch Auctions and Market Clearing


An FBA is a uniform-price, sealed-bid double auction conducted at frequent but discrete intervals, such as every 100 milliseconds [2]. By batching orders that arrive within the same interval, the FBA eliminates the outsized importance of microsecond speed advantages. If multiple participants observe the same news, they must compete on price rather than arrival time, thereby restoring the focus to fundamental valuation [2]. 


(?check ref again? should I just ref to the OG Budish paper or should I ref to the "derived" papers directly?)


-------------------------

### 2.1 The Clearing Price Algorithm *(CHECK for literature)*

The objective of the auction is to identify the market-clearing price ($P^*$) that maximizes the volume of executed trades. The process involves aggregating bids and asks into Cumulative Demand and Supply Arrays. Demand Depth is the total quantity participants are willing to buy at or above a given price and Supply Depth is the total quantity participants are willing to sell at or below a given price.

At each price tick, the cleared volume is defined as the minimum of the cumulative supply and demand, forming a Minimum Array. The auctioneer identifies
the global maximum of this Minimum Array to establish the clearing volume. 

(?check for ref? explain the algorithm in detail)

### 2.2 Tie-Breaking: A Design Choice

In many liquid markets, the maximum execution volume exists across a range of prices rather than a single point, creating a Volume Plateau. Identifying a specific price within this range requires a tie-breaking rule. It is critical to recognize that tie-breaking is a design choice and is not uniquely dictated by economic theory [5]. Different rules, such as pro-rata allocation on the margin or random selection, reflect different market philosophies and can impact participant incentives [6].

The Zeequent protocol adopts the Surplus Minimization rule. This mechanism identifies the price within the plateau where the absolute difference (imbalance) between supply and demand is at its global minimum, a point described as the Surplus Valley. The Plateau and the Valley can be seen in Fig. 1, made from synthetic market data. This approach provides an economically intuitive clearing point that minimizes unfulfilled interest while maximizing trades. (?check for ref?)

----------------
## 3. The Research Gap: Verifiability in the Decentralization Era

Despite the economic advantages of FBAs, a significant research gap exists regarding the verifiability of auction integrity in opaque environments. Early foundational work on decentralizing financial infrastructure, most notably by Clark et al. (2014) [7], established the feasibility of utilizing distributed ledgers for maintaining order books and prediction market logs. While Clark et al. successfully addressed concerns regarding censorship resistance and availability, their model, and much of the subsequent literature on FBAs [8], assumed a fundamental trade-off between transparency and privacy. 

(for anonymity reasons, is it ok to mention the paper explicitly? or is it a dead giveaway lol)


In practice, the transition from a transparent CLOB to a sealed-bid FBA introduces a Transparency Paradox. To prevent last-look arbitrage, orders must remain confidential until the auction clears []. This opacity creates a vulnerability where a malicious auctioneer could under-match orders to favor certain participants or manipulate the clearing price []. Current regulatory frameworks rely on reactive, disclosure-based auditing, which is often insufficient for high-frequency environments where historical records can be obfuscated []. There is a critical need for a protocol that provides proactive, mathematical certainty of fair play without requiring the disclosure of sensitive order data or the public exposure of the underlying order book [].



## 4. Zero-Knowledge Proofs: Practical Cryptographic Integrity

Zero-knowledge proofs (ZKPs), conceptualized by Goldwasser, Micali, and Rackoff (1989), allow a "prover" to convince a "verifier" that a statement is true without revealing any secret inputs []. Modern iterations, known as zk-SNARKs (Succinct Non-Interactive Arguments of Knowledge), possess attributes essential for financial infrastructure []:

**Zero-Knowledge:** No private input, such as an order price or size, is exposed during verification [].
**Succinctness:** The proof is small (often $\approx 1$ KB) and can be verified near-instantaneously, regardless of the number of orders [].
**Knowledge Soundness:** It is computationally impossible for a prover to generate a valid proof for a false statement [].


Our protocol leverages the PLONK (Permutations over Lagrange-bases for Oecumenical Noninteractive arguments of Knowledge), type of zk-SNARK proof system []. PLONK provides a "Universal Trusted Setup," allowing a single ceremony to generate parameters that support any circuit up to a certain size bound []. This flexibility is vital for dynamic financial markets where auction parameters and asset classes may change frequently.

----------------------------
**(just explained, needs citation for other methods)** 

## 5. Protocol Specification: An Off-Chain Verifiable FBA

Zeequent implements the FBA matching process off-chain to maintain the low-latency performance required for modern finance []. The specialist computes the clearing price on private infrastructure and then generates a zk-SNARK proof of correctness. 2 The protocol represents the order book as arrays (prices, bids, asks, and depths) interpolated into polynomials over an evaluation domain $H = \{1, \omega, \dots, \omega^{n-1}\}$ []. 

### 5.1 Accumulator and Range Constraints

To prevent the specialist from "inventing" volume or entering negative orders, the protocol first performs range checks. Using a Half-Field Range Check (reference to PLONKbook?), the protocol ensures all values lie in the first half of the modular interval $[0, (q-1)/2]$, mathematically guaranteeing that all inputs are positive. Supply and demand accumulators are verified via recursive summation vanishing equations :

**Supply Sum ($V_{Acc_A,2}$):** $Acc_A(\omega X) = Acc_A(X) + Ask(X)$.

**Demand Sum ($V_{Acc_B,2}$):** $Acc_B(X) = Acc_B(\omega X) + Bid(X)$.

### 5.2 Maximum Volume and Plateau Isolation

The system proves the matched volume at any tick is the lesser of supply and demand using a mutual exclusivity constraint:

$$V_{Plateau}(X) = (Acc_A(X) - Min(X)) \cdot (Acc_B(X) - Min(X)) = 0$$

To prove that the matched volume $V_{max}$ is the global maximum, the protocol uses bit-decomposition to show the difference $(V_{max} - Min(X))$ is non-negative. The "Volume Plateau" $[c, d]$ is locked by proving the volume is exactly $V_{max}$ inside the range and strictly lower outside, enforced by "Cliff" proofs that require a drop of at least one unit at the boundaries.



![[Screenshot from 2026-04-25 21-17-04.png]]
<img width="954" height="516" alt="market" src="https://github.com/user-attachments/assets/aebab79b-7ce4-46fe-b71f-d252814ca12f" />








### 5.3 Verifiable Tie-Breaking via the Valley Proof

Once the plateau is established, the protocol identifies the unique clearing price by minimizing market surplus. The surplus is formalized as the absolute difference between supply and demand. The "Valley Proof" is obtained with the same method but the reverse process, demonstraing that the chosen price corresponds to the global minimum of this surplus valley within the plateau interval. This provides a verifiable, non-arbitrary tie-breaking mechanism that can be audited by any regulator or participant.

## 6. Economic and Regulatory Synthesis

The transition to an off-chain zk-FBA model represents a paradigm shift in financial regulation from reactive, disclosure-based auditing to proactive, proof-based verification. In traditional markets, regulators detect abuse by analyzing historical records after the fact []. In the Zeequent model, the exchange provides a cryptographic certificate of correctness at the time of clearing. This rational privacy allows institutions to trade large blocks without revealing their strategies to predatory algorithms, while simultaneously providing regulators with mathematical proof that the exchange acted as a neutral intermediary. By solving the transparency paradox, zk-FBAs restore the focus of financial markets to price discovery and fundamental valuation, effectively ending the microsecond arms race [2].




----------------------







----------------------------

###### *Points to consider for the finance-heavy approach:*

Current frequent batch auctions require traders to take the exchange's word for it that the clearing price was calculated correctly and that the auction wasn't front-run by the exchange operator (how ZKPs shift the burden of trust from people/institutions to mathematics). 

Regulatory alignment (compliance by design): Financial regulators struggle to audit black-box algorithms inside high-frequency trading platforms (Regulatory Technology)?

The Latency Budget (If a batch auction occurs every 100ms, how much of that is available for proof generation? Describing how our implementation allows for modularity or potential parallelization (the engineering story rather than the math).



###### Outline:

1. Introduction: defining the trust gap. Trust the operator’s clearing logic (!), institutional participation (?)
2. System Model: visual diagram: participants -> order submission -> (black box ZK) -> public clearing Price.
3. The Value Proposition (too far from the actual scope?): Market Fairness. Contrast with dark pool opaque clearing (?) processes
4. Performance & Feasibility (would they even care?) Is this fast enough for real-time trading? (latency, throughput)
5. Regulatory & Adoption: helping compliance? (Automated auditing, proof of fairness to regulators)

---------------------
## Refrences:
intro:
1 Quantifying the High-Frequency Trading “Arms Race”

2 The High-Frequency Trading Arms Race: Frequent Batch Auctions as a Market Design Response 

3 Mechanism Design with Information Leakage

4 A Theory of Stock Exchange Competition and Innovation Will the Market Fix the Market

5 Supplementary Appendix to “Strategy-proofness in the Large”

6 Existence of Equilibrium in Auctions and Discontinuous Bayesian Games

7 On Decentralizing Prediction Markets and Order Books

8 Preserving Capital Markets Efficiency in the High-Frequency Trading Era



](https://web.stanford.edu/~jacksonm/exist32.pdf)





](https://eduardomazevedo.github.io/papers/azevedo-budish-spl-supplementary-material.pdf)
