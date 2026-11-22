# Security of the committed book

The Rust verifier checks the computation on one published history. A deployment must identify the instrument, price grid and batch interval, and give every participant the same final log digest. Those details determine what a verified tick means as a quoted price.

## Empty opening and log

Each column starts with a masked commitment to zero. The opening includes a Schnorr proof for the blinding component and a pairing check for the multiple of $Z_H$. Each subsequent log entry hashes the previous digest, position, column and new commitment. The verifier recovers a column's change by subtracting its previous commitment. Collision resistance binds the sequence to a given head. Two parties who accept different heads can verify different histories.

## Trader checks

For column $j$, tick $\ell$ and quantity $q$, the receipt supplies two commitments and a blinding $\rho$. The trader checks

$$C_{after}-C_{before}=q[L_\ell(\tau)]_1+\rho\Gamma.$$

The trader compares $j,\ell,q$ with their request and checks the transition against consecutive states of column $j$ in the common log. This includes the opening for the first change. Receipts for the same transition establish one contribution. The receipt carries no authenticated identity or order-arrival time. A market order has the same encoding as a limit order at the corresponding extreme tick. Anyone given the receipt can check those fields.

## Closing an accepted history

The update proof checks each commitment difference against a bounded non-negative contribution at a single point in the padded domain. Zero is valid. The honest exchange posts one group of four steps per accepted order, but the verifier checks individual differences. It establishes neither a group of four for each real submission nor exactly one positive difference in a group. The exchange can refuse submissions or add interest of its own.

Let $\mu$ be the number of steps. Under knowledge soundness, consistent algebraic extraction of the hiding commitments, lookup soundness, sound empty openings, collision resistance, a secure reference string and $\mu(2^{k_{in}}-1)<|\mathbb F|$, a distinct checked contribution survives in the final sum. The [constraint list](fixedConstraints.md) gives the update histogram and the crossing and no-cross relations. The crossing relation checks the ranges and padding of all four final columns and fixes eight public fields. A no-cross proof checks only the two ordinary final columns and their depths. The update proof still checks the differences in all four columns.

The outcome is unique for a fixed accepted history and rule. This claim concerns the computation of the tick and aggregate entitlements before rounding. Timely admission, settlement, and assigning residual whole units are separate questions. The argument assumes soundness for the custom transcript and externally supplied commitments. We have no end-to-end reduction for the exact implementation.

## Privacy

The exchange receives each order in plaintext. In an honest update, the three unused columns move by independent blindings and the active column moves by a blinded contribution. For fixed $q$ and $\ell$, uniform $\rho$ makes the active difference uniform in the group. Observers can count published entries and time their publication. The closing receipt reveals aggregate values, which may identify individual interest in a thin book.

A joint privacy result would require a simulator for the opening, log, disclosed receipts and both proofs on the same commitments, given only the specified public leakage and the participating traders' own information. Hiding a single log difference and counting the masked evaluations in a closing proof do not establish that result. Reclosing the same honest private state reproduces the same proof bytes. The protocol assumes at most one distinct closing transcript per opening.
