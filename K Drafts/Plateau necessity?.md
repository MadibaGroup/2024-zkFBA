| **Price** | **Bid** | **Ask**  | **Bid Depth (AccB​)** | **Ask Depth (AccA​)** | **Bid Surplus** | **Ask Surplus** | **Trade Vol (Min)** | **MCV**  | **Delta (Δ)** |
| --------- | ------- | -------- | --------------------- | --------------------- | --------------- | --------------- | ------------------- | -------- | ------------- |
| 0         | 100     | 0        | 10500                 | 0                     | 10500           | 0               | 0                   | 4800     | 10500         |
| 10        | 100     | 20       | 10400                 | 20                    | 10380           | 0               | 20                  | 4800     | 10380         |
| 20        | 100     | 30       | 10300                 | 50                    | 10250           | 0               | 50                  | 4800     | 10250         |
| 30        | 200     | 50       | 10200                 | 100                   | 10100           | 0               | 100                 | 4800     | 10100         |
| 40        | 200     | 100      | 10000                 | 200                   | 9800            | 0               | 200                 | 4800     | 9800          |
| 50        | 500     | 200      | 9800                  | 400                   | 9400            | 0               | 400                 | 4800     | 9400          |
| 60        | 1000    | 400      | 9300                  | 800                   | 8500            | 0               | 800                 | 4800     | 8500          |
| 70        | 1500    | 700      | 8300                  | 1500                  | 6800            | 0               | 1500                | 4800     | 6800          |
| 80        | 2000    | 3000     | 6800                  | 4500                  | 2300            | 0               | 4500                | 4800     | 2300          |
| 90        | 0       | 0        | 4800                  | 4500                  | 300             | 0               | 4500                | 4800     | **300**       |
| **100**   | **100** | **1000** | **4800**              | **5500**              | **0**           | **700**         | **4800**            | **4800** | **700**       |
| 101       | 100     | 0        | 4700                  | 5500                  | 0               | 800             | 4700                | 4800     | 800           |
| 102       | 100     | 0        | 4600                  | 5500                  | 0               | 900             | 4600                | 4800     | 900           |
| 103       | 0       | 0        | 4500                  | 5500                  | 0               | 1000            | 4500                | 4800     | 1000          |
| 104       | 0       | 0        | 4500                  | 5500                  | 0               | 1000            | 4500                | 4800     | 1000          |
| 105       | 0       | 0        | 4500                  | 5500                  | 0               | 1000            | 4500                | 4800     | 1000          |
| 106       | 500     | 100      | 4500                  | 5600                  | 0               | 1100            | 4500                | 4800     | 1100          |
| 107       | 0       | 100      | 4000                  | 5700                  | 0               | 1700            | 4000                | 4800     | 1700          |
| 108       | 1000    | 200      | 4000                  | 5900                  | 0               | 1900            | 4000                | 4800     | 1900          |
| 109       | 1000    | 200      | 3000                  | 6100                  | 0               | 3100            | 3000                | 4800     | 3100          |
| 110       | 2000    | 200      | 2000                  | 6300                  | 0               | 4300            | 2000                | 4800     | 4300          |

The decision comes down to a comparison between two significant price points:

**Price 90**, achieving a trade volume of **4,500** with a surplus of **300**.
**Price 100**, achieving a trade volume of **4,800** with a surplus of **700**.

Even though Price 90 has a lower surplus (imbalance), we choose Price 100 because it represents the Global Maximum Volume ($V_{max}$) for the entire order book. Settling at any other price would result in "under-matching," where the auctioneer fails to execute trades that the market is capable of supporting.

**The Factor Priority: Volume vs. Surplus**
In these markets, the factors follow a strict lexicographical priority:
**Priority 1: Trade Volume**: *The primary objective of the auction is to maximize the total quantity of shares traded.* The protocol first identifies the price or range of prices that hits this absolute peak.
**Priority 2: Surplus Valley**: The surplus is strictly a tie-breaker. It is only consulted if multiple prices achieve the exact same $V_{max}$.

**The data shows a "plateau at 4500" but a "peak at 4800".**
**If Surplus had priority**, the market would clear at **Price 90** (the "Valley"), leaving 300 shares unexecuted. Because **Volume** has priority, the protocol bypasses the "cleaner" balance at Price 90 to capture the higher liquidity at **Price 100**. By prioritizing volume, we ensure the auction fulfills its economic purpose of maximum matching before attempting to minimize **the leftover imbalance**.

--------------------------
1. The Problem of "Under-Matching", the most immediate consequence where the auction fails to execute the maximum possible number of shares.

Lost Liquidity: In this data, favoring the lowest Delta (300 at Price 90) instead of the highest volume (4800 at Price 100) results in 300 shares left unexecuted.

Deadweight Loss: Those 300 shares represent buyers and sellers who were willing to trade at a mutually agreeable price but were blocked by the protocol to achieve a "cleaner" balance.

2. Potential for Market Manipulation: rrioritizing Delta creates a loophole that malicious auctioneers can exploit to favor specific participants:

Like selective exclusion where an auctioneer could ignore a high-volume peak that includes a large order they want to "shut out". By settling at a different price with a lower Delta, they can claim they were just "optimizing for balance" while intentionally reducing market access.

Or artificial plateaus: in this example, the data shows sub-optimal "plateaus" at 4500. If Delta was the priority, the auctioneer could settle in these lower-volume ranges simply because the bid/ask spread happened to be narrower there, even if the overall market interest was much higher elsewhere.

-----------------------------
