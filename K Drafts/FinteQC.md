
### **Abstract**
Frequent batch auctions (FBAs) have been proposed as an alternative to traditional limit order books for trading securities. The motivation is to mitigate the predatory advantages of high-frequency traders (HFTs). With FBAs, a double-sided auction is held over a short interval (e.g., 1 second). All marketable orders submitted during the time window are executed at the same price, and arrival time is not a factor. FBAs are significantly less transparent than continuous-time orderbooks and rely on fully trusted specialists or exchanges to execute orders at the fairest price. In this research, we apply the cryptographic concept of zero-knowledge proofs (ZKPs) to develop a zk-FBA which enables the specialist to prove trades are executed fairly without revealing any of the orders directly. Our zk-FBA is implemented using modern ZKP techniques: as a custom zk-SNARK.

----------------------------------
(((((In a market structure such as Frequent Batch Auctions (FBAs), bid and ask orders are aggregated over short and discrete time intervals instead of being continuously processed. To match orders, they use a uniform price, which reduces the predatory advantages of high-frequency traders (HFT) for exploiting the system to get ahead of the rest of the market and causing maximum extractable value (MEV) attacks by front-running, as well as decreasing the arbitrage rents. This results in a more stable market with tighter bid-ask spreads and higher market quality, as well as an enhanced price discovery. 
To ensure these auctions are executed with integrity and fairness, we use Zero-Knowledge SNARKs. These tools provide succinct, cryptographically verifiable proofs, showing computations were performed honestly without revealing the underlying data. 
Unlike traditional continuous order books, FBAs often limit the amount of public data to prevent front-running. This reduces transparency, making it significantly harder for traders to verify if they are truly receiving a fair market-clearing price. 
We present Zeequent, an easily verified, custom ZK-SNARK protocol designed specifically for private call auctions that can issue proofs for finding the clearing price in the market, while maintaining the complete privacy of competitive details of all individual participants' bids.
(parts for intro? Also referencing check)))))))

https://github.com/MadibaGroup/2024-Gadgets-Code/

https://www.nyse.com/nyse-auction-data?symbol=AA&date=02-13-2026
----------------------------------------
## Introduction (The Mechanical Constant of the HFT Arms Race)


The efficiency of modern financial markets is often characterized by the speed at which information is incorporated into prices. [1] However, the predominant market design, the continuous limit order book (CLOB), has introduced a structural flaw: the "sniping" of stale quotes [2]. In a CLOB, time is treated as a continuous variable, and orders are processed serially [1]. This creates a "socially wasteful arms race" where high-frequency traders (HFTs) compete to capture arbitrage rents from public information that is symmetrically observable to all participants [3].

Empirical evidence suggests that these latency-arbitrage races occur approximately once per minute for many symbols and account for a remarkably large portion (up to 20%) of overall trading volume[1].  Because the continuous design rewards the first party to react to a signal, firms are compelled to invest heavily in microwave links and trans-oceanic cables[1].  Recent research estimates that the "size of the prize" in this arms race is approximately $5 billion per year in global equities [1].  This expenditure does not improve price discovery; it merely redistributes wealth from fundamental investors to the fastest intermediaries through wider spreads and reduced market depth [4].  Frequent batch auctions (FBAs) offer a structural remedy by moving from continuous to discrete time [2].

## 2. Frequent Batch Auctions and Market Clearing


An FBA is a uniform-price, sealed-bid double auction conducted at frequent but discrete intervals, such as every 100 milliseconds [2]. By batching orders that arrive within the same interval, the FBA eliminates the outsized importance of microsecond speed advantages. If multiple participants observe the same news, they must compete on price rather than arrival time, thereby restoring the focus to fundamental valuation [2]. (?check ref again?)



### 2.1 The Clearing Price Algorithm

The objective of the auction is to identify the market-clearing price ($P^*$) that maximizes the volume of executed trades. The process involves aggregating bids and asks into cumulative demand and supply curves.

**Demand Depth:** The total quantity participants are willing to buy at or above a given price.
**Supply Depth:** The total quantity participants are willing to sell at or below a given price.

At each price tick, the cleared volume is defined as the minimum of the cumulative supply and demand. The auctioneer identifies the global maximum of this "minimum array" to establish the clearing volume. (?check for ref?)









----------------------


![[Screenshot from 2026-04-25 21-17-04.png]]




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

5 