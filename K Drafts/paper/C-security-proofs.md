# Appendix C. Integrity of a recorded batch

## Statement and assumptions

Fix one instrument, price grid, cutoff and set of public parameters. Let $h$ be a final log digest that traders and the public verifier obtain from the same publication channel. An adversarial exchange chooses the entries, the outcome and both proofs. Acceptance means that the verifier checks the empty opening, replays the log, verifies the update proof on the resulting differences, verifies the outcome proof against the resulting column commitments, and obtains $h$. A trader additionally checks the fields, commitment equation and position of their private receipt as specified in [Section 4](04-committed-book.md).

Write $\mu$ for the number of log entries and $q_F$ for the scalar-field order. We assume collision resistance of the log hash, a securely generated reference string, soundness of the empty-opening check, and knowledge soundness of the particular polynomial arguments used here. The latter assumptions include the required degree bounds, the range lookup, the ordering of the Fiat–Shamir challenges and consistent extraction of the values underlying commitments shared across the opening, log and proofs. Evaluation binding alone does not give an extractor for an arbitrary published group element. We use $2^{k+1}<q_F$ for integer comparisons, the aggregate-width bound of [Section 3](03-protocol.md), and $\mu(2^{k_{\rm in}}-1)<q_F$ for the history.

**Proposition C.1 (integrity for one published batch).** Under these assumptions, except with the soundness error, every accepted outcome is the specified clearing result of the book accumulated from that log. For column $a$ and real tick $i$, let $T_{a,i}$ be any set of **distinct logged transitions** with checked trader receipts at that coordinate, and let $q_j$ be the checked quantity at transition $j$. If $B^h_{a,i}$ is the final integer volume at that coordinate, then

$$
 B^h_{a,i}\ \geq\ \sum_{j\in T_{a,i}}q_j .
$$

On a cross the eight-field receipt is uniquely determined by the four final columns. On a no-cross the ordinary columns have zero executable volume at every tick. The latter branch places no final per-cell width bound on IO volume.

## From log entries to an integer book

Let $u_j\in\mathbb F^n$ denote the extracted value contribution of the $j$th commitment difference on the price domain $H$, in its recorded column. The accepted opening fixes zero initial values on $H$. Replaying the hash chain from the opening fixes the ordered entries for digest $h$, up to a collision. Taking differences between successive states of each column makes the final column's value the sum of its contributions. A valid trader receipt for a transition in that sequence gives $u_j=q_j e_i$, where $e_i$ is the unit vector for its tick. This accounts for the checked transition. The remaining entries still require a public check because their traders' receipts need not be available.

The update proof commits to quantities and labels before deriving the weighting challenge $\eta$. Its range check U1 bounds each quantity. The verifier constructs the commitment to $W$ from a proved-empty mask and the public differences. Consistent extraction gives $W_i=\sum_j\eta^j u_{j,i}$ on $H$. U2–U4 then test the histogram identity in [Appendix B](B-constraint-relations.md). To see what this checks, fix $\eta$ and consider both sides as rational functions of the later challenge $\beta$. Away from poles and accidental equality at the sampled point, their residues agree at every label. At a price label $z=\omega^i$ this requires

$$
 \sum_{j=0}^{\mu-1}\eta^j
 \left(u_{j,i}-q_j\,\mathbf{1}[t_j=\omega^i]\right)=0 .
$$

At a label outside $H$, the corresponding sum of weighted quantities must be zero. The step representations, quantities and labels were fixed before $\eta$. If any coefficient in one of these equations is nonzero, its polynomial has degree at most $\mu-1$ and can vanish only at exceptional choices of $\eta$. Thus a nonzero step has a label in $H$ and adds its bounded quantity at exactly that coordinate. A zero step contributes no volume. This argument uses the commitment order twice, first to interpret the histogram test for fixed $\eta$, then to separate steps that share a label. It does not turn a single field equality into a deterministic polynomial identity.

Summing in a column now gives only non-negative terms. The bound on $\mu$ prevents their integer sum from wrapping in the field, which proves the inequality in Proposition C.1. Two traders presenting receipts for the same transition certify that contribution once. The update relation allows labels on padded rows; on a cross the final zero-padding checks and non-negative accumulation exclude a positive contribution there. On a no-cross the same conclusion applies to the ordinary columns.

## From that book to one clearing outcome

On a cross F1–F13 and the range lookup recover ordinary depths and $V_i=\min(D_i,S_i)$ as integer quantities. F14–F17 establish both the maximum and its entire interval. F18–F22 with F32 identify the lowest qualifying minimum-imbalance tick. [Appendix A](A-price-and-allocation.md) proves both that a candidate exists and that the predecessor check selects it. The IO recurrences and F33 bind eligible short-side IO interest at that price. At balance the scalar checks set $s=P=J=0$. Together these checks determine every field of the receipt on the very columns reconstructed from $h$. On a no-cross the ordinary recurrence and $D_iS_i=0$ imply $V_i=0$ everywhere. A positive-volume crossing receipt and a no-cross announcement therefore cannot both verify for the same accepted history under the assumptions above.

Conversely, for a bounded crossing book the honest witness uses the actual depths, surpluses and IO depths. The strict-neighbour slacks are the relevant gaps minus one and are non-negative wherever their selectors are active. The predecessor and plateau-neighbour checks occupy disjoint rows. The honest limb multiplicities and inverse helpers satisfy the lookup away from exceptional challenges. A valid ordinary no-cross book supplies its depths directly. This establishes feasibility of the auction relations. Proving cost, availability of the log and the exchange's willingness to publish are separate questions.

## Disclosure and the open privacy claim

For a single honest transition the fresh uniform blinding makes the commitment difference uniform for every fixed tick and quantity. Four independently blinded differences therefore conceal the side of an honest order in those differences alone. A simulator for the full view would have to reproduce a *single* opening and log that agree with colluding traders' receipts, an update proof tied to that log and a closing proof tied to its final commitments. It could use the disclosed schedule, the outcome and the colluding traders' own information. We have no such joint simulation result. The proposition establishes recorded contributions and correct clearing against a dishonest exchange; it gives no guarantee of admission, trader identity, whole-unit settlement or secrecy from the exchange.
