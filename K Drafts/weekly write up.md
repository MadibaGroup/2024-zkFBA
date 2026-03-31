### **1. Abstract**

Frequent Batch Auctions (FBAs) have emerged as a robust solution for enhancing market stability and price discovery by aggregating order flow over discrete time intervals, effectively neutralizing predatory high-frequency trading. To guarantee that these auctions are executed with absolute fiduciary integrity, we leverage Zero-Knowledge SNARKs to provide succinct, verifiable proofs of computational honesty without compromising sensitive institutional data.
A significant challenge arises because FBAs typically operate as blind auctions to protect against front-running, creating a trust gap for traders who cannot verify if a fill was executed at the true market-clearing price. This research introduces a custom ZK-SNARK protocol that mathematically certifies that reported execution volume and price represent the true global equilibrium of the hidden order book, ensuring optimal execution while maintaining total bid confidentiality.

---
.......

 Zeeperio-style explanation about math prerequisites

  .......
  
___

### **Protocol Architecture & Mathematical Foundation**

The protocol is built on a Polynomial Interactive Oracle Proof (PIOP) framework using a multiplicative subgroup $H$ of size $n$.
Evaluation domain is $H = \{1, \omega, \omega^2, \dots, \omega^{n-1}\}$, where $\omega$ is the generator of the subgroup in the finite field $\mathbb{F}_q$. Here, we have the vanishing polynomial $Z_H(X) = X^n - 1$, which equals zero for all $X \in H$.
To enforce recurrence relations across price ticks, we utilize the transition constraint structure (Plonkbook):    $$(X - \omega^{n-1}) \cdot [ \text{Constraint Logic} ] = 0$$
This ensures the relation holds for all $i \in \{0, \dots, n-2\}$ while preventing an undefined "wrap-around" at the final domain point (enforcing recurrence relations (e.g., $A_{i+1} = A_i + B_i$) without causing a contradiction at the final domain point.).

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

- **Contiguity**: There is exactly one "start" and one "end" to the clearing price range. The auctioneer cannot "cherry-pick" non-adjacent prices to manipulate the volume.
    
- **Unimodality Alignment**: This shape proof aligns with the economic theory that the **Trade Vol** function is unimodal. The SNARK proves the reported plateau is the unique, contiguous peak of that function.
    
- **Integrity of the Valley**: Because the plateau is proved to be a single block, the subsequent **Surplus Valley** check is guaranteed to be searching within the correct, high-liquidity interval rather than a fragmented set of prices.



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





