# Session Handoff — Zeequent CS/Security Paper

Last updated: 2026-09-07 (after: drafting Appendix C in full, option (b) of
the Appendix B decision)

## What this file is
A standing handoff note for the "write the security/math/CS-heavy companion
paper to Zeequent Batch Auctions" project. Updated after every response in
the current session per Kimia's standing instruction, so a fresh session can
pick up without re-deriving context. If you are a new session reading this:
read this file fully before doing anything else on this project.

## The task
Write a security/math/CS-heavy version of `~/zk_fba_real_data/train/main.pdf`
(the finance-heavy "Zeequent Batch Auctions: An Overview" paper by Kimia
Esmaili, Elizabeth van Oorschot, Jeremy Clark). Target venue: **FC'27**
(Financial Cryptography and Data Security 2027, fc27.ifca.ai), Regular paper
category (15 pages + refs/appendices), LNCS Springer template, mandatory
anonymization. Deadline: 17 Sept 2026 (firm/extended 24 Sept 2026 23:59 AoE).

Structural model: `~/zk_fba_real_data/train/zeeperio.pdf` (Malhotra/Essex/
Clark, Eperio voting zk-SNARK paper) — used purely for cadence/sectioning,
not content. Shape: Abstract → Intro → numbered Contributions → Background →
one constraint walked through in full PIOP detail (vanishing poly → quotient
→ KZG opening → Schwartz-Zippel → batching via RLC) → remaining constraints
as tables/boxes → user-facing checks → Implementation & Performance
(benchmark table) → Security Proof (high-level in body, full proof in
appendix) → Conclusion → Appendices.

Technical content source: `~/zk_fba_real_data/train/protocol_constraints.md`
— the full 33-constraint spec (§14 has the complete numbered table). This is
what fills the new paper's constraint section (analogous to Zeeperio §3).

Voice: the `zkpwriting` skill (installed at
`~/.claude/skills/zkpwriting/SKILL.md`) — terse declarative openers, "we" as
grammatical subject with named systems as the object of "we" sentences, flat
unapologetic limitations, no adjectival inflation ("novel", "powerful" etc
banned), cite/footnote sourcing rather than narrating it, **never invent a
citation, benchmark number, or named system that wasn't given**.

**Standing instruction from Kimia: do not draft actual paper prose
unprompted.** She said she will supply her own draft section text in a later
prompt for editing/tightening in this voice. Until she does, this is a
research/reference-gathering phase, not a drafting phase.

## Where the implementations live
- `~/zk_fba/` — Rust/arkworks hand-rolled PLONK-style prover. BN254, KZG10,
  MadibaGroup gadgets. Only 5 constraints (V_AccA_init/rec, V_AccB_init/rec,
  V_KL) — a minimal proof-of-concept, NOT the full 33-constraint spec.
  Benchmarks: full pipeline ~27.52ms (criterion mean) / ~34ms (single run),
  proof size ~1,200 bytes. See `~/zk_fba/README.md` and `~/zk_fba/CLAUDE.md`.
- `~/zk_fba_noir/` — Noir/Barretenberg circuit implementing the FULL 33
  constraints from protocol_constraints.md §14. Packages `fba_protocol_100`
  (N=100 ticks) and `fba_protocol_1000` (N=1000 ticks). Masks (Mask_P/V/C)
  computed in-circuit from disclosed public scalars rather than witnessed —
  justified because the clearing receipt already discloses V_max, V_minΔ,
  c, d, p* regardless of masking approach (no privacy cost). Benchmarks:
  100-tick — 6,326 ACIR opcodes, 13,736 gates, proof 14,656B, prove 0.13s;
  1000-tick — 63,026 opcodes, 111,611 gates, prove 0.45s. See
  `~/zk_fba_noir/PROTOCOL_DESIGN_AND_RESULTS.md`.
- 5-constraint-only Rust-vs-Noir comparison (`~/zk_fba_noir/NOIR_VS_RUST.md`):
  proof gen 81.1ms (Noir) vs 10.56ms (Rust), pairing verify 12.3ms vs 15.71ms,
  proof size 14,656B vs ~1,200B, total 93.4ms vs 27.52ms.
- These two implementations are at different scopes (5 constraints vs 33) —
  the paper's Implementation & Performance section needs to state this scope
  difference explicitly rather than imply a fair apples-to-apples benchmark.

## Bibliography already compiled (delivered to Kimia this session)
**main.pdf's own 8 references** (verbatim, confirmed by direct PDF read this
session — main.pdf contains NO algebraic formulas at all, it explicitly
defers "concrete cryptographic details" to the GitHub repo):
1. Budish, Cramton, Shim — Implementation details for FBAs, AER 104(5), 2014
2. Budish, Cramton, Shim — HFT arms race, QJE 130(4), 2015
3. Bünz, Fisch, Szepieniec — Transparent SNARKs from Dark Compilers, EUROCRYPT 2020
4. Gabizon, Williamson, Ciobotaru — PLONK, ePrint 2019
5. Goldreich, Micali, Wigderson — How to prove all NP statements in ZK, CRYPTO 1986
6. Harris — Trading and Exchanges, Oxford 2003
7. Kate, Zaverucha, Goldberg — KZG, ASIACRYPT 2010
8. van Oorschot, Deng, Clark — Plonkbook, GitHub Pages 2024

**zeeperio.pdf refs relevant to protocol/security/math** (10 of 41 total):
[8] Boneh/Fisch/Gabizon/Williamson — range proof from poly commitments, 2019
[9] Brassard/Chaum/Crépeau — Minimum disclosure proofs, JCSS 1988
[10] Bünz/Fisch/Szepieniec — dup of main.pdf [3]
[14] Chiesa/Hu/Maller/Mishra/Vesely/Ward — Marlin, EUROCRYPT 2020
[19] Gabizon/Williamson/Ciobotaru — dup of main.pdf [4]
[21] Goldreich/Micali/Wigderson — dup of main.pdf [5]
[28] Kate/Zaverucha/Goldberg — dup of main.pdf [7]
[33] Lipmaa — PLONK simulation extractable in ROM, TCC 2025
[35] Maller/Bowe/Kohlweiss/Meiklejohn — Sonic, CCS 2019 (source of "MBKM heuristic")
[40] Sefranek — How (not) to simulate PLONK, SCN 2024
Also flagged [3] Basin/Dreier/Giampietro/Radomirović (CCS 2021) as a
**structural template only** for the security-proof section (formal
predicate-based reduction style), not a content citation.

**Gap identified**: main.pdf never cites Fiat-Shamir directly (uses GMW [5]
for the ZK definition instead), despite both implementations relying on it
explicitly. The new paper should add a direct Fiat-Shamir citation (Fiat &
Shamir, CRYPTO 1986 — already correctly cited in `~/zk_fba/README.md`'s own
reference list).

**Unverified suggested additions** (flagged to Kimia as NOT yet adopted,
per zkpwriting's "never invent a citation" rule — need her confirmation
before citing): Plookup 2020, Häbock 2022 shuffle argument (this one IS
already named and used in `protocol_constraints.md` §6, so it's a real,
minimum-effort citation to pull in, not invented — just need the exact
paper title/venue, not yet retrieved), Schwartz-Zippel, Huber/Küsters/
Liedtke/Rausch E-VOTE-ID 2024 (implementation/benchmarking framing),
Barretenberg/UltraHonk/arkworks/nargo as software-artifact citations.

## Constraint-discrepancy investigation (this session's task, now resolved)

Kimia asked me to check whether `~/zk_fba/README.md`'s "Known Document
Corrections" section describes real errors in the reference documents. That
section claims two (unnamed) reference documents contain three errors that
the Rust implementation silently corrects. I compared the claims against
`protocol_constraints.md` (re-read in full this session) and `main.pdf`
(re-read in full via direct PDF read this session). Finding, claim by claim:

**Claim 1 — "V_AccA_rec should use Ask(ω·X), not Ask(X)."** CONFIRMED REAL.
`protocol_constraints.md` §4 states the supply transition as
`(X−ω^{n−1})[AccA(ωX) − AccA(X) − A(X)] = 0` — using `A(X)`. But AccA is
defined (same section) as a forward cumulative sum, AccA[i+1] = AccA[i] +
A[i+1], i.e. in polynomial form `AccA(ωX) − AccA(X) − A(ωX) = 0`. The doc's
own stated recurrence uses the wrong argument (`A(X)` instead of `A(ωX)`) —
an off-by-one error. `~/zk_fba/CLAUDE.md`'s documented Rust formula already
uses `Ask(wX)`, i.e. the corrected form. **The new paper must state the
recurrence with `A(ωX)`, not copy protocol_constraints.md §4's literal
wording.**

**Claim 2 — "skip points are asymmetric (ω²⁰ for AccA, ω³¹ for AccB), not
both ω^(n−1)."** TRUE BUT NEEDS FRAMING, NOT A FLAT "ERROR". In the abstract
n-tick model (no padding), `protocol_constraints.md` uses one skip point
`ω^{n−1}` for both recurrences — internally consistent in that model. The
asymmetry only appears once you concretely instantiate with NTT-friendly
padding: Rust's real data domain is N=21 ticks, zero-padded to
DOMAIN_SIZE=32 for the Radix2 NTT. Under padding, "last real data tick"
(ω²⁰) and "last domain tick" (ω³¹) diverge: AccA (forward accumulator) must
skip ω²⁰ because crossing into the zero-padding region breaks its
recurrence's meaning there; AccB (backward accumulator) is self-consistent
through the zero-padded region and only breaks at the true domain
wraparound, ω³¹. **This is not a bug in protocol_constraints.md — it's an
unstated modeling gap: the doc describes the un-padded case, and the paper
should explicitly add a short "data domain vs. NTT domain" remark when
presenting this constraint**, rather than silently adopting either the
doc's single skip point or the Rust code's split skip points without
explanation.

**Claim 3 — "V_AccB_init should be at ω²⁰, not ω⁰."** NOT REPRODUCIBLE /
LIKELY STALE. `protocol_constraints.md` §4 already states the AccB
(demand) init at `ω^{n−1}` (= ω²⁰ under n=21) — matching the "corrected"
value, not ω⁰. (AccA's init is separately, correctly stated at ω⁰ — that
one was never in question.) I could not find any document in
`~/zk_fba_real_data/train/` that actually places AccB's init at ω⁰. This
claim in `~/zk_fba/README.md` appears to describe an error in some earlier
draft of the spec (not the current `protocol_constraints.md`) or is simply
a mistake in the README's own wording. **Do not repeat this specific claim
in the new paper — the current protocol_constraints.md is already correct
on this point.**

**Bottom line for the paper**: when presenting the AccA/AccB constraints
(Zeeperio-style, one constraint walked through in full detail), use the
Rust-corrected recurrence (`A(ωX)` not `A(X)`) and explicitly call out the
data-domain-vs-NTT-domain distinction behind the asymmetric skip points,
citing this as a concrete implementation subtlety worth a sentence (in
keeping with zkpwriting's "state limitations/subtleties flatly" trait) —
but do not carry forward README's claim #3 (ω⁰ vs ω²⁰ for AccB_init), since
it doesn't match any current source document.

**Still not done**: this constraint-formula analysis has only been run
against the 5-constraint Rust MVP (V_AccA/V_AccB/V_KL). It has NOT been
cross-checked against the Noir implementation's 33-constraint version or
against protocol_constraints.md §4's constraints #3-#6 in the *numbered
constraint table* framing used elsewhere in that doc (only the prose
version in §4 was checked). If the paper ends up presenting the 33-constraint
version as primary (likely, since that's the complete spec), re-verify
these same three claims against `fba_protocol_100/src/main.nr` /
`protocol_main_template.nr` in `~/zk_fba_noir/` for consistency, since that
implementation is the more complete/current one.

## Open threads / next steps
1. ~~Kimia has not yet supplied her own draft text~~ — SUPERSEDED 2026-09-07:
   Kimia explicitly asked me to start drafting Abstract/Intro myself
   ("start writing in the cadence for this conference... do abstract and
   intro"), then asked for "the next part" (Background/Related Work). The
   old "wait for her draft" instruction no longer applies — I am now
   actively drafting the paper section by section, in the zkpwriting voice,
   pending her review/edits after each part.
2. The exact Häbock 2022 citation (title/venue) should be pulled from
   wherever `protocol_constraints.md` sourced it, or asked of Kimia directly
   — not guessed. Flagged inline in `paper_draft.md` §2.3 as an open
   editorial note.
3. ~~Pull constraint definitions/equations directly from
   protocol_constraints.md~~ — DONE: §3 (constraint-walkthrough section)
   is written, corrections applied.
4. Reconcile FC'27 formatting (LNCS template, 15pp Regular, anonymization)
   with the eventual draft's structure once drafting starts. Note:
   `paper_draft.md` already carries an editorial note that the author
   block is omitted for double-blind compliance, and that the self-citation
   to the companion overview paper (main.pdf) needs an anonymized citation
   form before submission — currently just referred to in prose as "a
   companion paper" with no bracket number, deliberately, to avoid
   inventing a self-citation format before Kimia decides on one.

## Paper draft status (`~/zk_fba_real_data/paper_draft.md`)
Live draft file — read this first if resuming cold, it has more detail
than this summary. Sections written so far, in the FC'27/Zeeperio cadence
(Abstract → Intro → numbered Contributions → Background/Related Work →
[constraint walkthrough, not yet started]):

- **Title**: placeholder only ("Zeequent: A Polynomial IOP for Frequent
  Batch Auctions") — Kimia should pick her own variant.
- **Abstract**: written. States the problem (companion paper leaves crypto
  construction unspecified), what we supply (33-constraint PIOP, two
  gadgets, KZG10+Fiat-Shamir), and gives the headline Noir benchmark
  (1000-tick: 111,611 gates, 14,656-byte proof, 0.45s prove) as the answer
  to main.pdf's own "not necessarily cheap enough" open question.
- **§1 Introduction**: written. Ends in a 4-item numbered Contributions
  list (constraint spec, two shared gadgets, computed-mask construction,
  two implementations at two scopes).
- **§2 Background and Related Work**: written, 3 subsections — 2.1 FBA
  recap (defers to companion paper + Harris [6] for market mechanics,
  doesn't re-derive), 2.2 ZK/PIOP background (GMW [5], Bünz et al PIOP
  model [3], PLONK [4], Plonkbook [8] gadget vocabulary; explains *why* we
  reject universal circuit compilers — Marlin [10], Sonic [11] — in favor
  of hand-encoding the fixed 33-constraint set), 2.3 KZG10 [7] +
  Fiat-Shamir [9] (why trusted setup is an acceptable tradeoff for FBA
  specifically: small periodic proof beats transparent setup here).
- **§3 The Constraint System**: written, 4 subsections — 3.1 formal
  notation ($H$, $\omega$, $Z_H$, $\mathsf{cm}[\cdot]$, disclosed scalars),
  3.2 full walkthrough of the Supply Depth Transition constraint
  (vanishing identity → quotient → KZG opening incl. shifted eval at
  $\omega\zeta$ → Schwartz-Zippel soundness argument → PLONK-style [4]
  batching via random linear combination), including the confirmed
  $A(\omega X)$-vs-$A(X)$ correction from the earlier discrepancy
  investigation folded in as a "note on the argument to A" paragraph, and
  the padding/skip-point subtlety folded in as a closing paragraph (NOT
  framed as a doc error — framed as "only appears once you fix a concrete
  domain size"). 3.3 gives the full 33-row constraint table (Table 1,
  adapted from `protocol_constraints.md` §14) with an added "Pattern"
  column tallying which constraints use bit-decomposition (8), Plookup
  (2), or the informal recurring "Product-zero" shape (booleanity/mutual-
  exclusivity/membership — deliberately NOT counted as a third formal
  gadget, just named for readability). 3.4 explains why $Mask_P$/$Mask_V$/
  $Mask_C$ are computed from disclosed scalars ($c$, $d$, $p^*$) rather
  than witnessed via shuffle/permutation — ties back to Contribution #3.
- **§4 User-Facing Checks**: written, 3 subsections — 4.1 The Update
  Receipt (formalizes main.pdf's "opening proof showing the committed
  polynomial changed by only the correct amount at only the correct tick"
  as $B_{new}=B_{old}+q\cdot L_\ell$, lifted to commitments via KZG10's
  additive homomorphism; key point: this specific check needs NO pairing
  and NO random challenge, just a scalar-mult + group-add, because
  perfect binding + linearity together certify "nothing else changed" for
  free). 4.2 What a Trader Checks After Close (eligibility against
  disclosed $p^*$ is pure public arithmetic, no crypto needed at all;
  the only place a trader needs an actual opening is confirming their own
  pro-rata ration at $p^*$, costing one KZG opening/pairing regardless of
  book depth). 4.3 Division of Verification Labor (the load-bearing
  framing point: full-batch verification cost scales with book depth
  (→ Section 5's benchmarks), but per-trader verification is O(1)
  regardless of book depth — this is the bridge into Section 5).
- **§5 Implementation and Performance**: written, 5 subsections. 5.1
  "Three Circuits, Not Two" — explicit disambiguation established this
  session after re-reading `~/zk_fba_noir/NOIR_VS_RUST.md` in full: there
  are THREE distinct benchmarked circuits, not two as implicitly assumed
  earlier — (a) Rust/arkworks 5-constraint MVP (N=21, domain 32), (b) Noir/
  Barretenberg 5-constraint-MATCHED circuit (same 5 constraints as (a), but
  domain forced to 4,096 by UltraHonk's wire/permutation/lookup structure,
  689 ACIR opcodes, 3,970 gates — this is the circuit used ONLY for the
  apples-to-apples Rust-vs-Noir comparison), (c) Noir/Barretenberg FULL
  33-constraint circuit at N=100 and N=1000 (a DIFFERENT circuit from (b),
  6,326/63,026 opcodes, 13,736/111,611 gates). This distinction is now
  baked into the paper text itself so readers don't conflate (b) and (c).
  5.2 "Same Constraints, Different Implementation" table — Rust vs
  Noir-5-constraint-matched: domain 32 vs 4,096 (128x), witness gen 0.067ms
  vs 82.7ms, proof gen 10.56ms vs 81.1ms (7.7x slower), pairing verify
  15.71ms vs 12.3ms (1.3x faster for Noir, with an explicit methodological
  caveat that Rust's number is from a diagnostic 26-individual+4-batched
  measurement, not a clean apples-to-apples verify call), total 27.52ms vs
  93.4ms (3.4x slower), proof size ~1,200B vs 14,656B (12x larger). Cites
  NOIR_VS_RUST.md's own explanation for the 7.7x gap (128x domain gap →
  307x raw op ratio → ~22x/9x after 14-thread parallelism → measured 7.7x
  is consistent) and its own explanation for Noir's faster pairing verify
  ("Barretenberg's BN254 pairing implementation is more optimised than the
  arkworks one"). 5.3 "Scaling the Complete Constraint Set" table — N=100
  vs N=1000 full-33-constraint circuit: opcodes 6,326 vs 63,026, gates
  13,736 vs 111,611, proof size 14,656B BOTH, prove 0.13s vs 0.45s, verify
  0.02s vs 0.01s, both PASS — plus the striking empirical observation that
  ALL THREE measured circuits (3,970 / 13,736 / 111,611 gates) produce
  identical 14,656-byte proofs (UltraHonk's constant-verifier-cost /
  constant-proof-size property, confirmed empirically not just asserted),
  plus a mention of the negative-control validity test (incrementing V_max
  by 1 fails at the plateau-endpoint constraint, confirming constraints are
  load-bearing not vacuous). 5.4 "Rust as a Verification Tool, Not a
  Deployment Candidate" — Table 2, the 11-row Rust per-layer benchmark
  breakdown from `~/zk_fba/CLAUDE.md`, cost breakdown (~42% KZG setup+MSM
  commits / ~57% batch_open+batch_check+range gadget / <1% everything
  else), flags the amortizable trusted-setup cost. 5.5 "What This Means for
  a Deployed Auction" — ties back to main.pdf's own "not necessarily cheap
  enough... future work" open question; states 0.45s at N=1000 fits a
  "once a second" cadence with room to spare; proposes a pipelined design
  for sub-second cadences but explicitly flags it as UNBUILT/UNMEASURED,
  not a claimed result.
- **§6 Security Proof**: written, 3 subsections plus a two-adversary framing
  intro (malicious exchange → soundness; curious verifier/trader → privacy).
  6.1 Completeness (short, direct — honest prover satisfies every identity
  pointwise by construction). 6.2 Knowledge Soundness — single-constraint
  Schwartz-Zippel argument (reuses §3.2), batching's union bound over 33
  constraints (still negligible, 254-bit field), KZG10 binding reduces to
  t-SDH [7]; explicitly notes that "verifier's equation checks out" →
  "prover knew a real witness" needs an extractor construction, which is
  DEFERRED to Appendix A rather than sketched inline, citing two newly
  assigned refs for why this step is subtle: **[12] Lipmaa, PLONK is
  simulation extractable in the ROM, TCC 2025** (= zeeperio's [33]) and
  **[13] Sefranek, How (not) to simulate PLONK, SCN 2024** (= zeeperio's
  [40]) — both already vetted in the bibliography section above, newly
  cited in-text for the first time this turn. Also folds in a protocol-
  specific soundness point: §3.4's computed masks need NO extra soundness
  argument since the verifier recomputes them from disclosed scalars
  directly — removes an attack class rather than adding one. 6.3 Privacy
  — describes the verifier's entire view (receipt + commitments + openings
  at ζ/ωζ + one per-trader opening at their own tick, §4.2) and states what
  an HVZK simulator argument WOULD look like (program Fiat-Shamir hash +
  KZG10 hiding blinding) — but then **flags a real, undissolved gap
  rather than claiming ZK is done**: pulls the exact inline comment from
  `~/zk_fba/CLAUDE.md`'s "Known Inefficiencies" #3 ("Hiding required by
  batch_open even though FBA proof is not zero-knowledge") as documented
  evidence that the Rust prototype's hiding commitments are a gadget
  side-effect, NOT a checked ZK guarantee — no simulator exists yet at
  either implementation scope, and this is stated as future work, not a
  claimed result. **Caught a leak while drafting**: an early draft of 6's
  intro cited "[zeeperio, Malhotra/Essex/Clark]" as the source of the
  body/appendix security-proof split — this was meta-commentary about the
  PAPER'S OWN structural model bleeding into the manuscript text itself
  (self-referential and a double-blind risk), caught via a grep sweep for
  "zeeperio" across the whole draft immediately after writing, and removed
  before this turn ended. Worth remembering as a check to repeat after
  future sections: grep the draft for internal planning vocabulary
  ("companion paper" is fine/intentional, but "zeeperio"/"structural
  model"/"per Kimia" etc should never appear in manuscript prose).
- **§7 Conclusion**: written, single section, no subsections (matches
  Zeeperio's own short-conclusion cadence). Restates the paper's core move
  (crossing the arithmetization boundary the companion paper stopped at),
  cites Section 5's 0.45s/1000-tick number as the concrete answer to the
  overview paper's open question, then draws the line at what §6 actually
  established vs. left open — explicitly names closing the zero-knowledge
  gap (no simulator built yet, per §6.3) as "the most direct piece of
  future work this paper leaves," ahead of the pipelined-proving idea from
  §5.5. Deliberately does NOT re-list the 4 contributions verbatim (would
  be redundant with §1) — ties back to them by content instead.
  **Appendix A: Knowledge-Soundness Reduction** — skeleton/placeholder
  only (bracketed editorial note), scoped explicitly: formalize the §6.2
  extractor construction using KZG10 binding [7] + Schwartz-Zippel (§3.2),
  and must actually engage with [12]/[13]'s batching-specific subtleties
  rather than just restate the single-constraint case. **Appendix B:
  [reserved]** — placeholder only, flags an open decision for Kimia: either
  (a) per-constraint non-expansivity arguments for the other 32 rows of
  Table 1 (Zeeperio's own structural pattern — one constraint in full,
  rest tabulated with a lighter per-item argument in an appendix), or (b)
  implementation/ABI details for the Rust/Noir circuits. Not decided yet.
  **Checked for the same "zeeperio" leak caught last turn** — one hit
  remains at line ~779, but it's inside Appendix B's own bracketed
  editorial note (an aside to Kimia about what to appendix, not manuscript
  prose), same category as the title note at the top of the file — judged
  fine, not a repeat of the earlier leak.
- **Appendix A: Knowledge-Soundness Reduction — now written in full**, 4
  subsections. A.1 states the relation $\mathcal{R}_{\mathsf{FBA}}$
  (commitments to every Table-1 column + disclosed scalars; witness
  satisfies it iff all 33 identities hold on $H$) and the two-stage
  extractor plan. A.2 extracts each committed polynomial individually via
  a standard rewinding argument (query $\deg(W)+1$ distinct $\zeta$'s,
  interpolate; a mismatch would mean a double-opening of one KZG10
  commitment, breaking the same $t$-SDH-style assumption §6.2 already
  invokes [7]) — grounded in the fact (confirmed by re-reading §3.2's own
  "Batching" paragraph) that every witness/quotient column is committed
  BEFORE the batching challenge α is drawn, so this stage doesn't need α
  at all. A.3 is the actual novel content of this appendix: treats the
  batched residual as a polynomial *in the indeterminate α* (degree ≤32),
  and applies Schwartz-Zippel a SECOND time — over α rather than ζ — to
  argue passing the one combined check forces every individual r_i(ζ)=0,
  not just their combination. Explicitly flagged as a second, independent
  application of the lemma, not a restatement of §3.2/§6.2's per-constraint
  argument (that one is over ζ; this one is over α). A.4 is an honest
  scope-limitation section: names what this reduction does NOT cover —
  plain knowledge soundness only covers a single accepting proof, not an
  exchange publishing one proof per batch indefinitely against reused
  public parameters (same SRS, same cm[L_ℓ] Lagrange commitments reused
  across every batch's update receipts per §4.1) — that's a simulation-
  extractability question, which is exactly what [12]/[13] treat for
  PLONK's batching in the ROM, and explicitly states we have NOT
  re-derived either paper's argument against this protocol's specific
  cm[L_ℓ] reuse. This is real, substantive engagement with why [12]/[13]
  are cited (not a hand-wave name-drop) — ties directly back to the
  simulation-extractability framing already set up in §6.2.
  **Zeeperio-leak check repeated**: grepped the whole draft again after
  writing — one hit remains at the Appendix B placeholder (bracketed
  editorial note, not manuscript prose, same as last turn's judgment).
- **Appendix B: Non-Expansivity of the Remaining 32 Constraints — now
  written in full**, option (a) (Kimia chose per-constraint non-
  expansivity arguments over implementation/ABI details). Re-read
  `~/zk_fba_real_data/train/protocol_constraints.md` in full again this
  turn to pull every one of the 32 remaining constraints' exact equations
  directly from source (§14's Full Constraint List) rather than
  re-derive/invent them. B.1 establishes a general lemma: every remaining
  constraint reduces to a relation × one of four gate types (no gate/full
  domain, point-quotient $Z_H(X)/(X-\omega^k)$, single-point Lagrange
  $L_k(X)$, or public mask $Mask_P/Mask_C/(1-Mask_P)/(1-ChkD)$) — for all
  four, $\deg(V_i) \le 2n-2$ hence $\deg(Q_i) \le n-2$, inside the same
  bound Appendix A.2's rewinding extraction already assumes; this is the
  paper's own definition of "non-expansive." Also makes an important
  correctness point: constraints Table 1 marks "Plateau only"/"Cliff tick
  only" (23, 31, 32) are NOT a different verification recipe — the public
  mask gate is zero outside its support, so the identity holds vacuously
  there and is checked via the same $Z_H$-divisibility as every full-domain
  constraint; "Active domain" describes where a constraint has bite, not
  a different check. Also flags Plookup (1, 2) as the one place the
  single-relation template doesn't literally apply (3 sub-equations each,
  per the Shared Gadgets section) but shows their sub-equations still only
  use gate types B.1 already covers (Lagrange boundary pins + a
  $(X-\omega^{n-1})$-gated transition identical in shape to Constraint 6's
  own gate) — no new machinery needed. B.2 is a full 32-row table (all of
  1–5, 7–33) giving each constraint's exact relation (verbatim from
  protocol_constraints.md), its gate type, and a one-line note — including
  correctly identifying that Constraint 5 (Demand transition) does NOT
  have the same off-by-one bug as Constraint 6 (Supply transition, already
  corrected in §3.2's body text): traced through the actual recurrence
  direction (AccB summed downward from the highest tick, so the "new" tick
  when descending is the current point $X$ itself, not $\omega X$) and
  confirmed the doc's own $AccB(X)-AccB(\omega X)-B(X)=0$ is already
  correct as stated, unlike Constraint 6's doc text which needed the
  $A(X)\to A(\omega X)$ correction — this is a genuine, checked distinction,
  not an assumed symmetry. Also flags that Constraints 3/4 are single-point
  base-case pins (not recurrences) so no analogous off-by-one question
  even arises for them. Closes with an honest note: 3 and 5 are argued as
  mirror images of 4 and 6 rather than independently re-derived — same
  degree bound and Schwartz-Zippel argument, direction/accumulator
  swapped, not claimed as a new independent proof.
- **Appendix C: Implementation and ABI Details — now written in full**,
  option (b) of the earlier Appendix B decision (implementation/ABI details
  for the Rust and Noir circuits, as a separate appendix rather than a
  replacement for Appendix B's non-expansivity content — both options are
  now written, each as its own appendix). Re-read source files fresh rather
  than relying on memory (per zkpwriting's "never invent a citation,
  benchmark number, or named system" rule): `~/zk_fba/src/lib.rs` (grepped
  every `pub fn`/`pub struct`/`pub type`, then read the `OrderBook` struct's
  full field list and the `PipelineResult` struct + `full_pipeline` function
  body verbatim), `~/zk_fba/src/main.rs` (confirmed it's a thin CLI demo
  importing the library, not additional API surface), `~/zk_fba/Cargo.toml`
  (exact dependency list/versions), `~/zk_fba_noir/protocol_main_template.nr`
  (the `fn main(...)` ABI signature), `~/zk_fba_noir/fba_protocol_100/
  Prover.toml` (field names, to confirm they match the circuit ABI — did NOT
  quote the actual sample data values in the appendix, only the ABI/field
  names), and `~/zk_fba_noir/fba_protocol_100/Nargo.toml`.
  C.1 (Rust/arkworks Prover) gives the four dependency groups (ark-ff/
  ark-poly/ark-bn254/ark-ec/ark-std, ark-poly-commit, MadibaGroup gadgets,
  sha2), the `OrderBook` struct verbatim, and the fixed pipeline call
  sequence (`build_domain` → `build_all_polys` → `commit_all`/
  `compute_quotients`/`commit_quotients` → `fiat_shamir_prove` →
  `compute_all_opening_proofs`/`verify_all_openings` → `prove_mcv_range`/
  `verify_mcv_range`), plus the `PipelineResult` struct and its
  `all_pass()` method — framed as mirroring, at the level of one function
  call, the same "every residual must vanish, not just their combination"
  argument Appendix A.3 makes formally, while being explicit that this is a
  diagnostic API (separate `constraint_result`/`quotient_check` fields) not
  a minimal production one. C.2 (Noir/Barretenberg Circuit) gives the full
  33-constraint circuit's `fn main(...)` ABI (private witness arrays `b`,
  `a`, `acc_b`, `acc_a`, `min_x`, `surp_b`, `surp_a`, `delta`, `slack_l`,
  `slack_r`; public scalars `v_max`, `v_min_delta`, `c`, `d`, `p_star`),
  explains the compile-time array-length global `N` forcing two separately
  compiled packages (100, 1000) rather than one parametric circuit, and
  makes the one substantive design point: derived columns (AccB, AccA, Min,
  SurpB, SurpA, Delta) are supplied as witness inputs directly rather than
  computed in-circuit from B/A alone, with correctness enforced via asserts
  (Appendix B's table) plus an independent CSV cross-check at
  witness-generation time — not trusted blindly. Closes with the
  `nargo`/`bb` CLI toolchain commands.
  **Anonymization finding, standing constraint going forward**:
  `~/zk_fba_noir/fba_protocol_100/Nargo.toml` contains the line
  `authors = ["Kimia Esmaili, Concordia University"]`. **This string must
  NEVER be quoted or referenced in `paper_draft.md` or any other
  manuscript-facing content** — FC'27 requires mandatory double-blind
  anonymization. Appendix C deliberately describes the Noir package
  generically ("a plain Noir binary package with no external dependencies")
  without naming its `authors` field. Verified via
  `grep -ni "zeeperio\|Kimia Esmaili\|Concordia" paper_draft.md` after
  writing this appendix — the only hit is the pre-existing bracketed title
  placeholder note at the top of the file, not manuscript prose; no leak.
- **Draft is now fully complete, all sections and all three appendices
  written.** Abstract, §1–§7, Appendix A, Appendix B, Appendix C all have
  real, substantive content — no placeholders remain except the title
  (still a bracketed placeholder) and the two flagged editorial notes
  (Häbock 2022 citation in §2.3/§3.4; anonymized self-citation format for
  the companion overview paper). Remaining open items, in rough priority
  order: (1) the Häbock 2022 citation (exact title/venue, not yet pursued),
  (2) picking a real title, (3) a first full read-through/revision/
  tightening pass across every section now that the full skeleton exists,
  (4) the anonymized-citation format decision before submission. There is
  no more "next section" or "next appendix" to write — any further "do X"
  from Kimia is now an editing/revision instruction against existing
  content, not a drafting one.

## Citation numbering established this session (for consistency across
## future sections — do not renumber without updating all of paper_draft.md)
Reuses main.pdf's own [1]-[8] numbering, then continues:
[1] Budish/Cramton/Shim AER 2014 · [2] Budish/Cramton/Shim QJE 2015 ·
[3] Bünz/Fisch/Szepieniec EUROCRYPT 2020 (transparent SNARKs / PIOP model)
· [4] Gabizon/Williamson/Ciobotaru PLONK ePrint 2019 · [5] GMW CRYPTO 1986
(ZK definition) · [6] Harris, Trading and Exchanges, Oxford 2003 ·
[7] Kate/Zaverucha/Goldberg KZG ASIACRYPT 2010 · [8] van Oorschot/Deng/
Clark Plonkbook 2024 · **[9] Fiat/Shamir CRYPTO 1986 (new addition this
session — confirmed real, taken from `~/zk_fba/README.md`'s own reference
list, not invented)** · **[10] Chiesa/Hu/Maller/Mishra/Vesely/Ward, Marlin,
EUROCRYPT 2020 (= zeeperio.pdf's [14])** · **[11] Maller/Bowe/Kohlweiss/
Meiklejohn, Sonic, CCS 2019 (= zeeperio.pdf's [35])**. [10] and [11] are
newly assigned numbers in *this* paper's bibliography, reusing content
already vetted in the "Bibliography already compiled" section above (they
were zeeperio.pdf refs [14] and [35] respectively) — not new unverified
additions. **[12] Lipmaa, PLONK is simulation extractable in the ROM, TCC
2025 (= zeeperio's [33]) and [13] Sefranek, How (not) to simulate PLONK,
SCN 2024 (= zeeperio's [40]) — both now cited in-text in §6.2, on why the
extractor construction is deferred to Appendix A rather than sketched
inline.** Basin/Dreier/Giampietro/Radomirović [zeeperio 3] was considered
as a structural template for the §6 reduction-style proof but NOT cited —
§6 as drafted doesn't actually adopt its formal predicate-based modeling
technique, so citing it would be name-dropping rather than sourcing a
real claim; revisit only if Appendix A ends up adopting that specific
technique. Next unused number is [14].
