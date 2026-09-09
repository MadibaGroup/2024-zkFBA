# Zeequent


**Keywords:** frequent batch auctions, polynomial IOPs, KZG commitments,
custom arithmetization, zero-knowledge proofs

---

## Abstract

Frequent batch auctions (FBAs) replace continuous limit-order matching with
periodic uniform-price clearing, reducing the payoff to the sub-second
latency advantages exploited by high-frequency traders [1,2]. An overview
of a zero-knowledge FBA protocol describes an exchange that commits to an
order book and proves — without revealing individual orders — that a
published clearing price and allocation satisfy the auction rule, but
leaves the cryptographic construction itself unspecified, deferring it to
an implementation repository. We supply that construction. We arithmetize
the three-phase FBA clearing rule — depth accumulation, plateau/tie-break
selection, and pro-rata allocation — as 33 vanishing polynomial identities
over a multiplicative subgroup, built from two reusable gadgets
(bit-decomposition non-negativity and a Plookup-style range membership
check) and verified via KZG10 commitments [7] with Fiat-Shamir
batching [9]. We give two independent implementations at different points
in the design space: a Rust/arkworks prover covering the core
accumulator-and-minimum constraints, and a full 33-constraint circuit in
Noir/Barretenberg proved at 100 and 1,000 price ticks. At 1,000 ticks the
Noir circuit compiles to 111,611 gates and produces a 14,656-byte proof in
0.45 seconds; we use these numbers to give the "not necessarily cheap
enough" concern raised by the overview paper a concrete answer instead of
an open question.

---

## 1 Introduction

FBAs, proposed by Budish, Cramton, and Shim [1,2], apply the logic of an
opening or closing auction — collect orders, then clear them together at a
single price — repeatedly throughout the trading day instead of once. This
removes the reward for sub-second latency that a continuous limit order
book (CLOB) pays out, since arriving a microsecond earlier than another
trader no longer changes execution order within a batch. The cost is
transparency: a CLOB shows every order as it arrives, but an FBA shows
only the aggregate outcome of a window that has already closed. A trader
observes whether their own order executed and at what price, but has no
way to independently check that the exchange included every eligible
order, excluded every ineligible one, and computed the clearing price
correctly.

A companion paper sketches a zero-knowledge answer to this problem: an
exchange that commits to its order book and, after each batch, publishes a
clearing receipt together with a proof that the receipt follows from the
committed book. That paper motivates the market design, states the
three-phase clearing rule, and names the gadget vocabulary the proof
would need — addition, subtraction, minimum, maximum, absolute value,
non-negativity, one-hot selection — but stops short of the arithmetization
itself, on the grounds that general-purpose zk-SNARK toolchains are too
expensive to run once per batch and a custom construction is needed
instead. This paper is that construction.

We do not build on a general-purpose proving framework. Producing a fresh
proof every batch — potentially once a second — rules out anything whose
constraint-generation overhead scales with the complexity of a
general-purpose circuit compiler; a bespoke polynomial IOP [3], in the
PLONK arithmetization style [4], lets us write exactly the 33 identities
the clearing rule requires and no more. We commit to each witness column
with KZG10 [7] and use two shared gadgets — one for every non-negativity
check the auction rule requires, one for every range check — rather than a
bespoke gadget per column, so that the constraint count grows with the
number of *distinct proof patterns* rather than the number of columns.

We evaluate this construction with two implementations that sit at
different points in the design space rather than one. A Rust/arkworks
prover implements the core accumulator-and-minimum constraints (Phase 1's
depth curves and the executable-volume computation) directly against the
`ark-poly-commit` KZG10 API, giving us a lower bound on proving cost when
only the volume-maximization step is proved. A Noir/Barretenberg circuit
implements the full 33-constraint specification — Phases 1 through 3,
plateau selection, tie-break, and cliff-exhaustiveness — compiled to
UltraHonk and proved at two data scales, 100 and 1,000 price ticks, to
show how cost scales with book depth. The two implementations are not a
head-to-head benchmark of the same workload; we report both because they
answer different questions — what a minimal accumulator proof costs, and
what the complete clearing rule costs at auction-realistic scale.

Our contributions are:

1. **A complete constraint specification for FBA clearing.** We express
   all three clearing phases — depth accumulation, plateau/tie-break
   selection via a valley pin, and cliff-exhaustiveness (proving the
   reported plateau is not a strict sub-interval of the true one) — as 33
   vanishing polynomial identities over a single evaluation domain,
   grouped by which of two shared gadgets each depends on.
2. **Two reusable gadgets in place of one gadget per column.** A
   bit-decomposition non-negativity gadget and a Plookup-style range-check
   gadget cover every inequality the clearing rule needs — order-size
   ceilings, surplus non-negativity, the delta floor, and cliff slack —
   without a separate proof pattern per use.
3. **A masking construction that avoids a witnessed permutation.** The
   plateau, valley, and cliff selectors (`Mask_P`, `Mask_V`, `Mask_C`) are
   computed by the verifier directly from the publicly disclosed clearing
   scalars ($c$, $d$, $p^*$) rather than committed as separate witness
   polynomials proved correct via a shuffle argument, at no additional
   disclosure: the clearing receipt already reveals these scalars
   regardless of which masking approach is used.
4. **Two independent implementations, at two different scopes.** A
   Rust/arkworks prover for the core accumulator constraints and a full
   33-constraint Noir/Barretenberg circuit, the latter proved at 100 and
   1,000 price ticks, with reported gate counts, proof sizes, and prover
   wall-clock time at each scale.

---

## 2 Background and Related Work

### 2.1 Frequent Batch Auctions

An FBA clears a double-sided order book at a single uniform price on a
fixed schedule, rather than matching orders one at a time as they arrive
[1,2]. We adopt the same three-phase clearing rule as the companion
overview paper: Phase 1 computes cumulative bid and ask depth at every
price tick and finds the maximum executable volume $V_{max}$; Phase 2
breaks ties among prices that all achieve $V_{max}$ by minimizing the
supply/demand imbalance; Phase 3 executes eligible orders at the resulting
clearing price $p^*$ and rations any residual imbalance pro-rata. We take
this rule as given and do not re-derive its market-design justification
here — we refer the reader to the companion paper and to Harris [6] for
the closing-auction mechanics it adapts. What we add is a way to prove
that an exchange executed this rule correctly on a committed, private
order book.

### 2.2 Zero-Knowledge Proofs and Polynomial IOPs

A zero-knowledge proof lets a prover convince a verifier that a statement
is true while revealing nothing beyond its truth [5]. We build on the
polynomial interactive oracle proof (PIOP) model [3], in which the prover
sends oracle access to a small number of polynomials and the verifier
checks a constant number of algebraic identities on them at a random
point. PLONK [4] instantiates this model with a fixed arithmetization
recipe — one wire-value polynomial per column, and constraints written as
low-degree polynomial identities that vanish on an evaluation domain — and
supplies a public catalogue of proof gadgets for common relations
(additions, boolean flags, range membership, permutations) that Plonkbook
[8] collects and names. We adopt PLONK's arithmetization style but not its
constraint set: FBA clearing needs its own gadgets (bit-decomposition
non-negativity, a plateau/valley masking argument), built in Section 3
from the same vanishing-polynomial machinery.

We deliberately do not build on a *universal* circuit compiler. Marlin
[10] and Sonic [11] both prove membership in the language of an arbitrary
R1CS/PLONK circuit supplied at proving time, at the cost of an indexing or
circuit-encoding step that runs on every proof; Bünz, Fisch, and
Szepieniec's transparent-SNARK compiler [3] additionally removes the
trusted setup, at a further verification-time cost. Because the clearing
rule is fixed in advance and identical for every batch, we get nothing
from the ability to prove an arbitrary circuit — we hand-encode the 33
constraints of Section 3 directly rather than compiling them through a
general front end, which is what lets a full-book proof run in well under
a second (Section 5) instead of paying universal-circuit overhead once
per batch.

### 2.3 Polynomial Commitments and Non-Interactivity

We commit to every witness polynomial with KZG10 [7]: a single
BN254 $\mathbb{G}_1$ point per polynomial, opened at a challenge point with
a constant-size pairing check regardless of the polynomial's degree. KZG10
requires a structured reference string from a trusted setup, which we
accept in exchange for its constant proof size — an FBA exchange
publishing a certificate once per batch benefits more from a small,
cheap-to-verify proof than from a transparent setup it only needs to run
once.

The base PIOP is interactive: the verifier sends a random evaluation
point after seeing the prover's commitments. We remove this interaction
with the Fiat-Shamir heuristic [9], replacing the verifier's challenge
with a hash of the transcript so far. Section 3.2 batches multiple opening
proofs into one using a random linear combination derived the same way,
reducing what would otherwise be one pairing check per committed
polynomial to a constant number of pairing checks per proof.

*[Editorial note: Section 6 of `protocol_constraints.md` names a
Häbock-2022 shuffle argument as an alternative to the public-Lagrange-mask
approach we use for `Mask_P`/`Mask_V` — worth a sentence here or in
Section 3.4 once we have the exact title/venue to cite. Flagging rather
than guessing, per the no-invented-citations rule.]*

---

## 3 The Constraint System

### 3.1 Notation

The evaluation domain is $H = \{1, \omega, \omega^2, \ldots, \omega^{n-1}\}$,
where $\omega$ is a primitive $n$-th root of unity in the BN254 scalar
field and each point $\omega^i$ corresponds to price tick $i$. The
vanishing polynomial $Z_H(X) = X^n - 1$ is zero at every point of $H$ and
nowhere else. A *vanishing equation* $V(X) = 0$ asserts that $V$ is
divisible by $Z_H(X)$, so that whatever relation $V$ encodes holds at
every tick simultaneously; the prover computes the quotient
$Q(X) = V(X)/Z_H(X)$ and commits to it alongside the witness columns. We
write $\mathsf{cm}[P]$ for the KZG10 [7] commitment to a polynomial
$P(X)$. Five scalars are disclosed as part of every clearing receipt and
carry no commitment: $V_{max}$ (the plateau height), $V_{min\Delta}$ (the
minimum imbalance inside the plateau), $c, d$ (the plateau's first and
last ticks), and $p^*$ (the clearing tick).

### 3.2 Walkthrough: the Supply Depth Transition

We take one constraint through the full protocol — vanishing polynomial,
quotient, KZG opening, and the Schwartz-Zippel argument that makes a
single evaluation sound — and give the rest as a table in Section 3.4. We
pick the supply-depth transition because it is a genuine full-domain
constraint (unlike the two single-point initialization constraints) and
because getting its recurrence direction right is not obvious, which is
exactly what a proof-of-correctness derivation is supposed to catch.

**The relation.** $AccA(X)$ is the cumulative ask depth: total volume
willing to sell at tick $i$ or lower, built as a forward running sum over
raw ask volumes $A(X)$. Advancing one tick, cumulative supply grows by
the ask volume at the *new* tick:
$$AccA[i+1] = AccA[i] + A[i+1].$$
In polynomial form, with $\omega X$ standing for "one tick forward,"
$$AccA(\omega X) - AccA(X) - A(\omega X) = 0.$$
This must hold at every tick except the last, $\omega^{n-1}$: there,
$\omega X$ would wrap around to $\omega^0$, and enforcing the recurrence
would assert a spurious relationship between the top and bottom of the
book. We gate the identity with a factor that vanishes exactly at that one
excluded point:
$$V_{\mathsf{AccA\_rec}}(X) \;=\; (X - \omega^{n-1}) \cdot
\big[AccA(\omega X) - AccA(X) - A(\omega X)\big].$$

*A note on the argument to $A$.* Reading the recurrence as
$AccA(\omega X) - AccA(X) - A(X) = 0$ — using the *current* tick's ask
volume instead of the *next* tick's — is an easy typo to make and an easy
one to miss, because both forms type-check: $A(X)$ and $A(\omega X)$ are
both single evaluations of the same committed polynomial. But only
$A(\omega X)$ matches the forward-sum definition above; using $A(X)$
silently shifts every accumulated value by one tick relative to the raw
order data it is supposed to summarize. We flag this explicitly because
it is the kind of error that a quotient computation will not catch on its
own — $V_{\mathsf{AccA\_rec}}$ built from $A(X)$ is just as divisible by
$Z_H(X)$ as one built from $A(\omega X)$, since divisibility only checks
that *some* polynomial identity holds on $H$, not that it is *the right
one*. Catching this requires checking the constraint against the
recurrence it is meant to encode by hand, which is what motivates writing
this derivation out in full rather than only stating the final equation.

**Quotient.** Because $V_{\mathsf{AccA\_rec}}(\omega^i) = 0$ for every
$i$ — at $n-1$ of the points because the bracketed recurrence holds
there, and at the remaining point because the $(X - \omega^{n-1})$ factor
itself vanishes — $V_{\mathsf{AccA\_rec}}(X)$ is divisible by $Z_H(X)$
with no remainder. The prover computes
$Q_{\mathsf{AccA\_rec}}(X) = V_{\mathsf{AccA\_rec}}(X) / Z_H(X)$ and
commits to it as $\mathsf{cm}[Q_{\mathsf{AccA\_rec}}]$.

**Opening.** The verifier's challenge $\zeta$ is derived by hashing the
transcript of commitments published so far — the Fiat-Shamir
transform [9], removing the round-trip a truly interactive verifier would
require. Because the constraint reaches one tick ahead, the prover must
open $AccA$ at *two* points, $\zeta$ and $\omega\zeta$, in addition to
opening $A$ at $\omega\zeta$ and $Q_{\mathsf{AccA\_rec}}$ at $\zeta$. Each
opening is a single KZG10 evaluation proof: for a commitment
$\mathsf{cm}[P]$ and claimed value $y = P(\zeta)$, the prover computes
$\pi = [(P(X)-y)/(X-\zeta)]_{\mathbb{G}_1}$, which exists as a polynomial
precisely because $P(\zeta) - y = 0$ makes $(X - \zeta)$ a factor of the
numerator.

**Verification and soundness.** The verifier recomputes
$Z_H(\zeta) = \zeta^n - 1$ and the gating factor $\zeta - \omega^{n-1}$
directly — both are public — and checks
$$(\zeta - \omega^{n-1}) \cdot \big[AccA(\omega\zeta) - AccA(\zeta) -
A(\omega\zeta)\big] \;\overset{?}{=}\; Q_{\mathsf{AccA\_rec}}(\zeta) \cdot
Z_H(\zeta),$$
using the opened evaluations on the left and the opened quotient on the
right, with each opening itself checked against its commitment via the
KZG10 pairing equation. If the underlying polynomial identity does *not*
hold on all of $H$ — for instance, if the prover tried to submit an
$AccA$ that skips a tick's worth of ask volume — then the difference
between the two sides of this equation is a nonzero polynomial of degree
bounded by the domain size, and a uniformly random $\zeta$ satisfies it
with probability at most that degree over the field size: negligible over
BN254's roughly 254-bit scalar field, by the Schwartz-Zippel lemma. A
cheating prover who additionally tries to open a false evaluation at
$\zeta$ still has to break KZG10's binding property to do so.

**Batching.** This is one of 33 such checks, and we do not want 33
separate pairing checks. Following PLONK's batching technique [4], the
verifier derives a second challenge $\alpha$ from the transcript and
checks a single random linear combination of all 33 residues,
$\sum_i \alpha^{i-1} r_i = 0$ where $r_i$ is constraint $i$'s left-hand
side minus its right-hand side above; the corresponding openings are
similarly aggregated into a constant number of pairing checks regardless
of how many constraints are being verified.

**A padding subtlety.** The derivation above assumes the tick domain
$H$ has exactly as many points as there are real price ticks. In practice
$H$'s size must be a power of two for the underlying NTT, so an
implementation with $N$ real ticks pads $H$ to the next power of two and
zero-fills the extra entries. Once that padding exists, "the last tick"
stops being a single unambiguous point: the *forward* accumulator's
recurrence (the one derived above) must stop at the last tick with real
data, since continuing into the zero-padded region no longer describes
anything meaningful, while a *backward* accumulator built the same way is
self-consistent through the padded region and only needs to stop at the
domain's true wraparound point. The two accumulators end up gated by two
different points once padding is introduced, even though the abstract,
unpadded derivation above uses one point for both. This is not a
correction to the relation itself — the relation is exactly what is
derived above — it is a detail that only appears once you fix a concrete
domain size, and it is worth stating explicitly rather than leaving an
implementation to discover it by trial and error.

### 3.3 The Remaining Constraints

The remaining 32 constraints follow the same recipe — a vanishing
identity, a quotient, and an opening at $\zeta$ (and $\omega\zeta$ where
the identity reaches one tick ahead) — so we give them as a table rather
than repeat the derivation. Table 1 reproduces the full constraint list,
grouped by which of the two shared gadgets from Section 2.2 each
constraint depends on, where "Product-zero" denotes the recurring
$(u)(u-1) = 0$ / $(u-a)(u-b)=0$ shape used for booleanity, mutual
exclusivity, and membership checks — a pattern common enough to name but
simple enough that we do not count it as a third formal gadget alongside
bit-decomposition and Plookup.

**Table 1 — Full constraint list** (adapted from the protocol
specification; $B(X)$/$A(X)$ = raw bid/ask volume, $AccB$/$AccA$ =
cumulative demand/supply, $Min$ = executable volume, $Mask_P$/$Mask_V$/
$Mask_C$ = plateau/valley/cliff selectors, $ChkD$ = plateau-imbalance
indicator, $SD$ = valley-masked imbalance, $Slack_L$/$Slack_R$ = cliff
slack.)

| # | Name | Pattern | Active domain |
|---|---|---|---|
| 1 | Bid range check | Plookup | All ticks |
| 2 | Ask range check | Plookup | All ticks |
| 3 | Demand init | Definitional (point pin) | $\omega^{n-1}$ |
| 4 | Supply init | Definitional (point pin) | $\omega^0$ |
| 5 | Demand transition | Definitional (recurrence) | All but last |
| 6 | Supply transition | Definitional (recurrence) | All but last — walked through above |
| 7 | $Min$ mutual exclusivity | Product-zero | All ticks |
| 8 | Ceiling ($V_{max}$ upper-bounds $Min$) | Bit-decomp | All ticks |
| 9 | $Mask_P$ booleanity | Product-zero | All ticks |
| 10 | $Mask_P$ position | Public mask / shuffle | All ticks |
| 11 | Plateau left endpoint | Public mask (Lagrange pin) | $\omega^c$ |
| 12 | Plateau right endpoint | Public mask (Lagrange pin) | $\omega^d$ |
| 13 | $InMCV$ inside plateau | Product-zero | All ticks |
| 14 | $InMCV$ outside plateau | Product-zero | All ticks |
| 15 | $SurpB$ non-negativity | Bit-decomp | All ticks |
| 16 | $SurpA$ non-negativity | Bit-decomp | All ticks |
| 17 | $\Delta$ definition | Definitional | All ticks |
| 18 | $ChkD$ booleanity | Product-zero | All ticks |
| 19 | $ChkD$ correctness | Product-zero | All ticks |
| 20 | $ChkD$ containment | Product-zero | All ticks |
| 21 | $Mask_V$ booleanity | Product-zero | All ticks |
| 22 | $Mask_V$ containment | Product-zero | All ticks |
| 23 | Delta floor | Bit-decomp | Plateau only |
| 24 | Valley pin | Public mask (Lagrange pin) | $\omega^{p^*}$ |
| 25 | $SD$ definition | Definitional | All ticks |
| 26 | $SD$ membership | Product-zero | All ticks |
| 27 | $Mask_C$ booleanity | Product-zero | All ticks |
| 28 | $Mask_C$ containment | Product-zero | All ticks |
| 29 | Left cliff | Bit-decomp | $\omega^{c-1}$ |
| 30 | Right cliff | Bit-decomp | $\omega^{d+1}$ |
| 31 | $Slack_L$ non-negativity | Bit-decomp | Cliff tick only |
| 32 | $Slack_R$ non-negativity | Bit-decomp | Cliff tick only |
| 33 | Booleanity, all bit columns | Product-zero | All ticks, per column |

Tallying Table 1's "Pattern" column against Section 2.2's two named
gadgets: bit-decomposition covers 8 of the 33 constraints (the order-size
ceiling, both surplus columns, the delta floor, and both cliff slacks),
Plookup covers 2 (the raw order-size range checks), and the remaining 23
are either definitional identities, public-Lagrange pins at a disclosed
scalar tick, or the product-zero pattern — no constraint in the system
needs a gadget outside this vocabulary.

### 3.4 Why the Masks Are Computed, Not Witnessed

Constraint 10 — $Mask_P$'s position — is the one place in Table 1 where
we depart from the two-option menu Section 2.2 lays out (a hardcoded
PLONK permutation, or a Häbock-style shuffle argument) in favor of a third
option specific to this setting: since $c$ and $d$ are already part of
the disclosed clearing receipt, $Mask_P(X)$ can be built directly as a
sum of public Lagrange basis polynomials, $\sum_{i=c}^{d} L_i(X)$,
computed by the verifier with no commitment and no position argument at
all. The same reasoning applies to $Mask_V$ (built from the disclosed
$p^*$) and $Mask_C$ (built from $c-1$ and $d+1$). This costs nothing in
disclosure — the receipt reveals $c$, $d$, and $p^*$ regardless of which
masking approach is chosen — and it removes constraints 10's grand
product and its associated boundary conditions entirely, along with the
analogous position arguments implicit in $Mask_V$ and $Mask_C$. A
shuffle or permutation argument remains available if a future version of
the protocol needs to keep $c$, $d$, or $p^*$ private; we do not need it
here because they are not.

---

## 4 User-Facing Checks

Section 3 gives the checks a batch-wide verifier runs to certify that an
entire clearing receipt is correct. A trader with one order in the book
does not need to run any of it. This section separates what an individual
trader can check about their own order — cheaply, and without touching
the 33-constraint machinery at all — from what only the full batch proof
can certify.

### 4.1 The Update Receipt

When a trader submits an order of quantity $q$ at their own limit price
(tick $\ell$), the exchange updates the relevant histogram polynomial —
say $B(X)$, for a bid — by adding $q$ at exactly that tick and nowhere
else:
$$B_{new}(X) = B_{old}(X) + q \cdot L_\ell(X),$$
where $L_\ell(X)$ is the public Lagrange basis polynomial for tick
$\ell$ (equal to 1 at $\omega^\ell$ and 0 at every other domain point).
KZG10 commitments are additively homomorphic — $\mathsf{cm}[P + Q] =
\mathsf{cm}[P] + \mathsf{cm}[Q]$ as $\mathbb{G}_1$ points — so this
relation lifts directly to commitments:
$$\mathsf{cm}[B_{new}] = \mathsf{cm}[B_{old}] + q \cdot \mathsf{cm}[L_\ell].$$
$\mathsf{cm}[L_\ell]$ depends only on the public tick $\ell$, not on any
private order data, so it can be precomputed once per tick and reused
across every order at that price. A trader who knows their own $q$ and
$\ell$ can check this equality directly against the exchange's newly
published $\mathsf{cm}[B_{new}]$: a scalar multiplication and a group
addition, no pairing and no random challenge required. Because KZG10 is
perfectly binding under the discrete-log assumption on BN254, passing
this check certifies more than "my order was recorded" — it certifies
that *no other tick changed either*, since any polynomial other than
$B_{old} + q L_\ell$ would commit to a different group element. This is
what makes the update receipt in the companion overview paper cheap: the
guarantee it gives — exactly $q$ added at exactly $\ell$, nothing else —
falls out of commitment linearity, not out of a proof the trader has to
verify constraint by constraint.

### 4.2 What a Trader Checks After Close

Once a batch closes, the exchange publishes the clearing receipt
($V_{max}$, $V_{min\Delta}$, $c$, $d$, $p^*$) together with the full
33-constraint proof from Section 3. Most of what a trader wants to know
about their own order does not require touching that proof at all.
Eligibility for a limit order is a direct comparison against the
disclosed $p^*$ — a buy executes if $p^* \leq \ell$, a sell if
$p^* \geq \ell$ — and the trader already knows their own $\ell$; this is
public arithmetic on two known scalars, not a cryptographic check. The
same is true of a market order, which is unconditionally eligible.

The one place an individual trader needs an opening rather than
arithmetic is checking their own pro-rata ration when their order sits
exactly at $p^*$ on the long side of a residual imbalance. Confirming a
specific allocation requires opening $SurpB(X)$ or $SurpA(X)$ (whichever
side is long) at the trader's own tick, against the commitment already
published as part of the clearing receipt, then applying the disclosed
rationing rule to the opened value. This costs one KZG10 opening and its
pairing check — independent of how many other ticks or orders are in the
book — not a re-run of the batch-wide verification from Section 3.

### 4.3 Division of Verification Labor

Two distinct verification jobs exist, and they have different costs. A
regulator or auditor who wants to certify that the *entire* clearing
receipt is correct runs the full batched proof from Section 3 once — a
cost that scales with book depth, which is exactly what Section 5
benchmarks. A trader who wants to certify only *their own* outcome runs
at most one opening, from Section 4.1 or 4.2, at a fixed cost independent
of book depth: a trader in a 1,000-tick book pays the same check as a
trader in a 100-tick book. Both checks are run against the same published
commitments, so a failed check is not a complaint but reproducible
cryptographic evidence — any third party holding the same public data can
run the identical check and reach the identical conclusion, without
needing anything the original trader had access to that they did not.

---

## 5 Implementation and Performance

The companion overview paper leaves one question open: producing a
cryptographic certificate for every batch "may simply be too slow or too
costly, even with modern tooling," and names benchmarking as future work.
This section is that benchmark. All figures are measured on an Apple M4
Max (14-core, 36 GB RAM, macOS Sequoia 15.3); Rust figures use
`rustc` 1.95.0 and `arkworks` 0.4.x, single-threaded; Noir figures use
`nargo` 1.0.0-beta.21 and Barretenberg (`bb`) 5.0.0-nightly.20260505.

### 5.1 Three Circuits, Not Two

We built three circuits, not a single pair to compare head-to-head:

1. **Rust/arkworks, 5 constraints.** A hand-rolled KZG10 prover for the
   accumulator-and-minimum constraints only ($V_{\mathsf{AccA\_init}}$,
   $V_{\mathsf{AccA\_rec}}$, $V_{\mathsf{AccB\_init}}$,
   $V_{\mathsf{AccB\_rec}}$, $V_{KL}$ — the min-mutual-exclusivity
   constraint), over the real $N=21$ price ticks padded to a
   32-point domain.
2. **Noir/Barretenberg, the same 5 constraints.** A Noir circuit
   asserting the identical five relations over the same 21-tick dataset,
   compiled to UltraHonk — built specifically to give (1) a same-workload
   comparison, isolating implementation and proof-system differences from
   constraint-count differences.
3. **Noir/Barretenberg, the full 33-constraint specification.** The
   complete constraint set from Section 3, covering all three clearing
   phases, run at two data scales — $N=100$ and $N=1{,}000$ price ticks —
   to show how the complete clearing rule's cost scales with book depth.
   No Rust equivalent of this circuit exists; extending the Rust
   prototype to all 33 constraints would mean hand-deriving 28 more
   quotient polynomials and opening proofs; the reason we build the full
   spec in Noir at all is that assertions there cost one loop each
   (Section 5.4).

Circuits (1) and (2) answer "what does implementation choice cost, at
fixed constraint count?" Circuit (3) answers "what does the complete
clearing rule cost, as the book gets deeper?" We keep the two questions
separate rather than reading a scaling trend across all three, since (1)/
(2) and (3) are different circuits solving different problems.

### 5.2 Same Constraints, Different Implementation

| Metric | Rust (5 constraints) | Noir (5 constraints) | Ratio |
|---|---|---|---|
| Polynomial/evaluation domain | 32 points | 4,096 points | 128x |
| Witness generation | 0.067 ms | 82.7 ms | 1,242x slower |
| Proof generation | 10.56 ms | 81.1 ms | 7.7x slower |
| Pairing verification | 15.71 ms | 12.3 ms | 1.3x *faster* |
| Prove + verify (total) | 27.52 ms | 93.4 ms | 3.4x slower |
| Proof size | ~1,200 bytes | 14,656 bytes | 12x larger |

The 128x domain gap explains most of the proving-time gap. Rust's domain
is the minimum viable size for 21 data points; UltraHonk must additionally
commit to the arithmetic, permutation, range-check, and lookup wire
columns its general-purpose gate model always carries, which pushes even
this 5-constraint circuit's gate count to 3,970 and its domain to the next
power of two, 4,096 — a fixed overhead paid once for using a
general-purpose backend, not a cost that grows with the number of
constraints written. The 12x proof-size gap has the same source: Rust
commits to exactly what the five constraints need (5 witness polynomials,
5 quotients, 3 shifted evaluations), while UltraHonk commits to its full
wire/permutation/lookup structure regardless of how small the circuit is.

Noir's pairing verification is faster despite the larger domain — a
genuine result of a more optimized BN254 pairing implementation in
Barretenberg, not an artifact of circuit size. One caveat on the
comparison: the Rust figure of 15.71 ms includes both 26 individual
pairing checks and the 4 batched checks run in the same benchmark, by the
Rust implementation's own design, "for diagnostic completeness" rather
than as a minimal-verifier measurement; the two numbers are not run under
identically minimal verification paths, and we report this rather than
adjust for it.

### 5.3 Scaling the Complete Constraint Set

| | $N=100$ | $N=1{,}000$ |
|---|---|---|
| ACIR opcodes | 6,326 | 63,026 |
| UltraHonk gates | 13,736 | 111,611 |
| Proof size | 14,656 bytes | 14,656 bytes |
| Prove time | 0.13 s | 0.45 s |
| Verify time | 0.02 s | 0.01 s |
| Verify result | PASS | PASS |

Gate count grows 8.1x from $N=100$ to $N=1{,}000$, and prove time grows
3.5x — sub-linear, consistent with UltraHonk's proving cost scaling with
$O(n \log n)$ FFTs over a domain that only doubles when gate count crosses
a power-of-two boundary. Proof size is identical at both scales, and
identical to Section 5.2's 5-constraint Noir circuit as well: three
circuits spanning 3,970 to 111,611 gates — a 28x range — all produce
exactly 14,656 bytes. This is UltraHonk's constant-verifier-cost property
holding in practice, not just in principle: whatever we assert inside the
circuit, the proof a third party has to check stays the same size.

We also ran a negative control: incrementing the disclosed $V_{max}$ by 1
in the $N=100$ circuit's public inputs causes the proof to fail exactly at
the plateau-endpoint constraint (constraint 11/12 in Table 1,
$(Min(X)-V_{max}) \cdot L_c(X) = 0$), confirming the constraints reject a
falsified clearing receipt rather than passing vacuously.

### 5.4 Rust as a Verification Tool, Not a Deployment Candidate

The Rust prototype's role is establishing that the constraint system is
correct, not competing on production performance — a distinction the two
implementations' own design goals make explicit: every intermediate
polynomial coefficient and quotient remainder in the Rust pipeline is
directly inspectable, at the cost of hand-writing a new quotient
computation, commitment, and opening proof for every future constraint;
Noir trades that inspectability for writing one assertion per constraint
and inheriting Barretenberg's production concerns — a real multi-party
trusted-setup ceremony, automatic proof blinding, and a prover/verifier
split into separate binaries — none of which the Rust prototype
implements. Table 2 gives the Rust pipeline's own per-layer cost, useful
for understanding where a from-scratch KZG10 prover spends its time even
though it is not the implementation we would deploy.

**Table 2 — Rust pipeline, per layer**

| Layer | Mean | Share of total |
|---|---|---|
| 1: array computation | 681 ns | — |
| 2: polynomial interpolation (5 IFFTs) | 7.60 µs | — |
| 3a: KZG trusted setup | 1.699 ms | — |
| 3b: commit 5 witnesses (5 MSMs) | 2.222 ms | — |
| 3c: quotient polynomials | 32.13 µs | — |
| 3d: commit 5 quotients (5 MSMs) | 1.405 ms | — |
| 3e: Fiat-Shamir transcript | 8.61 µs | — |
| 3f: 13 opening proofs (13 MSMs) | 5.185 ms | — |
| 3f: pairing verification | 15.71 ms | — |
| 4: direct algebraic check | 66.6 µs | — |
| **Total (fresh setup each run)** | **27.52 ms** | 100% |

Grouped by phase: proof generation (layers 1–3f openings) is ~11.5 ms
(~42%), pairing verification is ~15.7 ms (~57%), and the redundant
direct algebraic check is ~0.07 ms (under 1%) — pairing verification
dominates because a BN254 Miller loop and final exponentiation cost
roughly 10x a G1 MSM of comparable size. One further caveat on this total:
it includes a fresh KZG trusted setup (1.699 ms) on every run; a
deployment would run setup once and amortize it across every subsequent
batch, so 27.52 ms overstates the per-batch marginal cost of every batch
after the first.

### 5.5 What This Means for a Deployed Auction

At $N=1{,}000$ — an order of magnitude more price ticks than most single
listings need — the full 33-constraint proof takes 0.45 s to generate and
0.01 s to verify. Against the companion paper's own proposed cadence of
"once a second," this fits inside the interval with room to spare; against
a sub-second cadence, 0.45 s of proving does not fit inside the same
window on its own, but proving and order collection are logically
separate processes — nothing prevents a batch's proof from being
generated during the *next* batch's collection window rather than before
it opens. We have not built or measured such a pipelined deployment; we
flag it as the natural next design step rather than claim it as a result.
The concrete answer to the companion paper's open question, at the scales
measured here, is that per-batch proving cost is no longer the dominant
practical constraint on how often an FBA can run — the batch interval a
market operator chooses is.

---

## 6 Security Proof

Two adversaries matter here, and they want different things. A malicious
exchange wants to publish a clearing receipt that is wrong — a $p^*$ that
is not actually the imbalance-minimizing plateau tick, or a $V_{max}$ that
does not match the committed order book — and still pass verification.
A curious verifier, trader, or third party wants to learn something about
an individual bid or ask beyond what the receipt already discloses. The
first is a soundness question; the second is a privacy question. We argue
both at a high level here and defer the full construction — extractor and
simulator — to Appendix A.

### 6.1 Completeness

An honest exchange that runs the clearing algorithm on its real order book
and computes every witness column exactly as Section 3 defines it
satisfies every constraint pointwise on $H$ by construction, so every
vanishing polynomial $V_i(X)$ is divisible by $Z_H(X)$ with no remainder,
every quotient $Q_i(X)$ is a well-defined low-degree polynomial, and every
opening equation in Section 3.2 holds exactly at $\zeta$ and $\omega\zeta$.
Verification succeeds with probability 1. This direction of the argument
is immediate and we do not belabor it further.

### 6.2 Knowledge Soundness

A cheating exchange that submits a false clearing receipt must submit
witness polynomials for which at least one constraint's vanishing identity
does not hold on all of $H$ — otherwise the receipt would, by definition,
be correct. Section 3.2 already argues the single-constraint case: a
nonzero residual polynomial of bounded degree vanishes at a uniformly
random $\zeta$ with probability at most (degree)/(field size), negligible
over BN254's scalar field by the Schwartz-Zippel lemma. Batching 33
constraints into one random linear combination (Section 3.2, "Batching")
costs a union bound over which constraint the adversary tries to hide a
violation in — still negligible, since 33 is a constant and the field is
254 bits wide. A prover who instead tries to open a false evaluation at
$\zeta$ without a genuine underlying polynomial has to break KZG10's
binding property, which its own security proof reduces to the $t$-SDH
assumption [7]. Turning "the verifier's equation checks out" into "the
prover knew a real witness" — knowledge soundness rather than mere
soundness — requires constructing an extractor from any prover that
convinces the verifier with non-negligible probability; PLONK's own
batched-opening technique has been given exactly this treatment in two
independent follow-up analyses, one establishing simulation extractability
in the random oracle model [12] and one identifying specific ways a naive
Fiat-Shamir batching argument can fail to achieve it [13]. Both papers
exist because this step is subtle enough to get wrong silently, which is
why we do not sketch an extractor inline and instead give the full
reduction in Appendix A, built on the same batching structure Section 3.2
uses.

One soundness question is specific to this protocol rather than inherited
from PLONK: Section 3.4's computed masks. Because $Mask_P$, $Mask_V$, and
$Mask_C$ are built as public Lagrange sums over $c$, $d$, and $p^*$ rather
than committed and proved-correct via a separate position argument, there
is no additional soundness burden to discharge for them at all — the
verifier recomputes each mask directly from the disclosed scalars, so a
malicious prover cannot submit an inconsistent mask by construction, not
by virtue of a proof that its shuffle argument was executed correctly.
This removes an entire class of possible attack (a mask that claims to
select $[c,d]$ but is actually witnessed to select a different set of
ticks) rather than adding one.

### 6.3 Privacy: What the Verifier Learns

The verifier's entire view of the private order book, across every check
in Sections 3 and 4, consists of: the disclosed clearing receipt ($V_{max}$,
$V_{min\Delta}$, $c$, $d$, $p^*$); commitments to $B(X)$, $A(X)$, and every
derived witness column; and evaluations of those commitments at the
Fiat-Shamir challenge point $\zeta$ (and $\omega\zeta$), plus the one
per-trader opening from Section 4.2 at a trader's own tick $\ell$. No check
anywhere in this paper opens a committed polynomial at an attacker-chosen
tick other than a trader's own. An honest-verifier zero-knowledge argument
for this construction would build a simulator that produces
indistinguishable commitments and openings given only the disclosed
receipt — no witness order book — by programming the Fiat-Shamir hash and
using KZG10's hiding commitments (a random blinding factor per polynomial)
to make every simulated opening at $\zeta$ land on an arbitrary value.

We flag a real gap here rather than claim this is done: our Rust
prototype (Section 5) already commits with hiding — `batch_open` requires
it structurally — but its own inline documentation is explicit that "the
FBA proof is not zero-knowledge" as currently built, because hiding is
present only as a side effect of satisfying that gadget's precondition,
not because a simulator has been constructed and checked against it. No
simulator exists yet for this protocol at either implementation scope in
Section 5. What we have argued in this section is the shape the argument
should take, not that it has been completed; a full simulator
construction, and the accompanying proof that its output is
indistinguishable from a real transcript, is future work and is not
claimed as a result of this paper.

---

## 7 Conclusion

The companion overview paper states the FBA clearing rule and names the
gadget vocabulary a zero-knowledge proof of it would need, then stops at
the boundary of the arithmetization itself. We have crossed that boundary:
Section 3 gives all 33 constraints of the three-phase clearing rule as
vanishing polynomial identities over a single evaluation domain, built
from two gadgets rather than one gadget per column, with a masking
construction (Section 3.4) that discloses nothing beyond what the clearing
receipt already reveals. Section 4 shows that a trader checking their own
order never has to touch this machinery at all — an update receipt and, at
most, one opening. Section 5 answers the overview paper's open question
with a number rather than a hedge: the full constraint set proves in 0.45
seconds at 1,000 price ticks, comfortably inside a once-a-second clearing
cadence.

Section 6 draws the line at what this paper actually establishes. Knowledge
soundness follows the same argument PLONK's own batched-opening technique
already has, and we defer the extractor construction to Appendix A rather
than assert it inline. Privacy does not yet have that treatment: no
simulator has been built for this construction at either implementation
scope, and the Rust prototype's own documentation is explicit that its
hiding commitments satisfy a gadget precondition, not a checked
zero-knowledge guarantee. We would rather state that gap than paper over
it — closing it is the most direct piece of future work this paper leaves,
ahead of the pipelined-proving design sketched in Section 5.5 for
sub-second batch cadences.

---

## Appendix A: Knowledge-Soundness Reduction

### A.1 The Relation and What We Need an Extractor to Recover

The statement is a tuple of commitments — $\mathsf{cm}[B]$, $\mathsf{cm}[A]$,
and one commitment per derived column in Table 1 ($AccB$, $AccA$, $Min$,
$SurpB$, $SurpA$, $\Delta$, $Slack_L$, $Slack_R$, every quotient
$Q_i$) — together with the disclosed scalars $V_{max}$, $V_{min\Delta}$,
$c$, $d$, $p^*$. The witness is the set of polynomials underlying those
commitments. The relation $\mathcal{R}_{\mathsf{FBA}}$ holds exactly when
every one of the 33 vanishing identities in Table 1 holds pointwise on
$H$, so that the disclosed scalars are the genuine output of running the
three-phase clearing rule on the polynomials the witness commits to.
Knowledge soundness asks for an extractor $\mathcal{E}$ that, given
black-box (rewinding) access to any prover $P^*$ that makes the verifier
accept with probability $\varepsilon$ non-negligible in the security
parameter, outputs a witness satisfying $\mathcal{R}_{\mathsf{FBA}}$ except
with probability negligible in $\varepsilon$. We build $\mathcal{E}$ in two
stages: first recover each committed polynomial individually (A.2), then
show that the verifier's single batched check forces every one of the 33
underlying residuals to vanish, not merely their random linear
combination (A.3).

### A.2 Extracting Individual Witness Polynomials

Every witness and quotient column is committed before the batching
challenge $\alpha$ is derived — batching in Section 3.2 happens only at
the opening stage, so at commitment time each $\mathsf{cm}[W]$ already
fixes a single polynomial, if $P^*$ is behaving honestly at all. $\mathcal{E}$
rewinds $P^*$ to fresh values of the evaluation challenge $\zeta$ —
sampling more than $\deg(W)+1$ of them, since every witness column in this
protocol has degree at most $n-1$ over the size-$n$ domain — and, for each
$\zeta$ at which $P^*$ still produces an accepting opening of
$\mathsf{cm}[W]$, records the pair $(\zeta, W(\zeta))$. Interpolating
$\deg(W)+1$ such pairs yields a unique candidate polynomial
$\widehat{W}(X)$. If $P^*$ ever opens $\mathsf{cm}[W]$ to a value
inconsistent with $\widehat{W}$ at some further challenge point, it has
produced two valid openings of the same commitment to different values at
the same point — exactly the event KZG10's own binding proof rules out
under the same $t$-SDH-style assumption Section 6.2 already invokes for
the single-opening case [7]. So except with the same negligible
probability, $\widehat{W}$ is the unique polynomial $\mathsf{cm}[W]$
commits to, for every column $W$ the protocol commits to independently.

### A.3 From the Batched Residual to Every Individual Residual

Fix $\zeta$ and write $r_i(\zeta)$ for constraint $i$'s residual — the
left-hand side of its vanishing identity in Section 3.2's template, minus
$Q_i(\zeta) \cdot Z_H(\zeta)$ (or the appropriate point-gating factor for a
single-point constraint) — evaluated using the $\widehat{W}$'s recovered
in A.2. The verifier's actual check is not 33 separate equations
$r_i(\zeta) = 0$; it is the single combined equation
$\sum_{i=1}^{33} \alpha^{i-1} r_i(\zeta) = 0$ from Section 3.2's
"Batching" paragraph, where $\alpha$ is itself a Fiat-Shamir challenge
derived from the transcript of commitments — fixed before $P^*$ chose any
$\widehat{W}$, since every commitment in A.2 is published before $\alpha$
is drawn. Treat the left-hand side as a polynomial in the indeterminate
$\alpha$: $R(\alpha) = \sum_{i=1}^{33} \alpha^{i-1} r_i(\zeta)$, of degree
at most 32. If some $r_i(\zeta) \neq 0$, then $R(\alpha)$ is a nonzero
polynomial of degree at most 32, and it vanishes at a uniformly random
$\alpha$ with probability at most $32$ over the field size — negligible
over BN254's scalar field, by the same Schwartz-Zippel lemma Section 3.2
and Section 6.2 already use for $\zeta$, applied here to the *batching*
challenge rather than the *evaluation* challenge. This is a second,
independent application of the same lemma, not a restatement of the
first: Section 6.2's union bound over "which constraint hides a violation"
is the informal version of exactly this argument. Except with this second
negligible probability, then, $R(\alpha) = 0$ forces every $r_i(\zeta) = 0$
individually, and $\mathcal{E}$ outputs the $\widehat{W}$'s from A.2 as the
extracted witness — which, having every residual vanish at a uniformly
random $\zeta$, satisfies every one of Table 1's 33 identities on all of
$H$ except with the negligible probability Section 3.2 already bounds per
constraint.

### A.4 What This Reduction Does Not Cover

The argument above extracts a witness from a single accepting proof. It
does not address an exchange that publishes a fresh proof once per batch,
indefinitely, and an adversary who has seen many prior valid proofs before
attempting to forge one for a batch it cannot actually witness — a
malleability concern specific to a system that is not proving one
isolated statement but issuing a long-running sequence of them, all
against structurally related public parameters (the same SRS, the same
public Lagrange basis commitments $\mathsf{cm}[L_\ell]$ reused across every
batch's update receipts, Section 4.1). Ruling this out is *simulation*
extractability, not plain knowledge soundness: an extractor that must
still succeed even against a prover with oracle access to simulated
proofs of other, possibly related, statements. This is exactly the
property [12] and [13] examine for PLONK's own batched-opening argument in
the random oracle model, and exactly why Section 6.2 cites them rather
than treats this reduction as the end of the story — one gives conditions
under which PLONK-style batching remains simulation-extractable in the
ROM, the other identifies specific batching constructions where a naive
argument does not achieve it. We have not re-derived either paper's
argument against this protocol's specific reuse of $\mathsf{cm}[L_\ell]$
across batches; doing so, rather than the single-proof reduction given
above, is the correct next step before treating the protocol's long-running
deployment as fully analyzed.

## Appendix B: Non-Expansivity of the Remaining 32 Constraints

### B.1 The General Argument

Every constraint in Table 1 other than Constraint 6 (walked through in
Section 3.2) reduces to the same two ingredients: a *relation* — an
algebraic expression that equals zero exactly when the constraint's
underlying fact holds — multiplied by a *gate*, a public polynomial
restricting where the relation is actually enforced. Four gate types cover
every remaining constraint:

- **No gate (full domain).** The relation itself must vanish at every
  tick. Degree at most $n-1$.
- **Point-quotient gate**, $Z_H(X)/(X-\omega^k)$. Zero at every domain
  point except $\omega^k$; degree $n-1$. Used only for the two accumulator
  initializations (Constraints 3, 4).
- **Single-point Lagrange gate**, $L_k(X)$. Zero at every domain point
  except $\omega^k$, where it equals 1; degree $n-1$. Used for every
  disclosed-scalar pin — both plateau endpoints, the valley pin, both
  cliffs — and the two Plookup boundary conditions.
- **Public mask gate**, one of $Mask_P(X)$, $Mask_C(X)$,
  $(1-Mask_P(X))$, or $(1-ChkD(X))$. Built as a sum of public Lagrange
  basis polynomials from the disclosed $c$, $d$, or $p^*$, or as 1 minus
  one of these (Section 3.4); zero outside its support, 1 inside it;
  degree at most $n-1$.

For any of these four gate types, $V_i(X) = \mathrm{gate}(X) \cdot
\mathrm{relation}(X)$ vanishes at every point of $H$ exactly when the
relation holds wherever the gate is nonzero — off the gate's support, the
product is already zero regardless of the relation's value, so the
identity holds vacuously there. This is why every constraint Table 1
marks "Plateau only" or "Cliff tick only" is checked with the same
$Z_H(X)$-divisibility recipe as every full-domain constraint: "Active
domain" in Table 1 describes where a constraint has *bite*, not a
different verification recipe. In every case above, $\deg(V_i) \leq
(n-1) + (n-1) = 2n-2$, so $Q_i(X) = V_i(X)/Z_H(X)$ has degree at most
$n-2$ — inside the same bound Appendix A.2's rewinding extraction already
assumes for every witness column. No constraint in Table 1 forces the
extractor to query more evaluation points, or absorb a larger union bound,
than Appendix A already accounts for; this is what we mean by
*non-expansive*. The two Plookup constraints (1, 2) are the one place this
single-relation template does not literally apply: each expands into the
three equations of Section 2.2's gadget — two single-point Lagrange
boundary pins ($L_1$, $L_n$) plus one $(X-\omega^{n-1})$-gated running-
product transition, structurally identical to Constraint 6's own gate — at
the cost of two additional witness columns per Plookup instance (a grand
product $Z(X)$ and a sorted interleaving $s(X)$), each extracted by
Appendix A.2 exactly like any other committed column. No new gate type or
extraction technique is needed for them either.

### B.2 Constraint-by-Constraint

| # | Name | Relation (gate applied per B.1) | Gate | Note |
|---|---|---|---|---|
| 1 | Bid range check | Plookup running-product + two boundary pins on $B(X)$ vs. $t_{in}$ | Lagrange ($L_1$, $L_n$) + $(X-\omega^{n-1})$ | Three sub-equations, covered by B.1's closing paragraph; adds $Z_B(X)$, $s_B(X)$ |
| 2 | Ask range check | Same shape as 1, on $A(X)$ vs. $t_{in}$ | Lagrange + $(X-\omega^{n-1})$ | Adds $Z_A(X)$, $s_A(X)$; no new machinery beyond 1 |
| 3 | Demand init | $AccB(X) - B(X) = 0$ | Point-quotient at $\omega^{n-1}$ | $\deg(V_3) \le 2n-2$; mirrors 4 with roles swapped |
| 4 | Supply init | $AccA(X) - A(X) = 0$ | Point-quotient at $\omega^0$ | Symmetric to 3 |
| 5 | Demand transition | $AccB(X) - AccB(\omega X) - B(X) = 0$ | $(X-\omega^{n-1})$ | Mirror image of Constraint 6 (Section 3.2), demand instead of supply, decreasing instead of increasing — same degree bound, same Schwartz-Zippel argument, not re-derived |
| 7 | $Min$ mutual exclusivity | $(AccA(X)-Min(X))(AccB(X)-Min(X)) = 0$ | None (full domain) | Degree $2(n-1)$ relation itself, no separate gate needed; product-zero pattern |
| 8 | Ceiling | $(V_{max}-Min(X)) - \sum 2^j B_j^{ceil}(X) = 0$ | None | Bit-decomposition columns extracted individually per A.2; booleanity of each $B_j^{ceil}$ is a separate instance of Constraint 33 |
| 9 | $Mask_P$ booleanity | $Mask_P(X)(Mask_P(X)-1) = 0$ | None | Vacuous if $Mask_P$ is the public Lagrange-sum construction of Section 3.4 rather than a committed witness — see 10 |
| 10 | $Mask_P$ position | Public Lagrange sum (Section 3.4) or shuffle/permutation argument | — | Under the public-mask construction this paper adopts, 9 and 10 both hold with no witness or commitment at all, since $Mask_P$ is computed, not proved; the shuffle alternative (Habock 2022) is a separate three-equation argument structurally identical to Plookup's, not analyzed further here |
| 11 | Plateau left endpoint | $(Min(X)-V_{max}) \cdot L_c(X) = 0$ | Single-point Lagrange at $\omega^c$ | Degree $2n-2$, quotient degree $n-2$ |
| 12 | Plateau right endpoint | $(Min(X)-V_{max}) \cdot L_d(X) = 0$ | Single-point Lagrange at $\omega^d$ | Symmetric to 11 |
| 13 | $InMCV$ inside plateau | $(InMCV(X)-V_{max}) \cdot Mask_P(X) = 0$ | Public mask $Mask_P$ | Standard mask-gate instance |
| 14 | $InMCV$ outside plateau | $InMCV(X) \cdot (1-Mask_P(X)) = 0$ | Public mask $1-Mask_P$ | Standard mask-gate instance |
| 15 | $SurpB$ non-negativity | $SurpB(X) - \sum 2^j B_j^{sB}(X) = 0$ | None | Bit-decomposition; per-bit booleanity is Constraint 33 |
| 16 | $SurpA$ non-negativity | $SurpA(X) - \sum 2^j B_j^{sA}(X) = 0$ | None | Symmetric to 15 |
| 17 | $\Delta$ definition | $\Delta(X) - (SurpA(X)+SurpB(X)) = 0$ | None | Degree $\le n-1$, no bit-decomposition — a direct definitional identity |
| 18 | $ChkD$ booleanity | $ChkD(X)(ChkD(X)-1) = 0$ | None | Product-zero pattern |
| 19 | $ChkD$ correctness | $(\Delta(X)-V_{min\Delta}) \cdot ChkD(X) = 0$ | None (relation gated by witness column, not a public mask) | $ChkD$ is itself committed witness here, unlike $Mask_P$ — its position is not independently pinned by this constraint alone; see 20 |
| 20 | $ChkD$ containment | $ChkD(X) \cdot (1-Mask_P(X)) = 0$ | Public mask $1-Mask_P$ | Combined with 19, forces $ChkD$ to fire only inside the plateau and only where $\Delta = V_{min\Delta}$ |
| 21 | $Mask_V$ booleanity | $Mask_V(X)(Mask_V(X)-1) = 0$ | None | Product-zero pattern |
| 22 | $Mask_V$ containment | $Mask_V(X) \cdot (1-ChkD(X)) = 0$ | Witness gate $1-ChkD$ | Not a public-mask gate — $ChkD$ is itself a witness column, so this constraint's degree bookkeeping uses the "no gate" bound (relation degree $\le 2(n-1)$), not the public-mask bound |
| 23 | Delta floor | $Mask_P(X)\big[(\Delta(X)-V_{min\Delta})-\sum 2^j B_j^{flr}(X)\big] = 0$ | Public mask $Mask_P$ | The constraint Table 1 marks "Plateau only" — B.1's mask-gate argument is exactly why this is nonetheless a full-$H$ identity |
| 24 | Valley pin | $(\Delta(X)-V_{min\Delta}) \cdot L_{p^*}(X) = 0$ | Single-point Lagrange at $\omega^{p^*}$ | Same shape as 11/12, disclosed scalar is $p^*$ instead of $c$/$d$ |
| 25 | $SD$ definition | $SD(X) - Mask_V(X)\cdot\Delta(X) = 0$ | None | Degree $\le 2(n-1)$; definitional, not bit-decomposed |
| 26 | $SD$ membership | $SD(X)(SD(X)-V_{min\Delta}) = 0$ | None | Product-zero pattern; implied by 24 directly (Section 3, "Section 12" note) but retained for one-to-one traceability |
| 27 | $Mask_C$ booleanity | $Mask_C(X)(Mask_C(X)-1) = 0$ | None | Product-zero pattern |
| 28 | $Mask_C$ containment | $Mask_C(X) \cdot Mask_P(X) = 0$ | Public mask $Mask_P$ (both factors public if $Mask_C$ is also computed, Section 3.4) | Forces cliff ticks outside the plateau |
| 29 | Left cliff | $(V_{max}-Min(X)-1-Slack_L(X)) \cdot L_{c-1}(X) = 0$ | Single-point Lagrange at $\omega^{c-1}$ | Only meaningful if $c > 0$ (Section 5 of the companion protocol-design note); the $-1$ term changes the relation's constant, not its degree |
| 30 | Right cliff | $(V_{max}-Min(X)-1-Slack_R(X)) \cdot L_{d+1}(X) = 0$ | Single-point Lagrange at $\omega^{d+1}$ | Only meaningful if $d < n-1$; symmetric to 29 |
| 31 | $Slack_L$ non-negativity | $Slack_L(X) - \sum 2^j B_j^{slkL}(X) = 0$ | None | Bit-decomposition; width $k=\lceil\log_2 V_{max}\rceil$, same as Constraint 8 |
| 32 | $Slack_R$ non-negativity | $Slack_R(X) - \sum 2^j B_j^{slkR}(X) = 0$ | None | Symmetric to 31 |
| 33 | Booleanity, all bit columns | $B_j(X)(B_j(X)-1) = 0$ | None | Not one constraint but one instance per bit-witness column across 8, 15, 16, 23, 29, 31, 30, 32 (and the corresponding columns for 9/18/21/27 if those masks are witnessed rather than computed) — every instance is the same product-zero shape, degree $2$ before gating |

Every row above stays within the degree-$2n-2$ bound B.1 establishes, so
Appendix A's extractor construction applies uniformly across all 33
constraints with no per-constraint exception. The one place this appendix
does not give a fully independent argument is Constraints 3 and 5, which
we note are exact mirror images of Constraints 4 and 6 rather than
re-derive from scratch — the accumulator direction (demand decreasing vs.
supply increasing) changes which endpoint is pinned and which direction
the recurrence runs, not the degree bookkeeping or the soundness argument
itself.

---

## Appendix C: Implementation and ABI Details

Section 5 reports gate counts, proof sizes, and prover wall-clock time for
both implementations. This appendix gives the interfaces underneath those
numbers — the function signatures and circuit ABIs a reader would need to
reproduce or extend either implementation, without repeating Section 5's
benchmark discussion.

### C.1 Rust/arkworks Prover

The prover is organized as a library crate over four dependency groups:
`ark-ff`, `ark-poly`, `ark-bn254`, `ark-ec`, and `ark-std` for field,
polynomial, and curve arithmetic; `ark-poly-commit` for the KZG10 API
(`KZG10`, `Powers`, `VerifierKey`, `Commitment`, `Randomness`); the
MadibaGroup gadgets crate for `Transcript`, `batch_open`, `batch_check`,
and the range-proof gadget; and `sha2` for the Fiat-Shamir hash inside
`Transcript`.

The order book is represented as a single struct:

```rust
pub struct OrderBook {
    pub n:           usize,   // real price ticks (21)
    pub domain_size: usize,   // NTT domain, next power of 2 (32)
    pub prices:      Vec<u64>,
    pub bids:        Vec<F>,  // B(X)
    pub asks:        Vec<F>,  // A(X)
    pub bid_depth:   Vec<F>,  // AccB(X), backward cumulative sum
    pub ask_depth:   Vec<F>,  // AccA(X), forward cumulative sum
    pub min_arr:     Vec<F>,  // Min(X)
    pub mcv:         u64,     // V_max = max(min_arr)
}
```

loaded via `OrderBook::from_csv`, which cross-checks every derived column
(`bid_depth`, `ask_depth`, `min_arr`) against the recurrence it is supposed
to satisfy at load time, rather than trusting the input file. The proof
pipeline is a fixed call sequence — `build_domain` fixes the evaluation
domain; `build_all_polys` interpolates the five committed columns from
`OrderBook`; `commit_all` and `compute_quotients`/`commit_quotients`
produce the witness and quotient commitments Section 3.2 describes;
`fiat_shamir_prove` derives $\zeta$ and $\alpha$ from the transcript;
`compute_all_opening_proofs` and `verify_all_openings` run the batched
KZG10 opening from Section 3.2's "Batching" paragraph; `prove_mcv_range`/
`verify_mcv_range` run the ceiling constraint's range gadget separately
from the batched openings. `full_pipeline(book, rng)` runs every step and
returns:

```rust
pub struct PipelineResult {
    pub constraint_result: ConstraintResult,  // pointwise checks on H
    pub quotient_check:    QuotientCheck,      // quotients divide exactly
    pub fs_proof:          FiatShamirProof,    // batch_ok: combined pairing check
    pub opening_ok:        bool,
    pub range_ok:          bool,
    pub mcv:               u64,
    pub n:                 usize,
    pub domain_size:       usize,
}
```

with `PipelineResult::all_pass()` returning true only if every one of the
five fields above independently passes — mirroring, at the level of a
single function call, the same "every residual must vanish, not just
their combination" argument Appendix A.3 makes formally. This is a
diagnostic API, not a minimal one: a production prover would not expose
`constraint_result` and `quotient_check` as separate return values once
`fs_proof.batch_ok` already subsumes them (Section 5.4's caveat about
Table 2's pairing-verification line applies here for the same reason).

### C.2 Noir/Barretenberg Circuit

The full 33-constraint circuit is a single Noir source file,
`protocol_main_template.nr`, compiled into two packages by substituting a
compile-time array-length global `N` (100 or 1,000; Noir requires array
sizes to be known at compile time, so the two data scales are two
separate compiled artifacts rather than one circuit run twice). Its ABI:

```rust
fn main(
    // Private witnesses
    b:       [u64; N],  // B(X)     Bids++
    a:       [u64; N],  // A(X)     Asks++
    acc_b:   [u64; N],  // AccB(X)  Bid Depth
    acc_a:   [u64; N],  // AccA(X)  Ask Depth
    min_x:   [u64; N],  // Min(X)   Min(Bid,Ask)
    surp_b:  [u64; N],  // SurpB(X) Bid Surplus++
    surp_a:  [u64; N],  // SurpA(X) Ask Surplus++
    delta:   [u64; N],  // Delta(X) Abs(Delta)
    slack_l: u64,        // opened at c-1, only meaningful if c > 0
    slack_r: u64,        // opened at d+1, only meaningful if d < N-1

    // Public scalars (the clearing receipt, Section 3.1)
    v_max:       pub u64,
    v_min_delta: pub u64,
    c:           pub u32,
    d:           pub u32,
    p_star:      pub u32,
) { ... }
```

This ABI differs from the Rust prover's in one respect worth stating
explicitly: every derived column in Table 1 — $AccB$, $AccA$, $Min$,
$SurpB$, $SurpA$, $\Delta$ — is supplied as a private witness input
directly, rather than computed inside the circuit from $B$ and $A$ alone.
The circuit does not trust these inputs on that account: every constraint
in Appendix B's table asserts the relation each column is supposed to
satisfy, so an inconsistent value fails the corresponding `assert` during
witness generation, before a proof is ever attempted. Values are
recomputed from raw `b`/`a` and cross-checked against a second,
independent source (the underlying CSV's own precomputed columns) before
being written into the two packages' `Prover.toml` files — the same
load-time discipline Section C.1 describes for the Rust prover's
`from_csv`, applied here at witness-generation time instead of inside the
library.

Compiling and proving use the standard Noir/Barretenberg toolchain:
`nargo execute` generates the witness from `Prover.toml` against the
compiled circuit; `bb prove -b circuit.json -w witness.gz -k vk/vk -o
proof_dir/` produces the UltraHonk proof Section 5 benchmarks; `bb verify`
checks it; `bb gates -b circuit.json` reports the gate count Table 1 of
Section 5.3 reproduces. Both packages compile as a plain Noir binary
package with no external dependencies — the circuit is self-contained
Noir source, not a wrapper around another proving library.
