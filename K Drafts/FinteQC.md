
### **Abstract**
Frequent batch auctions (FBAs) have been proposed as an alternative to traditional limit order books for trading securities. The motivation is to mitigate the predatory advantages of high-frequency traders (HFTs). With FBAs, a double-sided auction is held over a short interval (e.g., 1 second). All marketable orders submitted during the time window are executed at the same price, and arrival time is not a factor. FBAs are significantly less transparent than continuous-time orderbooks and rely on fully trusted specialists or exchanges to execute orders at the fairest price. In this research, we apply the cryptographic concept of zero-knowledge proofs (ZKPs) to develop a zk-FBA which enables the specialist to prove trades are executed fairly without revealing any of the orders directly. Our zk-FBA is implemented using modern ZKP techniques: as a custom zk-SNARK.

----------------------------------
In a market structure such as Frequent Batch Auctions (FBAs), bid and ask orders are aggregated over short and discrete time intervals instead of being continuously processed. To match orders, they use a uniform price, which reduces the predatory advantages of high-frequency traders (HFT) for exploiting the system to get ahead of the rest of the market and causing maximum extractable value (MEV) attacks by front-running, as well as decreasing the arbitrage rents. This results in a more stable market with tighter bid-ask spreads and higher market quality, as well as an enhanced price discovery. 
To ensure these auctions are executed with integrity and fairness, we use Zero-Knowledge SNARKs. These tools provide succinct, cryptographically verifiable proofs, showing computations were performed honestly without revealing the underlying data. 
Unlike traditional continuous order books, FBAs often limit the amount of public data to prevent front-running. This reduces transparency, making it significantly harder for traders to verify if they are truly receiving a fair market-clearing price. 
We present Zeequent, an easily verified, custom ZK-SNARK protocol designed specifically for private call auctions that can issue proofs for finding the clearing price in the market, while maintaining the complete privacy of competitive details of all individual participants' bids.
(parts for intro? Also referencing check)

https://github.com/MadibaGroup/2024-Gadgets-Code/

https://www.nyse.com/nyse-auction-data?symbol=AA&date=02-13-2026
----------------------------------------
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





