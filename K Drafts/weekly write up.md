### **1. Abstract**

Frequent Batch Auctions (FBAs) have emerged as a robust solution for enhancing market stability and price discovery by aggregating order flow over discrete time intervals, effectively neutralizing predatory high-frequency trading. To guarantee that these auctions are executed with absolute fiduciary integrity, we leverage Zero-Knowledge SNARKs to provide succinct, verifiable proofs of computational honesty without compromising sensitive institutional data.
A significant challenge arises because FBAs typically operate as blind auctions to protect against front-running, creating a trust gap for traders who cannot verify if a fill was executed at the true market-clearing price. This research introduces a custom ZK-SNARK protocol that mathematically certifies that reported execution volume and price represent the true global equilibrium of the hidden order book, ensuring optimal execution while maintaining total bid confidentiality.

---
.......

 Zeeperio-style explanation about math prerequisites

  .......

To get the clearing price $p^*$, we must find the minimum between bid and ask volumes at every price. The largest volume value among these minimum volume values is our clearing price. We assume $V_{max}$ is the volume that sets the market's clearance price $p^*$.

To prove the clearing state without revealing private order books, the auctioneer (prover) transforms the data into cumulative distributions which are consequently interpolated into univariate polynomials, and sends succinct KZG commitments to the verifier (witness commitments). 
$\mathsf{Acc}_A(X)$ is the Supply (Ask) Accumulator column. Built from the asked volumes at the lowest price to the highest, representing total supply at price $\mathsf {P}_i$ or lower. $\mathsf{Acc}_B(X)$ is the Demand (Bid) Accumulator column. Built from the bid volumes at the highest price to the lowest, representing total demand at price $P_i$ or higher. $\mathsf{Min}(X)$ is the asserted minimum volume column at each price tick. 
To prove $\mathsf{Min}_i = \min(\mathsf{Acc}_{A,i}, \mathsf{Acc}_{B,i})$, we define the difference columns $K$ and $L$:
$K(x) = \mathsf{Acc}_A(x) - \mathsf{Min}(x)$.   
$L(x) = \mathsf{Acc}_B(x) - \mathsf{Min}(x)$.
Subsequently, the respective polynomials are:
$P_K(x) = P_{\mathsf{Acc}_A}(x) - P_{\mathsf{Min}}(x)$: The excess supply at price $X$.   
$P_L(x) = P_{\mathsf{Acc}_B}(x) - P_{\mathsf{Min}}(x)$: The excess demand at price $X$.
  
___

### **Protocol Architecture & Mathematical Foundation**



---
### **Columnar Integrity: The Data Availability Layer**
#### Raw Order Volumes (Bids & Asks)

Definition: These columns represent the private, raw quantity of shares bid and asked at each discrete price tick provided in the order book.

Project Role: They are the foundational commitment values. If the prover lies here, all derived cumulative columns and the final execution price will be invalid.

Polynomials:    
$BidVol(X)$: Encodes the Bid quantity at price tick $\omega^i$.
$AskVol(X)$: Encodes the Ask quantity at price tick $\omega^i$.


#### Demand Accumulator ($Acc_B$ / BidsDepth)

Definition: Represents total cumulative demand—the quantity willing to buy at a given price or higher. As seen in the spreadsheet image, standard demand sums _down_ the price list (starting high at low prices and decreasing as prices rise).

Polynomial: $Acc_B(X)$.
 
Vanishing Equation (Plookup Transition Logic): To ensure this decreasing backward sum is consistent, the recurrence is defined as $Acc_B(X) = Acc_B(\omega X) + BidVol(X)$ (Total Demand at index $i$ is total demand at next index $i+1$ plus current bids). We apply the supervisor’s template structure (transition template) that zeros the constraint at the very last point:
$$V_{Acc_B} := (X - \omega^{n-1}) \cdot \left[ Acc_B(X) - (Acc_B(\omega X) + BidVol(X)) \right] = 0$$

#### Supply Accumulator ($Acc_A$ / AsksDepth)

Definition: Represents total cumulative supply—the quantity willing to sell at a given price or lower. This sums _forwards_ along the price list (starting low and increasing as prices rise).

Polynomial: $Acc_A(X)$.

Vanishing Equation (Plookup Transition Logic): The recurrence is defined as $Acc_A(\omega X) = Acc_A(X) + AskVol(X)$ (Total supply at next index $i+1$ equals current supply at index $i$ plus current asks).
$$V_{Acc_A} :=(X - \omega^{n-1}) \cdot \left[ Acc_A(\omega X) - (Acc_A(X) + AskVol(X)) \right] = 0$$
--------------------------
#### Trade Volume ($Min(Acc_A, Acc_B)$ / TradeVol)

Definition: The actual volume of shares that will execute at a price tick, which is always the lesser of cumulative supply or cumulative demand.

Polynomial: $TradeVol(X)$.

Project Criticality : This column is the single most critical intermediate output. All subsequent market properties, including the $V_{max}$ Plateau Check, Cliffs Proof, and the Surplus Valley tie-breaking logic, are calculated from this column. Proving this column is strictly $\min(Acc_A, Acc_B)$ is essential for the security of the auction; without it, a malicious auctioneer could set a TradeVol higher than the available supply, effectively "inventing shares".

To prove a $\min$ relationship in ZK, we use the constraint that requires the value to strictly equal one of its sources, combined with range checks. We enforce that the "Slack" (Surplus) on at least one side of the matching equation must be zero:

$$V_{KL}(X) := (Acc_A(X) - TradeVol(X)) \cdot (Acc_B(X) - TradeVol(X)) = 0$$
While not a transition relation, this constraint must hold for all $X \in H$ to satisfy the integrity requirements.

------------------





### **The Permutation Argument for Selector Contiguity**

To prevent the prover from selecting "scattered" price ticks that do not represent a valid market interval, we enforce a constraint on the number of transitions (or "cliffs") in the selector polynomial $S(X)$.

#### **1. The Difference Polynomial $D(X)$**

We define a derived polynomial $D(X)$ that captures the step-changes between adjacent price ticks:

$$D(X) = S(X) - S(\omega X)$$

For a contiguous plateau $[c, d]$, the values of $D$ across the domain $H$ must be zero everywhere, except at the two boundaries:

- **The "Jump Up"**: $D(\omega^{c-1}) = 0 - 1 = -1$
    
- **The "Jump Down"**: $D(\omega^d) = 1 - 0 = 1$
    
- **All other points**: $D(\omega^i) = 1-1$ or $0-0 = 0$
    

#### **2. The Target Multiset $M$**

We define a public, fixed multiset $M$ that represents the "ideal" shape of a single plateau:

$$M = \{ -1, 1, 0, 0, \dots, 0 \}$$

The goal of the permutation argument is to prove that the set of values in our witness polynomial $D(X)$ is a **permutation** of the values in $M$.

---

### **3. The Grand Product Vanishing Equation**

To prove this permutation in ZK without revealing where the "jumps" occur, we use the **PLONK Grand Product** logic. We introduce a random challenge $\gamma$ from the verifier to "lock" the multiset.

The identity we must prove is that the product of all terms $(D_i + \gamma)$ equals the product of all terms $(M_i + \gamma)$:

$$\prod_{i=0}^{n-1} (D(\omega^i) + \gamma) = \prod_{i=0}^{n-1} (M_i + \gamma)$$

Because $M$ is mostly zeros, the target product is simplified to:

$$\text{Target} = (-1 + \gamma) \cdot (1 + \gamma) \cdot (\gamma)^{n-2}$$

#### **The Vanishing Transition Equation**

We define a Grand Product polynomial $Z_{perm}(X)$ that accumulates the ratio of these products across the domain:

1. **Start Condition**: $L_1(X)(Z_{perm}(X) - 1) = 0$
    
2. **Transition**:
    
    $$(X - \omega^{n-1}) \cdot [ Z_{perm}(\omega X) \cdot \gamma - Z_{perm}(X) \cdot (D(X) + \gamma) ] = 0$$
    
3. **End Condition**: $L_n(X)(Z_{perm}(X) - \text{Target}) = 0$
    

---

### **4. Economic Significance: Proving the "One Plateau" Rule**

By enforcing this permutation argument, the protocol mathematically guarantees that:

**Contiguity**: There is exactly one "start" and one "end" to the clearing price range. The auctioneer cannot "cherry-pick" non-adjacent prices to manipulate the volume.

**Unimodality Alignment**: This shape proof aligns with the economic theory that the **Trade Vol** function is unimodal. The SNARK proves the reported plateau is the unique, contiguous peak of that function.

**Integrity of the Valley**: Because the plateau is proved to be a single block, the subsequent **Surplus Valley** check is guaranteed to be searching within the correct, high-liquidity interval rather than a fragmented set of prices.



---------------------------


### **Generalized "Middle of the Interval" Plookup for all of our positive checks**

Because the auction operates in a finite field $\text{mod } q$, standard bit-decomposition is insufficient to prevent modular wrap-around.

#### **The Modular Equator Logic**

To define "positive" values in a modular field, we adopt a Half-Field Range Check. Any value $S(X)$ is considered valid if it lies in the first half of the interval $[0, \frac{q-1}{2}]$. This is critical for the Surplus Columns ($\mathbb{Z}_+$); if Trade Volume incorrectly exceeded available Depth, the resulting negative surplus would "jump" the equator to $q-1$, failing the range proof.

#### **Plookup Vanishing Equations**

Unlike bit-decomposition, which breaks a number into powers of two, this method proves that the surplus values ($f$) are contained within a public table of allowed positive values ($t$). This is achieved using a Grand Product Polynomial $Z(X)$ and a Sorted Polynomial $s(X)$ that combines the surplus data and the table.

For each surplus column (Bid Surplus $S_B$ and Ask Surplus $S_A$), the following two vanishing equations must hold over the domain $H$:

1. Boundary Constraint (Start Condition)

This ensures the grand product calculation begins correctly at the first price tick.

$$L_1(X)(Z(X) - 1) = 0$$

**$L_1(X)$**: The Lagrange polynomial that is 1 at the first price tick ($\omega^0$) and 0 elsewhere.

**$Z(X)$**: The Grand Product polynomial that tracks the cumulative relationship between the surplus and the valid range table.

#### 2. Transition Constraint (The Range Proof)

This equation enforces that every surplus value is "found" in the range table as we traverse the price ticks.

$$(X - \omega^{n-1}) \left[ Z(\omega X)(\gamma + s(X) + \beta s(\omega X)) - Z(X)(\gamma(1+\beta) + f(X) + \beta t(X)) \right] = 0$$

**$f(X)$**: The witness polynomial for the surplus column (either Bid Surplus or Ask Surplus).
**$t(X)$**: The public table polynomial containing the permitted range of positive integers (e.g., $0$ to $2^{16}-1$).
**$s(X)$**: A "Sorted" polynomial that interweaves the values of $f$ and $t$ in non-decreasing order to prove membership.
**$\beta, \gamma$**: Random challenges provided by the verifier to ensure the prover cannot manipulate the entries.

-----
(For selector, two paths: Permutation and Shuffle)
(Notation for PIOP)
(Rust: Monte Carlo it?)
(Data search?)


///////////////////////////////////////////////////////////////////////////////////////////

from Kimia's draft
-------------------------

#### **3.1. Conditions to check**

Firstly, a range proof demonstrates that $V_{max} - \mathsf{Min}(X) \geq 0$ for all $X \in H$. This uses bit-decomposition to prove non-negativity without revealing the differences (Maximum Check). Then the prover opens the values of $\mathsf{Min}(X)$ at the edges of the interval (e.g., prices 101 and 103) in plain text to show they are strictly less than $V_{max}$. The final price is justified by opening the Surplus Polynomial ($\mathsf{Acc}_A(X) - \mathsf{Acc}_B(X)$) at the chosen clearing price to show the sign of the market imbalance.

The auctioneer satisfies the following conditions to prove the clearing price:
#### **3.1.1. Condition 1: Correctness of the Minimum Column**

**Constraint A (Non-negativity)**: Prove $P_K(x) \geq 0$ and $P_L(x) \geq 0$ via the Positive Check. 
(checked in our vanishing polynomial OR is it possible for it to even become a negative number?)
**Constraint B (Mutual Exclusivity)**: $P_{KL}(x)$ is the result of $P_K(x) \cdot P_L(x)$ and eventually $P_K(x) \cdot P_L(x) = 0$.

We can just check if $P_K(x) \cdot P_L(x) = 0$ holds. To do so in polynomial terms, we have to use a vanishing polynomial equation to show that $P_{KL}(x)$ is zero on our domain. If we correctly prove that Constraint B holds, we have indirectly proven that Constraint A is also true, otherwise the verification would fail since any false value at any step would result in an error in our proof. If this equation yields zero when the verifier inputs their challenge, then the verifier is sure that the prover has indeed been truthful and has the right values that work in our system. 
This ensures that eventually our $V_{max}$ is the maximum chosen from the true minimum column and there are no volume values smaller than $\mathsf{Min}_i$ at $i$, since the clearing price is the global maximum value of the $\mathsf{Min}_i = \min(\mathsf{Acc}_{A,i}, \mathsf{Acc}_{B,i})$.  Hence, if $V_{max}$ is the maximum in this column, then it is biggest number to choose as our clearing price. 

-----
##### Prerequisites and Mathematical Setup

The auctioneer maintains public and private vectors interpolated into polynomials. The evaluation domain is arbitrary and can be any set as long as the prover and verifier agree. The auction state is encoded into a finite field $\mathbb{F}_{q}$. Let $ω ∈ \mathbb{F}_{q}$ generate a multiplicative subgroup of size $n$. The set of discrete price ticks is mapped to an evaluation domain $H$ consisting of $n$ roots of unity:

$$H = \{\omega^{0}, \omega^{1}, \dots, \omega^{n-1}\} \subseteq \mathbb{F}_{q}$$

Interpolating a polynomial over this domain can be accomplished in $\mathcal{O}(n.log n)$ instead of $\mathcal{O}(n^2)$. Traversing the domain is easy: the next index is the current index multiplied by $ω$, and because it is closed under multiplication, the next element after the last element $ω^{n−1}$ wraps back to the first element $ω^{n−1} · ω = ω^n = ω^0$. Finally, a polynomial that is $0$ at each price tick defined as $X ∈ H$ has a simple closed form:

$$Z_{H}(X) = X^{n} - 1$$
The drawback is that the prover cannot use arrays of arbitrary length $\mathcal{P}$ and expect that an order $n$ subgroup will just exist. The subgroups are based on the parameters of the elliptic curve, and curves like alt-bn128 and bls12-381 are designed to offer subgroups of sizes {2, 22 , 23 , 24 , . . .} up to 228 for alt-bn128 and 232 for bls12-381.

##### **Succinct Proof of Vanishing (PIOP Steps)**

To convince the verifier that $P_{KL}(X)$ vanishes on $H$ without revealing the polynomials we follow these steps:
1. The prover computes a quotient polynomial: $$Q(X) = \frac{P_{KL}(X)}{Z_H(X)} = \frac{(P_B(x) - P_{\mathsf {Min}}(x))(P_A(x) - P_{\mathsf {Min}}(x))}{X^n - 1}$$
2. The prover commits to $Q(X)$ and sends the commitment $cm_Q$ to the verifier.
3. The verifier (or a strong Fiat-Shamir hash) issues a random evaluation point $\zeta \notin H$ as a challenge.
4. The prover opens the commitment and  provides evaluations and proofs for $\mathsf{Acc}_A(\zeta), \mathsf{Acc}_B(\zeta), \mathsf{Min}(\zeta),$ and $Q(\zeta)$.

For the market clearing volume to be correct, at every tick $\omega^i \in H$, the $\mathsf{Min}$ value must match at least one side of the book. This is enforced by the constraint polynomial $P_{KL}(X)$:

$$P_{KL}(X) := \mathsf{P}_K(X) \cdot \mathsf{P}_L(X) = (\mathsf{Acc}_B(X) - \mathsf{Min}(X))(\mathsf{Acc}_A(X) - \mathsf{Min}(X))$$

The verifier accepts the clearing volume if and only if the following vanishing equation holds at the challenge point $\zeta$:

$$(\mathsf{Acc}_B(\zeta) - \mathsf{Min}(\zeta))(\mathsf{Acc}_A(\zeta) - \mathsf{Min}(\zeta)) - Q(\zeta) \cdot (\zeta^n - 1) = 0$$


If the assertion is true, $P_{KL}(X)$ is a vanishing polynomial on $H$.

By proving $\mathsf{P}_{KL}(x)$ vanishes on the domain, we have proved the correctness of the minimum column. Therefore, if $V_{max}$ is the greatest number on this column, the proof is sound.
At every price tick $i$, either $\mathsf{P}_K(\omega^i) = 0$ or $\mathsf{P}_L(\omega^i) = 0$. This also implies that either $\mathsf{Acc}_B(\omega^i) = \mathsf{Min}(\omega^i)$ or $\mathsf{Acc}_A(\omega^i) = \mathsf{Min}(\omega^i)$. When combined with the Positive Check ($P_K, P_L \geq 0$), it is mathematically impossible for $\mathsf{Min}(x)$ to be anything other than the true minimum of the supply and demand curves.

-----------------

#### **3.1.2. Condition 2: Maximum Volume**

The auctioneer opens $V_{max}$ in plain text which is a single value forming a plateau or just a point in our $\mathsf{Min}(x)$ column's histogram. Either way, our proof yields correctness. 
**Constraint**: Prove $V_{max} - \mathsf{Min}(x) \geq 0$ for all $x$ in the domain. This proves no number bigger than $V_{max}$ exists in the column. 


We follow a similar approach as of section 3.1.1 and prove the correctness of the constraint mentioned in this section using a vanishing polynomial. First, we define the difference relationship:

$$D(x) = v - P_{\mathsf {Min}}(x)$$

If $v$ is the maximum, $D(x)$ must be $\geq 0$ for all $x$ in our domain $H$.

##### **The Range Proof Constraint**

In a ZK circuit, "greater than or equal to" isn't a native operation. We prove $D(x) \geq 0$ by showing that $D(x)$ is a member of a pre-defined set of positive integers (e.g., $\{0, 1, 2, \dots, 2^{16}-1\}$).
We create a Range Check Polynomial $R(y)$ that equals zero only if $y$ is in that allowed range ($y \in \mathbb{W}$).

Now, we combine these to make our constraint polynomial $C(x)$:

$$C(x) = R(D(x))=R(v - P_{\mathsf {Min}}(x))$$

If $v$ is truly the maximum, then for every $x$ in our domain $H$, the value $v - P_{\mathsf {Min}}(x)$ will be a positive number within our allowed range. Therefore, $C(x)$ will evaluate to zero at every point in $H$; therefore, we must show that $C(x)$ is a vanishing polynomial.

As of before, the prover compute the Quotient Polynomial $Q(x)$:

$$Q(x) = \frac{R(v - P_{\mathsf {Min}}(x))}{Z_H(x)}$$

Afterwards, we do the verification the same way as section 3.1.1.
If $v$ is the maximum, the division has no remainder, $Q(x)$ is a valid polynomial and our vanishing equation yields zero by inputing the random challenge $\zeta$.
However, if $v$ is not the maximum, for some $x$, $v - P_{\mathsf {Min}}(x)$ will be negative; therefore, it is outside the range. Consequently, at some point, $R(v - P_{\mathsf {Min}}(x)) \neq 0$ and the proof will fail.

---------------
#### 3.1.3. Condition 3: Plateau and Boundary Drop-off

To prove the optimal clearing interval $[c,d]$, we perform boundary openings,  where the auctioneer opens values immediately outside the plateau (e.g., 7 and 8?).
**Constraint**: $\mathsf{Min}(\omega^{c-1}) < V_{max}$ and $\mathsf{Min}(\omega^{d+1}) < V_{max}$ with $\omega$ defined in the domain $H = \{1, \omega, \omega^2, \dots, \omega^{n-1}\}$, where $n$ is the number of price ticks in the order book.
This demonstrates that $V_{max}$ is reached only within the plateau, and volume drops off elsewhere.

Therefore, we set $V_{c-1}, V_{d+1}$ as plaintext volumes opened at the prices immediately outside the plateau (e.g., 7 and 8).
$\mathsf{Mask}_{i}(X) = \frac{Z_H(X)}{X - \omega^i}$ is a public helper polynomial that is zero everywhere on $H$ except at price tick $i$.

### **The Vanishing Polynomials for the Plateau**

To prove the "drop-off," we must first prove that the asserted values at the boundaries are correct and then verify their relationship to $V_{max}$ in plain text.

#### **Constraint: Plateau Ceiling (Global Maximum)**

(?done in the next section?) To prove that nothing in the entire book is above $V_{max}$, we define a difference polynomial $\mathsf{P}_{pos}(X)$ and prove it is non-negative via bit-decomposition.

$$\mathsf{V}_{ceiling}(X) := \mathsf{P}_{pos}(X) - \sum_{j=0}^{k-1} 2^j \cdot B_j(X) = 0$$

where $\mathsf{P}_{pos}(X) = V_{max} - \mathsf{Min}(X)$. If this vanishes, then $\mathsf{Min}(X) \leq V_{max}$ for all $X \in H$.

#### **Constraint: Left Drop-off Opening**
This proves that the volume at price tick $c-1$ is exactly $V_{c-1}$.

$$\mathsf{V}_{drop\_left}(X) := (\mathsf{Min}(X) - V_{c-1}) \cdot \mathsf{Mask}_{c-1}(X) = 0$$
#### **Constraint: Right Drop-off Opening**
This proves that the volume at price tick $d+1$ is exactly $V_{d+1}$.

$$\mathsf{V}_{drop\_right}(X) := (\mathsf{Min}(X) - V_{d+1}) \cdot \mathsf{Mask}_{d+1}(X) = 0$$
#### **Constraint: Plateau Confirmation**
To ensure $V_{max}$ is reached at the edges of the interval, we may also prove the values at $c$ and $d$.

$$\mathsf{V}_{plateau\_c}(X) := (\mathsf{Min}(X) - V_{max}) \cdot \mathsf{Mask}_{c}(X) = 0$$

$$\mathsf{V}_{plateau\_d}(X) := (\mathsf{Min}(X) - V_{max}) \cdot \mathsf{Mask}_{d}(X) = 0$$

### **Verification Logic**

As per the Zeeperio "start to finish" check, the verifier performs the following:

1. As we saw, there are many vanishing polynomials (constraints) that must all equal zero simultaneously. Instead of the verifier checking each of the $m$ constraints separately, which would be computationally expensive on-chain, the prover creates a random linear combination of all constraints using powers of $\alpha$. Here, $\alpha^i$ is the random weight assigned to the $i$-th constraint to ensure they don't cancel each other out maliciously.Again, to do the algebraic check, we use the random challenge $\zeta \notin H$ to check that the batched vanishing polynomials (multiplied by the quotient $Q(X)$) evaluate to zero.
$$Batch(\alpha) = \sum_{i=1}^{m} \alpha^i (V_i(X) - Q_i(X)Z_H(X))$$
$$\sum \alpha^i (\mathsf{V}_i(\zeta) - Q_i(\zeta) \cdot Z_H(\zeta)) = 0$$
2. (???) For checking the plaintext inequality, once the openings confirm $V_{c-1}$ and $V_{d+1}$ are the correct values for those indices, the verifier checks the final condition in plain text:
$$V_{c-1} < V_{max} \quad \text{and} \quad V_{d+1} < V_{max}$$

This sequence proves that the interval $[c, d]$ is the true plateau of maximum volume and that volume strictly decreases outside of it, effectively locking the market-clearing price. 

After successfully proving the three said conditions, the verifier is sure that the prover is truthful.

--------------------

### **(Do we even need to mention this check? Because we have one for plateau check)4. The Positive Check ZKP Equations**

To prove that a value $V_{max}$ is the maximum in $\mathsf {Min}(x)$, we must prove that for every price tick $i$, $V_{max} - \mathsf{Min}_i \geq 0$. Keeping in mind that $V_{max}$ will eventually be the clearing price $p^*$.
Following the IZPR approach for range proofs, we construct a difference polynomial  $P(x)$, representing the excess capacity or the amount by which a specific price tick failed to reach the global maximum volume. If $P(x)$ is 0 at a certain point, that price tick is part of the clearing plateau. If $P(x)\geq 0$ is always holding, it proves that no volume in the entire order book is greater than our claimed $V_{max}$. we define the constraint system for a commitment to $P(x)$ such that values are non-negative: (i.e., $12-\mathsf{Min}$ pos. check?)

$$P(x) - \sum_{j=0}^{k-1} 2^j \cdot B_j(x) = 0$$

Where:

$B_j(x)$ are boolean polynomials representing the bit-decomposition of the values. A negative result like $-1$ simply wraps around to a massive positive number (the field prime). To prove a number is small and positive (e.g., between $0$ and $2^{32}$?), we must prove it can be represented as a sum of bits. It forces the prover to break down the hidden value into binary.
**Boolean Constraint**: $B_j(x) \cdot (B_j(x) - 1) = 0$ must hold for all $j$. This forces every $B_j$ at every price tick to be exactly 0 or 1. 
**Vector Definition**: $V_{check}(x) = V_{max} - \mathsf{Min}(x) = P(x)$.

If the auctioneer lies and $V_{max}$ is smaller than a volume in the book, then $P_{\mathsf {Min}}(x)$ would be a negative number. In the SNARK's field, that negative number is so large it cannot be represented by only 32 bits, and the equation will fail to vanish. This proves $V_{max}$ is the true maximum.

### **5. Tie-breaking and Surplus**

If multiple prices result in $V_{max}$, the surplus polynomial is computed to resolve ties:

$P_{\mathsf{Surplus}}(x) = P_{\mathsf{Acc}_A}(x) - P_{\mathsf{Acc}_B}(x)$.    

The sign of the surplus indicates whether bids or asks predominate at a specific price tick. The final market-clearing price $p^*$ is typically the midpoint $\frac{c+d}{2}$ or a price within the interval where surplus is minimized. To check the surplus is minimized, we do the same thing in section 3.1.2, but in reverse, since we want the minimum number instead of the maximum. We define $v$ as $V_{min}$ in our surplus column and take $C(x) = R(P_{\mathsf {Surplus}(x)} - v)$ as our vanishing polynomial.

### **6. Conclusion**

This protocol successfully proves the market-clearing volume and price by demonstrating that $V_{max}$ is a global maximum achieved at the identified price ticks. Future iterations will refine tie-breaking using surplus as a secondary tie-breaker to enhance profitability and market efficiency.
