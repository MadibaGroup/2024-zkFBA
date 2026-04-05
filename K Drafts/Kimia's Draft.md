# **Secure Market Clearing via ZK-SNARKs: A Protocol for Private Call Auctions**


### **Abstract**
Frequent batch auctions (FBAs) have been proposed as an alternative to traditional limit order books for trading securities. The motivation is to mitigate the predatory advantages of high-frequency traders (HFTs). With FBAs, a double-sided auction is held over a short interval (e.g., 1 second). All marketable orders submitted during the time window are executed at the same clearing price, and arrival time is not a factor. FBAs are significantly less transparent than continuous-time orderbooks and rely on fully trusted specialists or exchanges to execute orders at the fairest price. In this research, we present a special-purpose zk-SNARK argument to develop a zk-FBA, which enables the specialist to prove trades are executed fairly with the correct clearing price, yielded by truthful computations, without revealing any of the orders directly.


### **1. Introduction**


(((Later with citation) Call markets are essential for price discovery, particularly during market opens or periods of high volatility. Traditional auctions require a central authority to view all order data, creating risks for front-running. This protocol allows an auctioneer to commit to an order book and prove the resulting clearing price is correct under the rules of supply and demand without leaking the competitive landscape of the participants.))

### **2. Background and Related work**

**2.1. Frequent Batch Markets Security**


**2.2. Succinct Proofs**


**2.3. The Positive Check**



#### **The Positive and Range Check**
The Positive Check enables verifiable inequalities in a SNARK circuit. Proving that a value is a maximum is important because it prevents malicious under-matching, as it ensures the auctioneer cannot pick a lower volume to favor certain participants. It also enforces economic validity by guaranteeing the cleared volume does not exceed available supply or demand at any tick. Plateau discovery prevention is another advantage of this check, as it proves the clearing price falls within the range where the highest number of trades can occur.



# **3. Zeequent: Protocol Specification**

#### **Market Setup**

####
(**Comment:**

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

3.8. The Tie Breaker Surplus and "Valley" constraints



Chart 1:

![[Code_Generated_Image 1.png]]

*How to find the Market Clearance Price (Value).*  We follow a simple algorithm to do so. Ideally, the first step would be to sort the data we have. In the auction ecosystem, we typically have prices, their corresponding bids and asks and the depth (cumulative volume of the request at that specific price) of these requests. These data form five distinguished arrays, sorted according to the prices. An example of these "sorted arrays" can be found in Table 1. The next step is to find the minimum request volume at each price tick. This forms another array (Min(Depth of Asks, Depth of Bids)). This is due to the convenience of supplying and demanding parties. Afterwards, we need to find the global maximum value in this minimum array to maximize the closing price of the market. This might be a single value at just one price, or a single value at different price ticks, forming a "Plateau", as it can be seen in Chart 1. Note that there might be a Plateau inside a plateau as the two arrays forming the minimum array (Depths of Bids and Asks) are strictly monotone, however not necessarily strictly increasing. You can see such data in Table 2 and chart 2 (///////////), forming a plateau inside a plateau. If there is a plateau instead of just one global maximum value, we need a tie-breaking policy to choose the clearing volume. We do so by calculating the ask and bid surplus, and eventually adding them up to form the Delta array. This creates a "Valley" array, that may or may not fall into our plateau price interval (example Table x and chart x). In this case, we just choose the minimum value of Delta in the plateau region. However, in case of the plateau and the valley overlapping, finding the global minimum of the valley helps us choose the clearing volume, which will eventually give us the uniform clearing price, with which we execute all the transactions and close the market. 






**Comment)**
####

----------------------------
To better convey how we go about the methodology, we'll use Table 1 as a an example:

**Table 1:**

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
$\mathsf {Min}$ array is the minimum of $\mathsf {Depth_B}$ and $\mathsf {Depth_A}$ arrays, from which we choose the Market Clearing Volume (price). 
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

| **Array**                | **Polynomial** | **Commitment to the polynomial** |
| ------------------------ | -------------- | -------------------------------- |
| $\mathsf {B}_i$          | $P_{Bid}$      | $Bid(X)$                         |
| $\mathsf {A}_i$          | $P_{Ask}$      | $Ask(X)$                         |
| $\mathsf {Depth_B}$      | $P_{Acc_B}$    | $Acc_B(X)$                       |
| $\mathsf {Depth_A}$      | $P_{Acc_A}$    | $Acc_A(X)$                       |
| $\mathsf {Min}$          | $P_{Min}$      | $Min(X)$                         |
| $\mathsf {surplus_{B}}$  | $P_{Surp_B}$   | $Surp_B(X)$                      |
| $\mathsf {surplus_{A}}$  | $P_{Surp_A}$   | $Surp_A(X)$                      |
| $\mathsf {Delta}$        | $P_{Delta}$    | $Delta(X)$                       |
| $\mathsf {Slack}$        | $P_{Slack}$    | $Slack(X)$                       |
| $\mathsf {Mask_Plateau}$ | $P_{Mask_{P}}$ | $Mask_P(X)$                      |
| $\mathsf {Mask_Plateau}$ | $P_{Mask_{V}}$ | $Mask_V(X)$                      |
| $\mathsf {Slack}$        | $P_{Slack}$    | $Slack(X)$                       |
|                          |                |                                  |
|                          |                |                                  |


We had to do range checks for the corresponding polynomials to the arrays in Table 1 that have a "++" beside them. The reason is to check if the numbers can be used in modular arithmetic in the first place. 



----------------------------------------------
## **A. Protocol Architecture & Mathematical Foundation: Introducing Tools**
#### **A.1. Transition Logic for Vanishing equations**

The protocol is built on a Polynomial Interactive Oracle Proof (PIOP) framework using a multiplicative subgroup $H$ of size $n$. Evaluation domain is $H = \{1, \omega, \omega^2, \dots, \omega^{n-1}\}$, where $\omega$ is the generator of the subgroup in the finite field $\mathbb{F}_q$. Here, we have the vanishing polynomial $Z_H(X) = X^n - 1$, which equals zero for all $X \in H$. To enforce recurrence relations across price ticks, we utilize the transition constraint structure (Plonkbook transition logic reference) for our vanishing equations:    $$(X - \omega^{n-1}) \cdot [ \text{Constraint Logic} ] = 0$$
This ensures the relation holds for all $i \in \{0, \dots, n-2\}$ while preventing an undefined "wrap-around" at the final domain point (enforcing recurrence relations (e.g., $A_{i+1} = A_i + B_i$) without causing a contradiction at the final domain point.).

-----------------------------------------
#### **A.2. The Modular Equator Logic for Range Check**

Because the auction operates in a finite field $\text{mod } q$, standard bit-decomposition is insufficient to prevent modular wrap-around. To define "positive" values in a modular field, we adopt a Half-Field Range Check. Any value $S(X)$ is considered valid if it lies in the first half of the interval $[0, \frac{q-1}{2}]$. This is critical for the Surplus Columns ($\mathbb{Z}_+$); if Trade Volume incorrectly exceeded available Depth, the resulting negative surplus would "jump" the equator to $q-1$, failing the range proof.

*Plookup Vanishing Equations*. Unlike bit-decomposition, which breaks a number into powers of two, this method proves that the surplus values ($f$) are contained within a public table of allowed positive values ($t$). This is achieved using a Grand Product Polynomial $Z(X)$ and a Sorted Polynomial $s(X)$ that combines the data (e.g., surplus data) and the example table. For each surplus column, the following two constraints or vanishing equations must hold over the domain $H$:

*Boundary Constraint (Start Condition)*. This ensures the grand product calculation begins correctly at the first price tick.

$$L_1(X)(Z(X) - 1) = 0$$

$L_1(X)$ is the Lagrange polynomial that is 1 at the first price tick ($\omega^0$) and 0 elsewhere. $Z(X)$ is the Grand Product polynomial that tracks the cumulative relationship between the surplus and the valid range table.

*Transition Constraint (The Range Proof)*. This equation enforces that every surplus value is "found" in the range table as we traverse the price ticks.

$$(X - \omega^{n-1}) \left[ Z(\omega X)(\gamma + s(X) + \beta s(\omega X)) - Z(X)(\gamma(1+\beta) + f(X) + \beta t(X)) \right] = 0$$

$f(X)$ is the witness polynomial to do range check for (e.g., the surplus columns). $t(X)$ is the public table polynomial containing the permitted range of positive integers (e.g., $0$ to $2^{16}-1$). $s(X)$ is a "Sorted" polynomial that interweaves the values of $f$ and $t$ in non-decreasing order to prove membership. $\beta, \gamma$ are random challenges provided by the verifier to ensure the prover cannot manipulate the entries.

----------------------------------

#### **A.3. Bit-Decomposition Logic for Value Check** 

For Finding the absolute extrema (e.g., finding global maximum), the goal is to prove that the assumed global maximum value is greater than or equal to another volume in the same array. To do so, we need to show the difference between the assumed max and other values in the array is greater or equal to zero. We achieve this by proving that the difference $D(X) = V_{max} - V(X)$ has a valid $k$-bit decomposition. If $D(X)$ can be represented as a sum of $k$ bits, it is mathematically forced to be in the range $[0, 2^k - 1]$, thus $V(X) \le V_{max}$. Same approach in reverse can be used for finding the global minimum.
*The Recurrence Relation.* We define $k$ witness columns (or a single column across $k$ rows) for bits $B_0, \dots, B_{k-1}$. For a fixed price tick $i$, the state is:

$$Acc_{j+1} = Acc_j + 2^j \cdot B_j$$
Where $Acc_0 = 0$ and $Acc_k = D_i$.

*Vanishing Equations for the "Ceiling".* Using our transition constraint structure to handle the $n$-sized subgroup $H$, the bit-Integrity (Booleanity), enforces that each decomposition component is a bit.$$B_j(X) \cdot (1 - B_j(X)) = 0 \pmod{Z_H(X)}, \quad \forall j \in \{0, \dots, k-1\}$$
So the tool for this check will be:
$$S(X) = \sum_{j=0}^{k-1} 2^j \cdot B_j(X)$$

--------------------
#### **A.4. Verification Logic for Batching**

As per the Zeeperio "start to finish" check, the verifier performs the following:

As you will see, there will be many vanishing polynomials (constraints) that must all equal zero simultaneously. Instead of the verifier checking each of the constraints separately, which would be computationally expensive, the prover creates a random linear combination of all constraints using powers of $\alpha$. Here, $\alpha^i$ is the random weight assigned to the $i$-th constraint to ensure they don't cancel each other out maliciously.Again, to do the algebraic check, we use the random challenge $\zeta \notin H$ to check that the batched vanishing polynomials (multiplied by the quotient $Q(X)$) evaluate to zero.
$$Batch(\alpha) = \sum_{i=1}^{m} \alpha^i (V_i(X) - Q_i(X)Z_H(X))$$
$$\sum \alpha^i (\mathsf{V}_i(\zeta) - Q_i(\zeta) \cdot Z_H(\zeta)) = 0$$


------------------------------------
## **B. Constraints for the Market** 
We should do a range check for Both $Bid$ and $Ask$ polynomials as seen in section A.2. to make sure the data is valid to use for our polynomial operations. 

####
(**Comment:** I can just mention "we use the tool A.x." and not write the whole thing for each part, Idk if that'd be ok in this academic context. Also, formatting of the sections and how to link them will be better handled in the LaTeX code.)
####

#### **Volume of Bids and Asks at Price $\mathsf {P}_i$ Polynomials ($Bid(X)$ and $Ask(X)$)**

*Definition.* These two polynomials represent the supply and demand in the market. In a finite field $\mathbb{F}_q$, a negative order (e.g., "I want to sell $-10$ shares") is represented as $q-1$. Without a range check on the raw inputs, the accumulators ($Acc_A$ and $Acc_B$) would inherit these massive values, causing the trade volume calculations to produce nonsensical or malicious results. By forcing $Bid(X)$ and $Ask(X)$ into the Lower Half of the field, we mathematically guarantee that every individual order is a legitimate, positive quantity. We define a public table $t_{in}(X)$ that contains all allowed order sizes. The safe interval is $\{0, 1, \dots, N_{max}\}$, where $N_{max}$ is the maximum possible size for a single order.    

*Constraint.* The "equator" boundary $N_{max} < \frac{q-1}{2}$ ensures that no order can be interpreted as a negative value by wrapping around $q$. For both $Bid(X)$ and $Ask(X)$, the prover must generate a Grand Product polynomial $Z_{in}(X)$ and a sorted polynomial $s_{in}(X)$ that satisfy the following equations over $H$:

 *Initialization (Lagrange Start).* The product must begin at 1 at the first price tick ($\omega^0$).

$$L_1(X) \cdot (Z_{in}(X) - 1) = 0$$

*Transition (The "Equator" Range Check).* We verify that every entry in the raw column exists in the "Safe Zone" table $t_{in}(X)$.

$$(X - \omega^{n-1}) \cdot \left[ Z_{in}(\omega X)(\gamma + s_{in}(X) + \beta s_{in}(\omega X)) - Z_{in}(X)(\gamma(1+\beta) + f_{raw}(X) + \beta t_{in}(X)) \right] = 0$$
$f_{raw}(X)$ is the polynomial for either $Bid(X)$ or $Ask(X)$. $\beta, \gamma$ are verifier's random challenges. $s_{in}(X)$ is the sorted polynomial proving $f_{raw} \subset t_{in}$.

####
**(Comment:**
*Termination (Final Consistency).* The final value of the grand product must match the expected permutation product of the multiset (not here but would probably come in handy for the parts requiring permutation).

$$L_n(X) \cdot (Z_{in}(X) - \text{Target}) = 0$$
**Comment)**
####
--------------------------

#### Demand Accumulator Polynomial $Acc_B(X)$

*Definition.* Represents total cumulative demand, the quantity willing to buy at a given price or higher. As seen in the spreadsheet image, standard demand sums _down_ the price list (starting high at low prices and decreasing as prices rise).

*Demand Accumulator Initialization.* Enforces that the demand starts at the highest price tick.
$$\mathsf V_{Acc_B, I}(X) = (Acc_B(X) - Bid(X)) \cdot \frac{Z_H(X)}{X - \omega^0} = 0$$

*Vanishing Equation (Plookup Transition Logic).* To ensure this decreasing backward sum is consistent, the recurrence is defined as $Acc_B(X) = Acc_B(\omega X) + Bid(X)$ (Total Demand at index $i$ is total demand at next index $i+1$ plus current bids). We apply the template structure (transition template) that zeros the constraint at the very last point:
$$\mathsf V_{Acc_B} := (X - \omega^{n-1}) \cdot \left[ Acc_B(X) - (Acc_B(\omega X) + Bid(X)) \right] = 0$$


#### Supply Accumulator Polynomial $Acc_A(X)$

*Definition.* Represents total cumulative supply, the quantity willing to sell at a given price or lower. This sums _forwards_ along the price list (starting low and increasing as prices rise).

*Supply Accumulator Initialization. Enforces that the supply starts at zero (or the first bid) at the lowest price tick. 
$$\mathsf V_{AccA, I}(X) = (Acc_A(X) - Ask(X)) \cdot \frac{Z_H(X)}{X - \omega^{n-1}} = 0$$

*Vanishing Equation.* The recurrence is defined as $Acc_A(\omega X) = Acc_A(X) + Ask(X)$ (Total supply at next index $i+1$ equals current supply at index $i$ plus current asks).
$$\mathsf V_{Acc_A} :=(X - \omega^{n-1}) \cdot \left[ Acc_A(\omega X) - (Acc_A(X) + Ask(X)) \right] = 0$$


---------------
###### 
#### **Minimum polynomial ($Min(X)$)**

*Definition.* The actual volume of shares that will execute at a price tick, which is always the lesser of cumulative supply or cumulative demand.

*Vanishing Equation.* This column is the single most critical intermediate output. All subsequent market properties, including the $V_{max}$ Plateau Check, Cliffs Proof, and the Surplus Valley tie-breaking logic, are calculated from this column. Proving this column is strictly $\min(Acc_A, Acc_B)$ is essential for the security of the auction; without it, a malicious auctioneer could set a trading volume higher than the available supply, effectively "inventing shares". 
This ensures that eventually our $V_{max}$ is the maximum chosen from the true minimum column and there are no volume values smaller than $\mathsf{Min}_i$ at $i$, since the Market Clearing Volume is the global maximum value of the $\mathsf{Min}_i = \min(\mathsf{Acc}_{A,i}, \mathsf{Acc}_{B,i})$.  Hence, if $V_{max}$ is the maximum in this column, it is biggest number to choose as our clearing price. To prove a $\min$ relationship in ZK, we use the constraint that requires the value to strictly equal one of its sources, combined with range checks. We enforce that the "Slack" (Surplus) on at least one side of the matching equation must be zero:
$$\mathsf V_{min}(X) = (Acc_A(X) - Min(X)) \cdot (Acc_B(X) - Min(X)) = 0$$


---------------------


#### Selector Polynomials ($Mask(X)$)







####
(**comment:** ADV: two methods to explore: shuffling and permutation (code, line chart to show complexity as to which one is better in which situation)
####






-------------------------------

#### Market Clearing Volume ($MCV(X)$)

*Definition.* Global Maximum Value


