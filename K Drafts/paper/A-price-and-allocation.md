# Appendix A. Existence and uniqueness of the price

For a crossing book, define a candidate as a tick in $[c,d]$ with $\delta_i=v$ and either $v=0$ or at least $v$ units of long-side ordinary volume at that tick. We prove the two facts behind the local test in [Section 3](03-protocol.md).

## Existence

If $v=0$ every balanced minimizer qualifies. Suppose $v>0$ and there is a demand-long minimizer. Let $j$ be the highest such tick. Then $D_j=V_{\max}+v$ and $S_j=V_{\max}$. If $j=d$ and the next row exists, $S_{j+1}\geq V_{\max}$ and $V_{j+1}<V_{\max}$, so $D_{j+1}<V_{\max}$. Thus $B_j=D_j-D_{j+1}>v$. At the upper grid boundary take $D_{j+1}=0$ with the same conclusion. If $j<d$, monotonicity of $g$ and minimality of $v$ imply that $j+1$ is supply long. A demand-long neighbour there would have $g_{j+1}=-v$ and contradict the choice of $j$. Hence $D_{j+1}=V_{\max}$ and $B_j=v$. In either case $j$ qualifies. If all minimizers are supply long, the symmetric argument at the lowest one proves existence.

## Why the predecessor check is complete

With $v>0$, only the highest demand-long minimizer and the lowest supply-long minimizer can qualify. On a sequence of equal negative minimizers, $g_{i+1}-g_i=A_{i+1}+B_i=0$, so all but the last have $B_i=0$. The positive case is symmetric. Suppose the lowest qualifying tick $p^\star>c$ is supply long and $\delta_{p^\star-1}=v$. If $g_{p^\star-1}=v$, then $A_{p^\star}=0$, contradicting qualification. If $g_{p^\star-1}=-v$, the demand depth falls from $V_{\max}+v$ to $V_{\max}$, so $B_{p^\star-1}=v$ and the predecessor qualifies. Both are contradictions. At balance, a predecessor with imbalance zero would itself qualify. Thus the lowest candidate satisfies the strict predecessor check whenever it is active. Conversely, [Section 3](03-protocol.md) shows that an accepted candidate with this check has no lower candidate. There is exactly one crossing receipt per valid book.

## Why lowest qualifying is a policy choice

Consider two ticks, numbered 0 and 1, with ordinary bids $(2,5)$, ordinary asks $(5,2)$, and no IO. At tick 0 demand and supply are $(7,5)$; at tick 1 they are $(5,7)$. Each price executes five units with imbalance two, and each has two units of long-side interest at the price. Either price would allow all strictly better ordinary orders to fill. Our lowest-qualifying rule selects tick 0. F21 rules out tick 1 because its predecessor has the same absolute imbalance. The requirement that rationing be feasible and the convention for choosing between feasible prices do different work.

## Fill conservation before rounding

If demand is long, ordinary sellers supply $V_{\max}$ and IO sellers supply $I$. Better-priced buyers take $V_{\max}+v-P$ and buyers at the price take $P-v+I$. Purchases and sales both equal $V_{\max}+I$. Since $P\geq v$ and $0\leq I\leq \min(v,J)$, both applicable rates lie in $[0,1]$. Reverse the roles for long supply. At balance the ordinary sides each trade $V_{\max}$. Nothing in this argument assigns indivisible residual units to individual orders.
