# Appendix B. Constraint relations

Here $X^+=\omega X$, $Y^+=Y(\omega X)$ and $t=X-\omega^{n-1}$. The predicate $\mathrm{rng}_b(Y)$ denotes reconstruction from $b$ bounded bits represented in limbs. It is applied on all rows unless gated. $L_i$ selects one row, $Z$ selects padding, $K_P$ selects the plateau, $K_C$ selects its neighbours and $K_T$ selects the predecessor of the proposed clearing tick when the tie-break requires it, as specified in [Section 3](03-protocol.md). Write $D^o,S^o$ for eligible IO depths, $m$ for ordinary executable volume, $u,a$ for bid and ask surpluses, $e$ for the shared slack, and $\nu=[v>0]$. F1–F33 are labels for groups of checks. F20 has separate bid and ask pins, and F23 expands into several lookup identities. Each equality in the table holds on $H$; a selector makes it active only on the specified rows.

| Group | Row relation or check |
| --- | --- |
| F1–F4 | $\mathrm{rng}_{k_{\rm in}}(B),\mathrm{rng}_{k_{\rm in}}(A)$ and $ZB=ZA=0$. |
| F5–F8 | $L_{n-1}(D-B)=0$, $L_0(S-A)=0$, $t(D-D^+-B)=0$, $t(S^+-S-A^+)=0$. |
| F9–F13 | $u=D-m$, $a=S-m$, $ua=0$, $\mathrm{rng}_k(u),\mathrm{rng}_k(a)$. |
| F14–F17 | $\mathrm{rng}_k(V_{\max}-m)$, $L_c(m-V_{\max})=L_d(m-V_{\max})=0$, $K_C(V_{\max}-m-1-e)=0$. |
| F18–F22 | $\delta=u+a$, $\mathrm{rng}_k(\delta-v)$ on $K_P$, $L_{p^\star}(u-(1-s)v)=L_{p^\star}(a-sv)=0$, $K_T(\delta-v-1-e)=0$, $\mathrm{rng}_k(e)$. |
| F23 | One logarithmic-derivative lookup for all reconstructed limbs and scaled top limbs. |
| F24–F27 | F1–F4 applied to the two IO columns. |
| F28–F31 | F28 and F29 initialize and accumulate IO demand $(B^o,D^o)$. F30 and F31 initialize and accumulate IO supply $(A^o,S^o)$. |
| F32–F33 | $\nu L_{p^\star}((1-s)B+sA-P)=0$ and $\nu L_{p^\star}((1-s)S^o+sD^o-J)=0$. |

The public checks are $0\leq c\leq p^\star\leq d<N$, $1\leq V_{\max}<2^k$, $s\in\{0,1\}$, $0\leq v\leq P<2^k$, $0\leq J<2^k$ and $v=0\Rightarrow s=P=J=0$. The left neighbour is tested when $c>0$ and the right neighbour when $d+1<n$. If $d=N-1<n-1$, the right neighbour is the first padding row, where executable volume is zero. For the priority check, $K_T=L_{p^\star-1}$ only when $p^\star>c$ and either $s=1$ or $v=0$; otherwise $K_T=0$. The cliff rows lie outside the plateau and the predecessor lies inside, so the slack checks can share a column. For a no-cross close use F1–F8, the ordinary range lookup and $D_iS_i=0$ everywhere.

## Lookup

We use Haböck’s logarithmic-derivative identity ([19](references.md#ref-19)) to test membership of our range limbs in the public table. The pairing of lookup terms below is the layout used by our clearing argument.

Let $f_1,\ldots,f_E$ be all looked-up expressions and let $T_i$ be the table value (zero on unused rows). The first round fixes these columns and the multiplicity column $M_T$. For a complete pair put $x=\beta-f_{2j-1}$, $y=\beta-f_{2j}$. On each row require $h_jxy=x+y$. With an odd number of expressions the last helper instead satisfies $h_{\lceil E/2\rceil}(\beta-f_E)=1$. A table helper satisfies $U(\beta-T)=M_T$, and a cyclic running sum satisfies $L^+-L=\sum_jh_j-U$. Summing the recurrence around the domain gives, away from poles,

$$
\sum_{i=0}^{n-1}\sum_{e=1}^E\frac{1}{\beta-f_e(\omega^i)}
=\sum_{i=0}^{n-1}\frac{M_T(\omega^i)}{\beta-T_i}.
$$
 The running sum has a free additive constant. Only its cyclic differences enter the check. An out-of-table value would give a pole on the left with a positive integer multiplicity and no matching pole on the right. The bound $En<|\mathbb{F}|$ keeps this multiplicity nonzero in the field. Since the commitments precede $\beta$, an incorrect rational identity can agree at the sampled challenge only exceptionally, including when a denominator vanishes. The benchmark layout has $E=51$ and hence 25 complete pairs and one singleton.

## Update histogram

On the update domain let $P_\eta$ equal $\eta^j$ on step rows and zero on unused rows. A selector $S_H$ marks the embedded price rows, and $X_H$ equals their price-domain label on those rows and zero elsewhere. The update proof checks the quantity range with its own domain's limb width. This range check is U1. U2–U4 are, in order,

$$
\begin{aligned}
G(\beta-T_{\rm step})&=P_\eta\,\mathrm{Qty},\\
 U_H(\beta-X_H)&=S_H W,\\
 L_2(\kappa X)-L_2(X)&=G-U_H ,
\end{aligned}
$$

where $\kappa$ generates the update domain. Telescoping yields [Eq. (2)](04-committed-book.md#eq-hist). The verifier binds the commitment for $W$ to the logged differences, including the proved empty mask. Extraction for these externally supplied commitments is an additional assumption. Polynomial coincidence estimates for individual challenges are insufficient to assign an end-to-end security level to the implementation.
