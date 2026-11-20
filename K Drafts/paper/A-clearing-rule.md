# A The Clearing Rule

We state the clearing rule of Section 2.1 as a function $\mathsf{Clear}$ from books to receipts. Theorems 1 and 2 (Appendix C) are stated against this definition.

## A.1 Definition

A *book* on $n$ ticks, of which the first $N$ are real, is four sequences of integers $b_i, a_i, b^{io}_i, a^{io}_i$, $0 \le i < n$, with every entry in $[0, 2^{k_{in}})$ and zero for $i \ge N$. $b_i$ is the bid volume with limit exactly tick $i$ and $a_i$ the ask volume; a market buy is recorded at tick $N-1$ and a market sell at tick 0. The $io$ sequences hold the imbalance-only orders. For each tick define

$$D_i = \sum_{j \ge i} b_j, \qquad S_i = \sum_{j \le i} a_j, \qquad V_i = \min(D_i, S_i), \qquad g_i = S_i - D_i, \qquad \delta_i = |g_i|,$$

and likewise $D^{io}_i = \sum_{j \ge i} b^{io}_j$ and $S^{io}_i = \sum_{j \le i} a^{io}_j$. $D_i$ is the demand willing to trade at price $i$, $S_i$ the supply, and $V_i$ the volume that can execute. $D$ is non-increasing; $S$ and $g$ are non-decreasing. Tick $i$ is *demand long* if $g_i < 0$, *supply long* if $g_i > 0$, and *balanced* if $g_i = 0$. Its *at-price long volume* is $\lambda_i = b_i$ if it is demand long and $\lambda_i = a_i$ if it is supply long.

**Definition 2 ($\mathsf{Clear}$).** Let $V = \max_i V_i$. If $V = 0$, $\mathsf{Clear}(\text{book}) = \textsf{no-cross}$. Otherwise let

- $\Pi = \{i : V_i = V\}$ be the *plateau*, with $c = \min \Pi$ and $d = \max \Pi$;
- $v = \min_{i \in \Pi} \delta_i$ be the minimum imbalance;
- a *candidate* be a tick $i \in \Pi$ with $\delta_i = v$ and ($v = 0$ or $\lambda_i \ge v$);
- $p^*$ be the least candidate.

Then $\mathsf{Clear}(\text{book}) = (V, c, d, p^*, v, s, P, J)$, where $s = P = J = 0$ if $v = 0$, and otherwise $s = [g_{p^*} > 0]$, $P = \lambda_{p^*}$, and $J = S^{io}_{p^*}$ if $s = 0$, $J = D^{io}_{p^*}$ if $s = 1$.

## A.2 Properties

**The plateau is an interval.** For $i < j < l$ we have $V_j \ge \min(V_i, V_l)$, because $D_j \ge D_l \ge V_l$ and $S_j \ge S_i \ge V_i$. Hence $\Pi = \{c, \ldots, d\}$.

**A candidate exists.** This is Lemma C.1, so $\mathsf{Clear}$ is defined for every book.

**At most two candidates when $v > 0$.** On $\Pi$, $g$ is non-decreasing and takes the values $-v$ and $+v$ on the minimizers. Let $j$ be a demand-long minimizer that is not the highest one. Then $g_{j+1} = g_j$, and since $g_{j+1} - g_j = a_{j+1} + b_j$, we get $b_j = 0 < v$: $j$ is not a candidate. By symmetry only the lowest supply-long minimizer can be a candidate. The two candidates are therefore the highest demand-long minimizer and the lowest supply-long minimizer, and the first, when it exists, is the lower tick. When $v = 0$ every balanced plateau tick is a candidate and $p^*$ is the lowest.

**Why the candidate condition.** The rule must be able to fill every better-priced order even if there is no eligible IO volume. In that case all $v$ unfilled units must come from the long side at $p^*$, requiring $\lambda_{p^*} \ge v$. At a candidate, Proposition C.4 gives feasible fill ratios for every $J$.

## A.3 Fills

Let $I = \min(J, v)$ be the admitted imbalance-only volume and $R = v - I$ the residual. Every trader computes their own fill from the receipt (Table 8). The traded volume is $V + I$. Fills are integers; the rounding of the two ratios is a venue rule outside the proof.

**Table 8.** The fill of an order of quantity $q$, given the receipt.

| Order | Fill |
|---|---|
| Limit or market order strictly better than $p^*$, or on the short side at $p^*$ | $q$ |
| Limit order at exactly $p^*$ on the long side | $q \cdot (P - R)/P$ |
| Eligible imbalance-only order on the short side, when $J>0$ | $q \cdot I/J$ |
| Any other order | 0 |

## A.4 The Running Example

Table 9 gives the quantities of Definition 2 for the book of Figure 2.

**Table 9.** The book of Figure 2, tick by tick.

| Tick $i$ | 0 | 1 | 2 | 3 | 4 | 5 | 6 | 7 |
|---|---|---|---|---|---|---|---|---|
| $b_i$ | 0 | 0 | 3 | 0 | 0 | 10 | 2 | 0 |
| $a_i$ | 0 | 4 | 6 | 0 | 0 | 0 | 5 | 3 |
| $a^{io}_i$ | 0 | 0 | 0 | 0 | 1 | 0 | 0 | 0 |
| $D_i$ | 15 | 15 | 15 | 12 | 12 | 12 | 2 | 0 |
| $S_i$ | 0 | 4 | 10 | 10 | 10 | 10 | 15 | 18 |
| $V_i$ | 0 | 4 | 10 | 10 | 10 | 10 | 2 | 0 |
| $g_i$ | $-15$ | $-11$ | $-5$ | $-2$ | $-2$ | $-2$ | 13 | 18 |

$V = 10$ and $\Pi = \{2, \ldots, 5\}$. On the plateau $\delta$ is $5, 2, 2, 2$, so $v = 2$ with minimizers 3, 4, 5, all demand long. Ticks 3 and 4 have $\lambda_i = b_i = 0 < 2$, and tick 5 has $\lambda_5 = 10 \ge 2$: the only candidate is $p^* = 5$. Then $s = 0$, $P = 10$, and $J = S^{io}_5 = 1$, so $I = 1$, $R = 1$. The asks at or below tick 5 sell 10 and the IO order sells 1. The bids at tick 6 buy 2 and the bids at tick 5 buy $10 \cdot 9/10 = 9$. Both sides trade 11.

## A.5 Relation to the Nasdaq Closing Cross

Nasdaq's rule selects the cross price in ordered steps [Nasdaq 4754(b)(2)]: (A) the price that maximizes the shares executed; (B) if several, the price that minimizes the imbalance; (C) if several, the entered price at which shares will remain unexecuted; (D) if several, the price closest to the midpoint of the continuous market's best bid and offer. Table 10 relates our rule to these steps.

**Table 10.** The steps of the Closing Cross and of $\mathsf{Clear}$.

| Closing Cross | $\mathsf{Clear}$ | Difference |
|---|---|---|
| (A) maximize executed shares | $V$ and the plateau $\Pi$ | None |
| (B) minimize the imbalance | $v$ and the minimizers | None |
| (C) the entered price at which shares remain unexecuted | Candidate: the long side's volume at the tick covers the imbalance | We state the step as the inequality $\lambda_i \ge v$ |
| (D) closest to the bid-ask midpoint | The least candidate | A batch market has no continuous quote to refer to |

Three further differences come from the setting. The Closing Cross also draws on orders resting in the continuous book; a batch auction has none. We admit imbalance-only orders after the price is set, so they cannot move it. Rule 4754(b)(3) allocates at the cross price by order type and time priority; we ration the long side pro rata, since a batch has no time priority [Budish et al.].
