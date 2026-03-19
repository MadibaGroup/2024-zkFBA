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

-----------------------------------------

-----------------------------
The prover proves these are the exact cliffs by demonstrating two contradictory states at adjacent points:

1. At index $c$: The volume is exactly $V_{max}$.

2. At index $c-1$: The volume is strictly less than $V_{max}$.    

If the prover tried to claim $c$ was a "lower" value (meaning they shifted the plateau to the right), the constraint at the _real_ cliff would fail.

### The "Squeeze" Mechanism

Imagine the real plateau is at indices $[5, 10]$.

If the prover lies and says $c=6$, the **Left Cliff** constraint will check index $5$.    

At index $5$, the volume is actually $V_{max}$.    
The equation for the cliff is: $(V_{max} - Min(X) - 1 - Slack) = 0$.    
Plugging in $V_{max}$ at index 5: $(V_{max} - V_{max} - 1 - Slack) \Rightarrow (-1 - Slack) = 0$.    
Since $Slack$ must be $\ge 0$ (enforced by bit-checks ), this equation can **never** be zero. The proof fails.


---


we do not necessarily need a separate vanishing polynomial for every single point, but we do need equations that target them. In PLONK, we use the property that a polynomial $f(X)$ vanishes over $H$ if it is a multiple of $Z_H(X) = X^n - 1$.

To cover the whole range $[c, d]$, you use a **Plateau Mask**:

$$V_{plateau}(X) = (Min(X) - V_{max}) \cdot Mask_{[c,d]}(X) = A(X) \cdot Z_H(X)$$
**$Mask_{[c,d]}(X)$** is a polynomial that is $1$ for all $X \in \{\omega^c, \dots, \omega^d\}$ and $0$ elsewhere.


The Cliff Vanishing Equations: for the exact boundaries, we use the Lagrange polynomials $L_{c-1}(X)$ and $L_{d+1}(X)$:
1. **Left Cliff:** $(V_{max} - Min(X) - 1 - Slack_L(X)) \cdot L_{c-1}(X) = B(X) \cdot Z_H(X)$
2. **Right Cliff:** $(V_{max} - Min(X) - 1 - Slack_R(X)) \cdot L_{d+1}(X) = C(X) \cdot Z_H(X)$

https://blog.zksecurity.xyz/posts/bulletproofs-range-proofs/#:~:text=In%20many%20privacy%2Dpreserving%20systems,it%20makes%20the%20math%20cleaner.

https://www.cs.yale.edu/homes/cpap/published/libra-crypto19.pdf

https://eprint.iacr.org/2022/284.pdf



-------------------------------
The Plateau Mask ($Mask_{[c,d]}$) is a polynomial that is $1$ inside the interval and $0$ outside. You compute it by summing the individual Lagrange Basis Polynomials ($L_i$) for every index in the plateau:$$Mask_{[c,d]}(X) = \sum_{i=c}^{d} L_i(X)$$


The Slack variables ($Slack_L, Slack_R$) are witnesses provided by the prover to satisfy the "strictly less than" requirement.
Find the Difference: The prover looks at the actual volume at the cliff, say $Min(\omega^{c-1})$.
Calculate Gap: They calculate how far that volume is from the "ceiling" ($V_{max} - 1$).
Assign Value: $Slack_L = (V_{max} - 1) - Min(\omega^{c-1})$.
Bit-Decomposition: To prove this $Slack$ is positive (and not a "fake" number caused by field wrap-around), the prover must decompose it into bits $B_j$:
$$Slack(X) = \sum_{j=0}^{k-1} 2^j \cdot B_j(X)$$Each $B_j$ is then constrained to be only $0$ or $1$.