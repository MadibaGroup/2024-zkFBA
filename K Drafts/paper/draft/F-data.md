# F Order-Flow Data and Tie Frequency

*Draft in Markdown. Label: `app:data`.*

This appendix gives the method behind Section 5.6 and the results that Table 6 summarizes.

## F.1 Source and Choice of Days

We use Nasdaq TotalView-ITCH [ITCH] for AAPL: every add, cancel, fill, and trade message with its order identifier. We choose the days by a procedure fixed in advance.

1. **Frame.** The 219 trading days from 2 January to 13 November 2026.
2. **Exclusions.** We remove days within one day of an FOMC decision, a CPI release, or an employment report, monthly option expiries, the index reconstitution day, the days around the election, early closes, and days next to holidays. 119 days remain.
3. **Order.** We shuffle the remaining days with a fixed seed.
4. **Screen.** We take the days in that order and keep a day unless it has a further scheduled event (other macro releases, central-bank minutes, month ends, earnings days of AAPL and NVDA and the day after, AAPL's ex-dividend and product-event days), an unusual session in the stock or in the market (a return beyond 2 standard deviations of the last 60 days, volume above twice or below half the 20-day median, a range above twice the median), or a shock in six large technology stocks or in semiconductor, oil, Treasury, or gold funds. 62 of the 119 days pass.

The analysis uses the first four passing days (Table 18). We also replay one day that the screen rejects, as a contrast.

**Table 18.** Days replayed.

| Day | Order messages | Role |
|---|---|---|
| 23 January 2026 | 3,176,565 | Analysis |
| 4 May 2026 | 5,629,415 | Analysis |
| 9 July 2026 | 6,065,233 | Analysis |
| 20 July 2026 | 11,064,805 | Analysis |
| 27 August 2026 | 5,092,753 | Rejected by the screen (day after NVDA earnings) |

## F.2 From Messages to Batches

We cut the regular session, 09:30 to 16:00 New York time, into consecutive batches of 0.1, 1, 10, or 60 seconds, on a one-cent grid. The bid volume of a batch at a price is the resting bid depth at the end of the interval, plus the displayed bid volume executed during it, plus the volume of buyer-initiated trades during it; ask volume is symmetric. An order cancelled within the interval is left out. A sub-cent price is rounded down for a buy and up for a sell. Trades without an aggressor side, which are crosses and executions against non-displayed orders, are 24% to 33% of trade messages and are left out. We clear each batch on a window of 300 ticks on either side of its trade prices.

The resting book matters for the result. A batch built from the interval's trades alone has a plateau almost every time; with the resting depth, a single maximizing price is the norm.

**Checks.** Reconstructed depth never goes negative on any day. Across the 156,950 crossing batches of the analysis days, no plateau touches the edge of its window and the rule always finds a candidate. The Python implementation of the rule used for the replay passes the test cases of the Rust implementation, and the 32 batches written out as benchmark books give the same receipt in both. These 32 are, for each of the five days and four batch lengths, the first plateau batch with one minimum-imbalance tick and the first with several, where one exists.

## F.3 Tie Frequency

Each crossing batch falls in one of three classes: one maximizing tick; a plateau with one minimum-imbalance tick; a plateau with several. Table 6 gives the means over the four days. At one second, 3.63% of crossing batches have a plateau, with a 95% interval of 1.5% to 5.7% from a $t$ interval over the four days. At 0.1, 10, and 60 seconds the figures are 2.10% (1.2% to 3.0%), 2.85% (1.6% to 4.1%), and 0.71% (0.5% to 0.9%). The intervals are wide because the day is the unit of observation and there are four.

**Table 19.** Per day: percentage of crossing batches with a plateau / with several minimum-imbalance ticks.

| Day | 0.1 s | 1 s | 10 s | 60 s |
|---|---|---|---|---|
| 23 January | 1.30 / 0.09 | 2.16 / 0.07 | 2.11 / 0.00 | 0.77 / 0.00 |
| 4 May | 2.25 / 0.14 | 3.62 / 0.13 | 2.44 / 0.00 | 0.77 / 0.00 |
| 9 July | 2.14 / 0.35 | 3.38 / 0.35 | 3.91 / 0.17 | 0.77 / 0.00 |
| 20 July | 2.72 / 0.72 | 5.37 / 0.75 | 2.95 / 0.09 | 0.51 / 0.00 |
| 27 August (rejected) | 1.80 / 0.39 | 3.33 / 0.48 | 3.25 / 0.00 | 0.51 / 0.00 |

The rejected day lies inside the range of the four analysis days at every batch length. On this evidence the screen decides which days are described more than it changes the numbers.

**Table 20.** By time of day, four analysis days pooled: plateau / several minimum-imbalance ticks, in percent.

| Batch length | First 30 minutes | Middle | Last 30 minutes |
|---|---|---|---|
| 0.1 s | 3.62 / 0.87 | 2.04 / 0.31 | 1.29 / 0.17 |
| 1 s | 5.92 / 0.86 | 3.70 / 0.32 | 2.01 / 0.13 |
| 10 s | 3.33 / 0.28 | 2.94 / 0.05 | 1.39 / 0.00 |
| 60 s | 0.00 / 0.00 | 0.76 / 0.00 | 0.83 / 0.00 |

At batch lengths up to ten seconds, ties are most frequent in the first half hour and least frequent in the last.

**The last step of the rule.** Over all four batch lengths, 546 crossing batches have several minimum-imbalance ticks. In 462 of them two candidates remain after the candidate condition of Definition 2, so the choice of the least candidate decides the price. Every step of the rule is reached in the data.

## F.4 The Generator

Benchmark books must have a chosen number of ticks and should tie as real batches do. We re-centre every crossing batch of the analysis days on its clearing tick and record, per batch length, the occupancy and volume quantiles of each tick within 100 ticks of the centre and of twelve bands out to 10,000 ticks, and the actual volumes within 10 ticks of the centre together with the total bid volume above and ask volume below them. We compare three generators on 20,000 books of 601 ticks each (Table 21).

- **Independent ticks.** Each tick is drawn on its own from the measured occupancy and volume distribution.
- **Core occupancy.** Which ticks within 5 of the centre hold volume is copied jointly from a real batch; the sizes are independent.
- **Bootstrap.** The volumes within 10 ticks of the centre are those of a real batch, the periphery is drawn from the marginals, and its totals are rescaled to that batch's.

**Table 21.** Tie classes of real and generated batches, in percent of crossing batches, and the median $V_{max}$. The real rows pool all crossing batches, so they differ slightly from the means over days in Table 6.

| Batch length | Source | One maximizing tick | Plateau, one minimum | Plateau, several | Median $V_{max}$ |
|---|---|---|---|---|---|
| 0.1 s | Real | 97.81 | 1.82 | 0.38 | 80 |
| | Independent ticks | 91.17 | 7.60 | 1.23 | 67 |
| | Core occupancy | 97.44 | 2.48 | 0.09 | 63 |
| | Bootstrap | 97.77 | 1.87 | 0.36 | 80 |
| 1 s | Real | 96.21 | 3.43 | 0.36 | 120 |
| | Independent ticks | 85.40 | 12.45 | 2.15 | 132 |
| | Core occupancy | 93.89 | 5.91 | 0.20 | 119 |
| | Bootstrap | 95.94 | 3.74 | 0.32 | 120 |
| 10 s | Real | 97.15 | 2.79 | 0.06 | 804 |
| | Independent ticks | 92.03 | 7.43 | 0.55 | 1,034 |
| | Core occupancy | 93.67 | 6.21 | 0.11 | 1,028 |
| | Bootstrap | 97.22 | 2.73 | 0.05 | 801 |
| 60 s | Real | 99.29 | 0.71 | 0.00 | 4,852 |
| | Independent ticks | 98.26 | 1.70 | 0.04 | 6,562 |
| | Core occupancy | 98.52 | 1.43 | 0.05 | 6,546 |
| | Bootstrap | 99.31 | 0.69 | 0.00 | 4,859 |

Independent ticks overstate ties three- to fourfold. Copying the occupancy pattern is close at 0.1 seconds and about double from one second upward, because independent sizes move the crossing point away from the centre. The bootstrap matches at all four lengths. This is by construction: on the core ticks the rule sees the real batch's depths. The agreement shows that plateaus almost never extend beyond 10 ticks from the clearing price. The bootstrap is a resampling of real batches with a synthetic periphery, not a parametric model of order flow.

The 25 calibrated benchmark books come from the bootstrap generator at one second, five each at 100, 250, 500, 1,000, and 2,000 ticks.

## F.5 Limits

Section 5.6 states the limits of scope: one stock, four days, flow from a continuous market, no imbalance-only orders. Two more concern the method. The percentages depend on the batch definition of F.2, in particular on including the resting book. The bootstrap library contains volumes from a licensed feed, so the terms of the licence decide whether the generated books can be published.
