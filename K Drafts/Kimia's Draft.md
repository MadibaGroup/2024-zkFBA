# **Secure Market Clearing via ZK-SNARKs: A Protocol for Private Call Auctions**


### **Abstract**
Frequent batch auctions (FBAs) have been proposed as an alternative to traditional limit order books for trading securities. The motivation is to mitigate the predatory advantages of high-frequency traders (HFTs). With FBAs, a double-sided auction is held over a short interval (e.g., 1 second). All marketable orders submitted during the time window are executed at the same clearing price, and arrival time is not a factor. FBAs are significantly less transparent than continuous-time orderbooks and rely on fully trusted specialists or exchanges to execute orders at the fairest price.  In this research, we present a special-purpose zk-SNARK argument to develop a zk-FBA, which enables the specialist to prove trades are executed fairly with the correct clearing price, without revealing any of the orders directly.


### **1. Introduction**






((Call markets are essential for price discovery, particularly during market opens or periods of high volatility. Traditional auctions require a central authority to view all order data, creating risks for front-running. This protocol allows an auctioneer to commit to an order book and prove the resulting clearing price is correct under the rules of supply and demand without leaking the competitive landscape of the participants.))

### **2. Background and Related work**

**2.1. Frequent Batch Markets Security**


**2.2. Succinct Proofs**


**2.3. The Positive Check**



#### **2.1. The Positive and Range Check**
The Positive Check enables verifiable inequalities in a SNARK circuit. Proving that a value is a maximum is important because it prevents malicious under-matching, as it ensures the auctioneer cannot pick a lower volume to favor certain participants. It also enforces economic validity by guaranteeing the cleared volume does not exceed available supply or demand at any tick. Plateau discovery prevention is another advantage of this check, as it proves the clearing price falls within the range where the highest number of trades can occur.



### **3. Zeequent: Protocol Specification**

#### **Market Setup**



Parameters.

Walkthrough of a constraint.


3.1. Zeequent Constraints (Batch)

3.2. Public Helper Polynomials

3.3. Market Specific Polynomials (?)

3.4. Market Setup Soundness Constraints

3.5. Global Maximum Constrints (min and Finding G Max)

3.5. Global Max check Selector Constraints

3.6. The "Plateau" Constraints (?)

3.7. Global Min check Selector Constraints

3.8. The Tie Breaker Surplus constraints




----------------------------
To better convey how we go about the methodology, we'll use Table 1 as a an example:

Table 1:

| **Price** | **Bids++** | **Asks++** | **Bid Depth** | **Ask Depth** | **MCV** | **Selector** | **In {0, MCV}** | **Bid Surplus** | **Ask Surplus** | **Abs(Delta)** | **Delta Check** | **Selector** | **S*Delta** | **Selector** | **MCV Ref** | **Cliff Value** | **1** | **Slack++** |
| --------- | ---------- | ---------- | ------------- | ------------- | ------- | ------------ | --------------- | --------------- | --------------- | -------------- | --------------- | ------------ | ----------- | ------------ | ----------- | --------------- | ----- | ----------- |
| 0         | 100        | 0          | 11700         | 0             | 0       | 0            | 0               | 11700           | 0               | 11700          | 0               | 0            | 0           | 0            | 5000        | 0               | 0     | 5000        |
| 10        | 100        | 20         | 11600         | 20            | 20      | 0            | 0               | 11580           | 0               | 11580          | 0               | 0            | 0           | 0            | 5000        | 0               | 0     | 5000        |
| 20        | 100        | 30         | 11500         | 50            | 50      | 0            | 0               | 11450           | 0               | 11450          | 0               | 0            | 0           | 0            | 5000        | 0               | 0     | 5000        |
| 30        | 200        | 50         | 11400         | 100           | 100     | 0            | 0               | 11300           | 0               | 11300          | 0               | 0            | 0           | 0            | 5000        | 0               | 0     | 5000        |
| 40        | 200        | 100        | 11200         | 200           | 200     | 0            | 0               | 11000           | 0               | 11000          | 0               | 0            | 0           | 0            | 5000        | 0               | 0     | 5000        |
| 50        | 500        | 200        | 11000         | 400           | 400     | 0            | 0               | 10600           | 0               | 10600          | 0               | 0            | 0           | 0            | 5000        | 0               | 0     | 5000        |
| 60        | 1000       | 400        | 10500         | 800           | 800     | 0            | 0               | 9700            | 0               | 9700           | 0               | 0            | 0           | 0            | 5000        | 0               | 0     | 5000        |
| 70        | 1500       | 700        | 9500          | 1500          | 1500    | 0            | 0               | 8000            | 0               | 8000           | 0               | 0            | 0           | 0            | 5000        | 0               | 0     | 5000        |
| 80        | 2000       | 1000       | 8000          | 2500          | 2500    | 0            | 0               | 5500            | 0               | 5500           | 0               | 0            | 0           | 0            | 5000        | 0               | 0     | 5000        |
| 90        | 700        | 1500       | 6000          | 4000          | 4000    | 0            | 0               | 2000            | 0               | 2000           | 0               | 0            | 0           | 1            | 5000        | 4000            | 1     | 999         |
| 100       | 100        | 1000       | 5300          | 5000          | 5000    | 1            | 5000            | 300             | 0               | 300            | 0               | 0            | 0           | 0            | 5000        | 0               | 0     | 5000        |
| 101       | 100        | 0          | 5200          | 5000          | 5000    | 1            | 5000            | 200             | 0               | 200            | 0               | 0            | 0           | 0            | 5000        | 0               | 0     | 5000        |
| 102       | 100        | 0          | 5100          | 5000          | 5000    | 1            | 5000            | 100             | 0               | 100            | 0               | 0            | 0           | 0            | 5000        | 0               | 0     | 5000        |
| 103       | 0          | 0          | 5000          | 5000          | 5000    | 1            | 5000            | 0               | 0               | 0              | 0               | 1            | 0           | 0            | 5000        | 0               | 0     | 5000        |
| 104       | 0          | 0          | 5000          | 5000          | 5000    | 1            | 5000            | 0               | 0               | 0              | 0               | 1            | 0           | 0            | 5000        | 0               | 0     | 5000        |
| 105       | 0          | 0          | 5000          | 5000          | 5000    | 1            | 5000            | 0               | 0               | 0              | 0               | 1            | 0           | 0            | 5000        | 0               | 0     | 5000        |
| 106       | 500        | 100        | 5000          | 5100          | 5000    | 1            | 5000            | 0               | 100             | 100            | 0               | 0            | 0           | 0            | 5000        | 0               | 0     | 5000        |
| 107       | 500        | 100        | 4500          | 5200          | 4500    | 0            | 0               | 0               | 700             | 700            | 0               | 0            | 0           | 1            | 5000        | 4500            | 1     | 499         |
| 108       | 1000       | 200        | 4000          | 5400          | 4000    | 0            | 0               | 0               | 1400            | 1400           | 0               | 0            | 0           | 0            | 5000        | 0               | 0     | 5000        |
| 109       | 1000       | 200        | 3000          | 5600          | 3000    | 0            | 0               | 0               | 2600            | 2600           | 0               | 0            | 0           | 0            | 5000        | 0               | 0     | 5000        |
| 110       | 2000       | 200        | 2000          | 5800          | 2000    | 0            | 0               | 0               | 3800            | 3800           | 0               | 0            | 0           | 0            | 5000        | 0               | 0     | 5000        |

Every column is an array, which will be interpolated into a polynomial, and eventually the prover will have commitments to those polynomials (notation assigned in Table 2). Those commitments are the entities we use in our vanishing equations, being the backbone of our methodology, will be batched together later on to provide a verifiable proof. 

The first two columns, Order Book Vectors, are private vectors where $\mathsf {B}_i$ is the volume of bids and $\mathsf {A}_i$ is the volume of asks at price $\mathsf {P}_i$.
Respectively, $\mathsf {Depth_B}$ and $\mathsf {Depth_A}$ columns work as accumulators, being the cumulative demand from highest to lowest price and cumulative supply from lowest to highest price.
$\mathsf {Min}$ array is the minimum of $\mathsf {Depth_B}$ and $\mathsf {Depth_A}$ columns, from which we choose the Market Clearing Volume (price). 
Selector vectors, $\mathsf {Mask_Plateau}$ and $\mathsf {Mask_Valley}$, will later on limits the interval we are working with, help with our range checks, and reduce the computation complexity.  
Vector $\mathsf {surplus_{B}}$ and $\mathsf {surplus_{A}}$ are:
$\mathsf {surplus_{B}} = \mathsf {Depth_B} - \mathsf {Min}$ 
$\mathsf {surplus_{A}} = \mathsf {Depth_A} - \mathsf {Min}$
which denote the surplus of the market. 
$\mathsf {Delta}$ is the absolute value of the $\mathsf {surplus_{B}} + \mathsf {surplus_{A}}$.
$\mathsf {Clif}$ array will only carry the two values at each side of the "plateau" and consequently help with our tie-breaking "valley" identification. 
$\mathsf {Slack}$ soaks up the difference in the clifs of the plateau.
Market Clearing Volume is an array set to the identified, unified price at every index $i$. 
Notation of arrays and the corresponding commitments to their polynomials are seen as below:

**Table 2:**

| **Array**                | **Polynomial** | **Commitment ot the polynomial** |
| ------------------------ | -------------- | -------------------------------- |
| $\mathsf {B}_i$          | $P_{Bid}$      | $Bid$                            |
| $\mathsf {A}_i$          | $P_{Ask}$      | $Ask$                            |
| $\mathsf {Depth_B}$      | $P_{Acc_B}$    | $Acc_B$                          |
| $\mathsf {Depth_A}$      | $P_{Acc_A}$    | $Acc_A$                          |
| $\mathsf {Min}$          | $P_{Min}$      | $Min$                            |
| $\mathsf {surplus_{B}}$  | $P_{Surp_B}$   | $Surp_B$                         |
| $\mathsf {surplus_{A}}$  | $P_{Surp_A}$   | $Surp_A$                         |
| $\mathsf {Delta}$        | $P_{Delta}$    | $Delta$                          |
| $\mathsf {Slack}$        | $P_{Slack}$    | $Slack$                          |
| $\mathsf {Mask_Plateau}$ | $P_{Mask_{P}}$ | $Mask_P$                         |
| $\mathsf {Mask_Plateau}$ | $P_{Mask_{V}}$ | $Mask_V$                         |
| $\mathsf {Slack}$        | $P_{Slack}$    | $Slack$                          |
|                          |                |                                  |


We had to do range checks for the corresponding polynomials to the arrays in Table 1 that have a "++" beside them. The reason is to check if the numbers can be used in modular arithmetic in the first place. 



----------------------------------------------
### **Protocol Architecture & Mathematical Foundation**

The protocol is built on a Polynomial Interactive Oracle Proof (PIOP) framework using a multiplicative subgroup $H$ of size $n$.
Evaluation domain is $H = \{1, \omega, \omega^2, \dots, \omega^{n-1}\}$, where $\omega$ is the generator of the subgroup in the finite field $\mathbb{F}_q$. Here, we have the vanishing polynomial $Z_H(X) = X^n - 1$, which equals zero for all $X \in H$.
To enforce recurrence relations across price ticks, we utilize the transition constraint structure (Plonkbook reference):    $$(X - \omega^{n-1}) \cdot [ \text{Constraint Logic} ] = 0$$
This ensures the relation holds for all $i \in \{0, \dots, n-2\}$ while preventing an undefined "wrap-around" at the final domain point (enforcing recurrence relations (e.g., $A_{i+1} = A_i + B_i$) without causing a contradiction at the final domain point.).

#### Demand Accumulator ($Acc_B$ / BidsDepth)

Definition: Represents total cumulative demand—the quantity willing to buy at a given price or higher. As seen in the spreadsheet image, standard demand sums _down_ the price list (starting high at low prices and decreasing as prices rise).

Polynomial: $Acc_B(X)$.

Vanishing Equation (Plookup Transition Logic): To ensure this decreasing backward sum is consistent, the recurrence is defined as $Acc_B(X) = Acc_B(\omega X) + BidVol(X)$ (Total Demand at index $i$ is total demand at next index $i+1$ plus current bids). We apply the supervisor’s template structure (transition template) that zeros the constraint at the very last point:
$$V_{Acc_B} := (X - \omega^{n-1}) \cdot \left[ Acc_B(X) - (Acc_B(\omega X) + BidVol(X)) \right] = 0$$

#### Supply Accumulator ($Acc_A$ / AsksDepth)

Definition: Represents total cumulative supply—the quantity willing to sell at a given price or lower. This sums _forwards_ along the price list (starting low and increasing as prices rise).

Polynomial: $Acc_A(X)$.

Vanishing Equation (Plookup Transition Logic): The recurrence is defined as $Acc_A(\omega X) = Acc_A(X) + AskVol(X)$ (Total supply at next index $i+1$ equals current supply at index $i$ plus current asks).
