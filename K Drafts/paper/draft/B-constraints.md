# B The Full Constraint List

*Draft in Markdown. Label: `app:constraints`.*

Notation is that of Section 3.2. Each identity holds on all of $H$; the last column says where it is not trivially satisfied. $w$ is the limb width ($w = 8$ for $n \ge 256$), $m_{in} = \lceil k_{in}/w \rceil$, $m = \lceil k/w \rceil$, and $\nu = 1$ if $V_{min\Delta} > 0$, else 0.

## B.1 Scalar Checks

Before any polynomial check the verifier rejects the receipt unless

$$0 \le c \le p^* \le d \le N-1, \qquad 1 \le V_{max} < 2^k, \qquad 0 \le V_{min\Delta} < 2^k,$$

$$s \in \{0,1\}, \qquad V_{min\Delta} \le P < 2^k, \qquad 0 \le J < 2^k, \qquad V_{min\Delta} = 0 \Rightarrow s = P = J = 0.$$

## B.2 Clearing Constraints

**Table 11.** The constraints of the clearing proof.

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
| F23 | Table membership | The lookup identities of Section 3.4 over all looked-up values | All ticks |
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

F20 is a pair, so the list has 33 identities besides the lookup. The looked-up values are the limbs of the nine range-checked quantities ($B$, $A$, $IOB$, $IOA$, $SurpB$, $SurpA$, $V_{max} - Min$, $\Delta - V_{min\Delta}$, $Slack$) and, for a width that is not a multiple of $w$, the scaled top limb of each. Their number is $E = 4 m_{in} + 5m$, plus 4 if $w \nmid k_{in}$, plus 5 if $w \nmid k$. F23 consists of $\lceil E/2 \rceil$ pair identities, the table-helper identity, and the running-sum identity.

**Column count.** The first round has 13 value columns, $4m_{in} + 5m$ limb columns, and the multiplicity column $Mult$; the second has $\lceil E/2 \rceil + 2$. For $n = 1024$, $k_{in} = 32$, $k = 42$: 46 limb columns, $E = 51$, 26 helpers, 88 columns.

**Degrees and openings.** The pair identities have degree at most $3n - 3$ before masking and every other identity at most $2n - 2$. The quotient has degree at most $2n + 3$ with masking, and the implementation requests a reference string of degree $2n + 8$. $AccB$, $AccA$, $IOAccB$, $IOAccA$, $A$, $IOA$, and $S$ are opened at $\zeta$ and $\omega\zeta$; every other column at $\zeta$.

## B.3 No-Cross Constraints

The no-cross proof commits to $B$, $A$, $AccB$, $AccA$, the limbs of $B$ and $A$, and the lookup columns for those limbs. Its constraints are F1–F8, F23 restricted to those limbs, and

$$AccA(X) \cdot AccB(X) = 0.$$

## B.4 Update-Validity Constraints

The update-validity proof runs on the domain $K$ of size $M$ with generator $\kappa$, where $\kappa^{M/n} = \omega$. With $\mu$ the number of logged steps and $\eta$ the challenge of Section 4.3, its public polynomials are $P_\eta$ with $P_\eta[j] = \eta^j$ for $j < \mu$ and 0 elsewhere; $X_H$ with value $\omega^i$ at row $iM/n$ and 0 elsewhere; $S_H$, the indicator of the rows $iM/n$; and the range table. Its columns are $Qty$, $Tick$, $W$, the limbs of $Qty$, the lookup columns for those limbs, and three columns $G$, $U_H$, $S_2$ committed after $\beta$.

**Table 12.** The constraints of the update-validity proof, each on all of $K$.

| # | Name | Identity |
|---|---|---|
| U1 | Quantity range | $Qty - \sum_{l<m_{in}} 2^{wl} \ell^{Qty}_l = 0$, with every limb in the table |
| U2 | Step side | $G(X)\,(\beta - Tick(X)) - P_\eta(X)\,Qty(X) = 0$ |
| U3 | Tick side | $U_H(X)\,(\beta - X_H(X)) - S_H(X)\,W(X) = 0$ |
| U4 | Running sum | $S_2(\kappa X) - S_2(X) - (G(X) - U_H(X)) = 0$ |

For 32-bit quantities the proof has 15 columns: $Qty$, $Tick$, $W$, four limbs, the multiplicities, two pair helpers, the table helper, the range running sum, $G$, $U_H$, and $S_2$. With the quotient and $\Phi_{mask}$ it holds 17 commitments.
