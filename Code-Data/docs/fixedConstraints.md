# Auction constraints

A batch contains $N$ real ticks. The evaluation domain $H=\{1,\omega,\ldots,\omega^{n-1}\}$ has $n=\max(2,2^{\lceil\log_2 N\rceil})$ points. Each identity holds on $H$. The prover combines the identities and divides by $Z_H=X^n-1$. Use $B,A,IOB,IOA$ for the four volume columns. Ordinary demand accumulates from high ticks to low ticks and ordinary supply from low to high. The two IO columns enter the crossing receipt only after the ordinary price is selected.

The crossing receipt contains $(V_{max},c,d,p^*,V_{min\Delta},s,P,J)$. The first three entries specify maximum ordinary volume and its plateau. The next three identify the selected tick, minimum imbalance and long side (0 for demand, 1 for supply when the imbalance is positive). $P$ is ordinary long volume at the selected tick and $J$ is eligible short-side IO volume. At balance, $s=P=J=0$. For positive imbalance, admitted IO contributes $\min(J,V_{min\Delta})$ units beyond $V_{max}$.

$L_i$ selects row $i$. $Pad$ selects rows $N$ through $n-1$, $Mask_P$ selects $[c,d]$, and $Mask_C$ selects its two immediate neighbours when present on the domain. $Mask_T$ selects $p^*-1$ only when $p^*>c$ and either supply is long or imbalance is zero. Set $\nu=1$ for a positive imbalance and 0 otherwise. With $w=\min(8,\log_2 n)$, let $m_{in}=\lceil k_{in}/w\rceil$ and $m=\lceil k/w\rceil$, where $k=k_{in}+\lceil\log_2 N\rceil\le62$.

## Public receipt

The verifier first checks the public receipt against the following bounds and conventions.

$$0 \le c \le p^* \le d \le N-1, \qquad 1 \le V_{max} < 2^k, \qquad 0 \le V_{min\Delta} < 2^k,$$

$$s \in \{0,1\}, \qquad V_{min\Delta} \le P < 2^k, \qquad 0 \le J < 2^k, \qquad V_{min\Delta} = 0 \Rightarrow s = P = J = 0.$$

## Clearing constraints

| # | Name | Identity | Active at |
|---|---|---|---|
| F1 | Bid range | $B - \sum_{l<m_{in}} 2^{wl} \ell^{B}_l = 0$ | All ticks |
| F2 | Ask range | $A - \sum_{l<m_{in}} 2^{wl} \ell^{A}_l = 0$ | All ticks |
| F3 | Bid padding | $Pad \cdot B = 0$ | Padding |
| F4 | Ask padding | $Pad \cdot A = 0$ | Padding |
| F5 | Demand start | $L_{n-1} \cdot (AccB - B) = 0$ | $\omega^{n-1}$ |
| F6 | Supply start | $L_0 \cdot (AccA - A) = 0$ | $\omega^0$ |
| F7 | Demand step | $(X - \omega^{n-1})\,[AccB(X) - AccB(\omega X) - B(X)] = 0$ | All but the last |
| F8 | Supply step | $(X - \omega^{n-1})\,[AccA(\omega X) - AccA(X) - A(\omega X)] = 0$ | All but the last |
| F9 | Bid surplus | $SurpB - (AccB - Min) = 0$ | All ticks |
| F10 | Ask surplus | $SurpA - (AccA - Min) = 0$ | All ticks |
| F11 | Minimum selection | $SurpA \cdot SurpB = 0$ | All ticks |
| F12 | $SurpB \ge 0$ | $SurpB - \sum_{l<m} 2^{wl} \ell^{sB}_l = 0$ | All ticks |
| F13 | $SurpA \ge 0$ | $SurpA - \sum_{l<m} 2^{wl} \ell^{sA}_l = 0$ | All ticks |
| F14 | Ceiling | $(V_{max} - Min) - \sum_{l<m} 2^{wl} \ell^{ceil}_l = 0$ | All ticks |
| F15 | Plateau, left end | $L_c \cdot (Min - V_{max}) = 0$ | $\omega^c$ |
| F16 | Plateau, right end | $L_d \cdot (Min - V_{max}) = 0$ | $\omega^d$ |
| F17 | Cliffs | $Mask_C \cdot (V_{max} - Min - 1 - Slack) = 0$ | $\omega^{c-1}$, $\omega^{d+1}$ where they exist |
| F18 | Imbalance | $\Delta - (SurpA + SurpB) = 0$ | All ticks |
| F19 | Imbalance floor | $Mask_P \cdot [(\Delta - V_{min\Delta}) - \sum_{l<m} 2^{wl} \ell^{flr}_l] = 0$ | Plateau |
| F20a | Pin, demand side | $L_{p^*} \cdot (SurpB - (1-s)\,V_{min\Delta}) = 0$ | $\omega^{p^*}$ |
| F20b | Pin, supply side | $L_{p^*} \cdot (SurpA - s\,V_{min\Delta}) = 0$ | $\omega^{p^*}$ |
| F21 | Tie-break | $Mask_T \cdot (\Delta - V_{min\Delta} - 1 - Slack) = 0$ | $\omega^{p^*-1}$ when $Mask_T \ne 0$ |
| F22 | $Slack \ge 0$ | $Slack - \sum_{l<m} 2^{wl} \ell^{slk}_l = 0$ | All ticks |
| F23 | Table membership | The lookup identities below over all looked-up values | All ticks |
| F24 | IO bid range | $IOB - \sum_{l<m_{in}} 2^{wl} \ell^{IOB}_l = 0$ | All ticks |
| F25 | IO ask range | $IOA - \sum_{l<m_{in}} 2^{wl} \ell^{IOA}_l = 0$ | All ticks |
| F26 | IO bid padding | $Pad \cdot IOB = 0$ | Padding |
| F27 | IO ask padding | $Pad \cdot IOA = 0$ | Padding |
| F28 | IO demand start | $L_{n-1} \cdot (IOAccB - IOB) = 0$ | $\omega^{n-1}$ |
| F29 | IO demand step | $(X - \omega^{n-1})\,[IOAccB(X) - IOAccB(\omega X) - IOB(X)] = 0$ | All but the last |
| F30 | IO supply start | $L_0 \cdot (IOAccA - IOA) = 0$ | $\omega^0$ |
| F31 | IO supply step | $(X - \omega^{n-1})\,[IOAccA(\omega X) - IOAccA(X) - IOA(\omega X)] = 0$ | All but the last |
| F32 | At-price long volume | $\nu\, L_{p^*} \cdot ((1-s)\,B + s\,A - P) = 0$ | $\omega^{p^*}$ when $V_{min\Delta} > 0$ |
| F33 | Eligible IO volume | $\nu\, L_{p^*} \cdot ((1-s)\,IOAccA + s\,IOAccB - J) = 0$ | $\omega^{p^*}$ when $V_{min\Delta} > 0$ |

F20 is a pair, so the list has 33 identities besides the lookup. The looked-up values are the limbs of the nine range-checked quantities ($B$, $A$, $IOB$, $IOA$, $SurpB$, $SurpA$, $V_{max} - Min$, $\Delta - V_{min\Delta}$, $Slack$) and, for a width that is not a multiple of $w$, the scaled top limb of each. Their number is $E = 4 m_{in} + 5m$, plus 4 if $w \nmid k_{in}$, plus 5 if $w \nmid k$. F23 consists of $\lceil E/2 \rceil$ helper identities (one per complete pair and one singleton when $E$ is odd), the table-helper identity, and the running-sum identity.

**Column count.** The first round commits to 13 value columns, $4m_{in}+5m$ limb columns, and $Mult$. The second adds $\lceil E/2\rceil+2$ helper columns. At $n=1024$, $k_{in}=32$, and $k=42$, we have 46 limb columns and $E=51$ lookup inputs. The 25 paired helpers and one singleton bring the total, including the other helpers, to 88 columns.

**Degrees and openings.** The lookup pair equations are cubic in witness columns. Vanishing-polynomial masking raises their degree above the unmasked bound of $3(n-1)$. The prover caps the interpolated quotient at degree $2n+8$ for this layout. $AccB$, $AccA$, $IOAccB$, $IOAccA$, $A$, $IOA$ and $S$ are opened at both $\zeta$ and $\omega\zeta$. The other columns are opened at $\zeta$.

### Lookup equations (F23)

We prove table membership for all limbs of all nine quantities with one logarithmic-derivative lookup. Let $f_1, \ldots, f_E$ be the looked-up columns and $T$ the table column. The prover commits to a multiplicity column $Mult$, where $Mult[v]$ counts the cells equal to $v$. After a challenge $\beta$, it commits to helper columns defined on $H$ by

$$h_j = \frac{1}{\beta - f_{2j-1}} + \frac{1}{\beta - f_{2j}}, \qquad U = \frac{Mult}{\beta - T}, \qquad S[i+1] = S[i] + \sum_j h_j[i] - U[i].$$

For each complete pair we impose the first identity below. If $E$ is odd, the last helper instead satisfies $h_{\lceil E/2\rceil}(\beta-f_E)=1$. In the 51-input configuration this gives 25 paired helpers and one singleton.

$$h_j \,(\beta - f_{2j-1})(\beta - f_{2j}) - (\beta - f_{2j-1}) - (\beta - f_{2j}) = 0, \qquad U\,(\beta - T) - Mult = 0,$$

$$S(\omega X) - S(X) - \Big(\sum_j h_j(X) - U(X)\Big) = 0. \tag{F23}$$

The running sum wraps from $\omega^{n-1}$ to 1. Summing its differences over $H$ cancels every term of $S$, yielding

$$\sum_{i=0}^{n-1} \sum_{e=1}^{E} \frac{1}{\beta - f_e[i]} = \sum_{i=0}^{n-1} \frac{Mult[i]}{\beta - T[i]}.$$

As an identity of rational functions in $\beta$, this holds only if every $f_e[i]$ is a table value. The limbs and $Mult$ are committed before $\beta$ is drawn, so a prover with a cell outside the table passes with probability at most $(2E+2)\,n/|\mathbb{F}|$ for an ideal random challenge. The running sum needs no initial condition, since only its differences are constrained.

## No-cross constraints

The no-cross proof uses only ordinary bids and asks, their depth columns, the limbs of $B$ and $A$, and the corresponding lookup columns. We impose F1–F8, apply F23 to those limbs, and require

$$AccA(X) \cdot AccB(X) = 0.$$

## Update validity

For update validity, let $\kappa$ generate the domain $K$ of size $M$, with $\kappa^{M/n}=\omega$. The public polynomial $P_\eta$ takes value $\eta^j$ on the first $\mu$ rows, one per logged step, and zero elsewhere. On row $iM/n$, $X_H$ equals $\omega^i$ and $S_H$ equals one. Both vanish on the other rows. The update domain is a power of two with $M\ge\max(n,\mu)$. For U1 use $w=\min(8,\log_2 M)$ and $m_{in}=\lceil k_{in}/w\rceil$; an incomplete top limb has the same scaled lookup as in F23. A nonzero step is constrained to a point of $H$, which includes padding when $N<n$. The witness includes $Qty$, $Tick$, $W$, quantity limbs and their lookup columns. The prover commits to $G$, $U_H$, and $S_2$ after receiving $\beta$.

| # | Name | Identity |
|---|---|---|
| U1 | Quantity range | $Qty - \sum_{l<m_{in}} 2^{wl} \ell^{Qty}_l = 0$, with every limb in the table |
| U2 | Step side | $G(X)\,(\beta - Tick(X)) - P_\eta(X)\,Qty(X) = 0$ |
| U3 | Tick side | $U_H(X)\,(\beta - X_H(X)) - S_H(X)\,W(X) = 0$ |
| U4 | Running sum | $S_2(\kappa X) - S_2(X) - (G(X) - U_H(X)) = 0$ |

The verifier takes the ordered differences $\Phi_j$ from the log and checks the empty mask commitment $\Phi_{mask}$. After the prover commits to the quantities and tick labels, the transcript derives $\eta$. The verifier computes $C_W=\Phi_{mask}+\sum_{j<\mu}\eta^j\Phi_j$ and checks it against the proof's commitment for $W$. U2–U4 compare the weighted quantities of the logged steps with the evaluations of this same polynomial on $H$ through a running sum at challenge $\beta$.

For 32-bit quantities and eight-bit limbs the proof has 15 columns: $Qty$, $Tick$, $W$, four limbs, the multiplicities, two pair helpers, the table helper, the range running sum, $G$, $U_H$, and $S_2$. With the quotient and $\Phi_{mask}$ it holds 17 commitments.


See the [clearing-rule proof](CORRECTNESS_PROOF.md) for candidate existence and uniqueness and the [log and receipt analysis](SECURITY_BOOK.md) for the update statement.
