Polynomials defined over the evaluation domain $H = \{\omega^0, \dots, \omega^{n-1}\}$ where $Z_H(X) = X^n - 1$:

### **1. Accumulator Verification**

These prove that the supply and demand curves are correctly built from the private bid and ask arrays.

**Supply Accumulator Initialization ($V_{Acc\_A, 1}$):** Enforces that the supply starts at zero (or the first bid) at the lowest price tick.
$$V_{Acc\_A, 1}(X) = (Acc_A(X) - Arr_A(X)) \cdot \frac{Z_H(X)}{X - \omega^{n-1}} = 0$$

**Supply Recursive Sum ($V_{Acc\_A, 2}$):** Proves the cumulative volume at price $i$ is the previous volume plus current asks.
$$V_{Acc\_A, 2}(X) = (Acc_A(X) - Arr_A(X) + Acc_A(\omega \cdot X)) \cdot (X - \omega^{n-1}) = 0$$

**Demand Accumulator Initialization ($V_{Acc\_B, 1}$):** Enforces that the demand starts at the highest price tick.
$$V_{Acc\_B, 1}(X) = (Acc_B(X) - Arr_B(X)) \cdot \frac{Z_H(X)}{X - \omega^0} = 0$$

**Demand Recursive Sum ($V_{Acc\_B, 2}$):** Proves cumulative volume sums from highest to lowest price.

$$V_{Acc\_B, 2}(X) = (Acc_B(X) - Arr_B(X) + Acc_B(\omega^{-1} \cdot X)) \cdot (X - \omega^0) = 0$$


---

### **2. Minimum Selection and Ceiling**

These ensure that the cleared volume is strictly the lesser of supply and demand and never exceeds the claimed maximum.

**Mutual Exclusivity ($V_{KL}$):** Proves $Min(X)$ is exactly equal to either the supply or the demand at every tick.
$$V_{KL}(X) = (Acc_A(X) - Min(X)) \cdot (Acc_B(X) - Min(X)) = 0$$
**Global Maximum Ceiling ($V_{ceiling}$):** Proves no volume in the book is higher than $V_{max}$ using bit-decomposition.
$$V_{ceiling}(X) = Mask_{plateau} \cdot \left( (V_{max} - Min(X)) - \sum_{j=0}^{k-1} 2^j \cdot B_j(X) \right) = 0$$

---

### **3. Plateau Boundaries (Cliffs)**

These lock the optimal clearing interval $[c, d]$ by proving the volume is exactly $V_{max}$ inside and strictly lower outside.

**Plateau Start/End ($V_{plateau\_c}, V_{plateau\_d}$):** Binds the minimum volume at indices $c$ and $d$ to the public $V_{max}$.

$$(Min(X) - V_{max}) \cdot Mask_{c}(X) = 0$$ and $$(Min(X) - V_{max}) \cdot Mask_{d}(X) = 0$$

**Left Cliff with Slack ($V_{cliff\_L}$):** Proves the volume at $c-1$ is strictly less than $V_{max}$ by at least 1 unit.

$$V_{cliff\_L}(X) = (V_{max} - Min(X) - 1 - Slack_L) \cdot Mask_{c-1}(X) = 0$$

**Right Cliff with Slack ($V_{cliff\_R}$):** Proves the volume at $d+1$ is strictly less than $V_{max}$.

$$V_{cliff\_R}(X) = (V_{max} - Min(X) - 1 - Slack_R) \cdot Mask_{d+1}(X) = 0$$


**Plateau within a plateau?** In standard call markets, a plateau within a plateau (where the clearing volume drops and then rises again) is impossible. The reason being that $Acc_A$ is monotonic (non-decreasing) and $Acc_B$ is monotonic (non-increasing). The intersection of an increasing function and a decreasing function, which defines the cleared volume is always a single unimodal peak or a single contiguous plateau. 
However, a Surplus Plateau can exist inside a Volume Plateau. This happens when multiple prices have the exact same maximum volume AND the same surplus (e.g., both supply and demand are flat across a tick).


---

### **4. Surplus and Tie-breaking**

These identify the optimal clearing price $p^*$ within the plateau by minimizing imbalance.

**Surplus Definition ($V_{surp\_def}$):** Defines the imbalance vector as the difference between supply and demand.
$$V_{surp\_def}(X) = Surplus(X) - (Acc_A(X) - Acc_B(X)) = 0$$

**Global Maximum Floor ($V_{floor}$):** Proves no volume in the book is lower than $V_{min_surp}$ using bit-decomposition.
$${V}_{floor}(X) = {Mask}_{plateau}(X) \cdot \left( (P_{Surplus}(X) - V_{min\_surp}) - \sum_{j=0}^{k-1} 2^j \cdot B_{j, surp}(X) \right) = 0$$
**Surplus Minimization ("Valley" Proof):** Proves the clearing price has the smallest absolute imbalance in the plateau.

$$V_{surp}(X) = (|Surplus(X)| - V_{min\_surp} - Slack_{surp}) \cdot Mask_{plateau}(X) = 0$$
---

### **5. Foundational Integrity (Bit Checks)**

These ensure the auxiliary variables used for range proofs are valid.

**Bit Booleanity ($V_{bit}$):** Enforces that bit-decomposition polynomials ($B_j$) and slack variables are exactly 0 or 1.
$$V_{bit, j}(X) = B_j(X) \cdot (B_j(X) - 1) = 0$$