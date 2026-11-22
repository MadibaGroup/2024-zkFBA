# Appendix H. From a price-row constraint to a polynomial opening

## A demand recurrence on two ticks

Take two real price ticks in increasing order, with six units bid at the lower tick and two at the higher. The corresponding ordinary demands are eight and two. Over the scalar field let $H=\{1,-1\}$, with $1$ indexing the lower tick. The unmasked interpolants are

$$
 B(X)=4+2X,\qquad D(X)=5+3X.
$$

They recover the four quantities by evaluation. Moving to the next row replaces $X$ by $-X$. At the lower tick we need $D(1)-D(-1)=B(1)$. At the higher tick there is no successor, so we check $D(-1)=B(-1)$ separately. Multiplying the recurrence by $X+1$ disables it at that last row. Substitution gives

$$
 F(X)=(X+1)\bigl(D(X)-D(-X)-B(X)\bigr)
     =(X+1)(4X-4)=4(X^2-1).
$$

The constraint holds at both points of $H$, although $F$ is a nonzero polynomial. Its quotient by the vanishing polynomial $Z_H(X)=X^2-1$ is the constant $4$. Now suppose the exchange reports demand seven at the lower tick and leaves the other entries unchanged. The boundary still holds, but the recurrence gives $F(1)=2(7-2-6)=-2$. No polynomial $Q$ can then satisfy $F=QZ_H$, since $Q(1)Z_H(1)=0$ for every $Q$.

The clearing proof enforces the identities in [Appendix B](B-constraint-relations.md). After the lookup helpers are fixed, the challenge $\alpha$ weights the identities to limit cancellation between errors. The exchange then commits to the quotient, after which a fresh challenge selects the point at which the verifier tests the identity ([16](references.md#ref-16)). For a fixed false identity, the discrepancy between its two sides is a nonzero polynomial. If its degree is $d$, it has at most $d$ roots. This accounts for the order of the two challenges. The integrity claim for the full batch also depends on the opening, updates and shared commitments specified in [Appendix C](C-security-proofs.md).

## Authenticating an evaluation

The quotient test requires evaluations consistent with commitments from earlier rounds. We obtain that guarantee from KZG ([20](references.md#ref-20)). For one book column write $b$ for its interpolant and $f=b+\rho_0 Z_H$ for the masked polynomial. Thus $f(x)=b(x)$ at a price row $x\in H$. Let $r$ be the commitment blinding polynomial. In the notation of [Section 4](04-committed-book.md), the published commitment is $C=[f(\tau)]_1+[\gamma r(\tau)]_1$. Brackets denote scalar multiples of the group generators. The reference string lets the prover compute these group elements from polynomial coefficients without learning the setup secrets $\tau$ or $\gamma$.

For a single opening at $z$, let $y=f(z)$ and $u=r(z)$. The numerators below both vanish at $z$, so both divisions yield polynomials.

$$
 w_f(X)=\frac{f(X)-y}{X-z},\qquad
 w_r(X)=\frac{r(X)-u}{X-z}.
$$

The evaluation witness is $W=[w_f(\tau)]_1+[\gamma w_r(\tau)]_1$. Bilinearity turns the two factorizations into the verifier’s pairing check.

$$
 e\bigl(C-y[1]_1-u[\gamma]_1,[1]_2\bigr)
   = e\bigl(W,[\tau]_2-z[1]_2\bigr).
$$

Under evaluation binding, the prover cannot produce valid openings to two different values $y$ at the same point for the same commitment, except with negligible probability. The check authenticates the evaluation supplied to the verifier. The auction constraints determine whether those authenticated values support the announced receipt.

In our Rust implementation the clearing verifier combines openings for several columns at the same point and batches the checks at $\zeta$ and $\omega\zeta$. The shifted point supplies the successor values used in recurrences such as the one above. The verifier receives evaluations of masked columns, and the single-column blinding evaluation in the displayed equation is aggregated in the batched opening. The degree of a mask limits how many independent evaluations it can conceal. [Section 5](05-security.md) discusses the consequences of reusing the book commitments across the batch.
