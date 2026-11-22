# Appendix F. Sampling, reconstruction and generator validation

## Sampling frame

The initial frame runs from 2 January through 13 November 2026. Calendar exclusions cover FOMC decisions, CPI and employment releases with their neighbouring trading sessions, monthly option expiries, index reconstitution, the election window, early closes and days adjacent to holidays. The study’s sampling plan uses the Federal Reserve calendar for FOMC events and the Bureau of Labor Statistics calendar for the specified labour and inflation releases ([4](references.md#ref-4), [31](references.md#ref-31)). The remaining 119 dates were shuffled with seed 20261117. The final screen also considers selected macro releases, central-bank minutes, month ends, earnings of AAPL and NVDA, AAPL ex-dividend dates and product events.

The market screen compares the absolute AAPL and QQQ returns with two historical standard deviations over a 60-day lookback. AAPL volume below half or above twice its 20-day median also flags a day, as does an AAPL or QQQ range above twice its 20-day median. Peer-stock and cross-asset checks flag large moves in six technology stocks and four sector or asset funds. The full rules and draw order are retained with the data scripts. The final screen passed 62 dates and excluded 57. We analyzed 23 January, 4 May, 9 July and 20 July. The 27 August comparison failed the final screen and is excluded from calibration and four-day rate estimates.

**Table F.1.** Feed records and one-second replay results for the four calibration dates. The last two columns use crossing intervals on the indicated day as their denominator.

| Date | Feed records | Crossing intervals | Volume plateau (%) | Tied minimum (%) |
| --- | --- | --- | --- | --- |
| 2026-01-23 | 3,176,565 | 11,800 | 2.16 | 0.07 |
| 2026-05-04 | 5,629,415 | 12,605 | 3.62 | 0.13 |
| 2026-07-09 | 6,065,233 | 12,337 | 3.38 | 0.35 |
| 2026-07-20 | 11,064,805 | 16,801 | 5.37 | 0.75 |

## Which observations enter each estimate

The unit of analysis for a daily rate is a crossing interval with at least one trade of identified aggressor side. Across the four interval partitions there were 156,950 crossings. No maximizing interval reached the boundary of its reconstructed price window, which extended 300 ticks beyond the interval trade-price range on each side. Every crossing had a qualifying price. These checks address truncation at the chosen window and implementation of the price rule. The messages originated in a continuous market.

**Table F.2.** Mean of four daily rates, giving each date equal weight. A tied minimum requires several maximizing ticks with the same minimum absolute imbalance.

| Interval | Mean crossing batches per day | Volume plateau (%) | Tied minimum (%) |
| --- | --- | --- | --- |
| 0.1 s | 23,131 | 2.10 | 0.33 |
| 1 s | 13,386 | 3.63 | 0.32 |
| 10 s | 2,331 | 2.85 | 0.06 |
| 60 s | 390 | 0.71 | 0.00 |

For the one-second plateau rate, the four-day mean was 3.63%. A descriptive Student $t$ interval using four daily observations gives approximately 1.5% to 5.7%. Pooling all one-second crossings instead gives 3.79% because more active days contribute more intervals. Neither calculation corrects for restricting the dates to screened sessions.

For prover evaluation we selected the first available volume plateau with one imbalance minimizer and the first with multiple minimizers for each day and duration. The selection includes the screened-out comparison day and yielded 32 books. This deliberately enriches the timing corpus for cases in which the price rule must resolve ties. Only the four analysis days enter the prevalence and calibration estimates.

## Generator validation by duration

Calibration retains up to 20,000 near-centre observations per duration. For occupied cells it interpolates empirical conditional quantiles, rounds to an integer and imposes a minimum of one unit. Outside the near region, 12 distance bands extend to 10,000 ticks. The default bootstrap preserves actual volume configurations in the central 21 ticks. For that reason, the generator code, aggregate validation statistics and the public worked example can be distributed separately from the calibration library and the generated volume files. The timing inputs use one-second calibration, seed 2027, and five draws per grid width.

**Table F.3.** Volume plateaus as a percentage of crossing books. The replay column pools intervals across the four analysis days. Each model drew 20,000 books of 601 ticks per duration. Each column uses its own crossing count.

| Interval (s) | Nasdaq replay | Independent cells | Joint occupancy | Joint volumes and outer totals |
| --- | --- | --- | --- | --- |
| 0.1 | 2.19 | 8.83 | 2.56 | 2.24 |
| 1.0 | 3.79 | 14.60 | 6.11 | 4.06 |
| 10.0 | 2.85 | 7.98 | 6.33 | 2.78 |
| 60.0 | 0.71 | 1.74 | 1.48 | 0.70 |

The independent model generates more volume plateaus than the replay at every duration. Preserving the central volumes and the outer totals gives the closest plateau rate of the three models at one second. [Section 6](06-data-and-generator.md) reports the corresponding imbalance ties and median executable volumes. The comparison measures preservation of calibration statistics on the sampled stock and dates.

## Matching the Cboe observations

Cboe’s participant FAQ describes the BXE auction update and summary messages and specifies that a summary is published when volume executes ([11](references.md#ref-11)). In our sample we matched flagged executions to periodic summaries by timestamp and compared the sum of execution sizes with each summary. The 362 summaries account for 400 executions. We also associated each of the 493 positive-price indicative updates with the next summary, out of 922 periodic updates. In 320 auctions the earliest associated positive-price update was within 100 ms of the summary. This interval is measured between messages. Interpreting it as the full auction duration would require the actual trigger time. Interpretation of historical fields is subject to the feed specification in force on the observation date.

For the 101-tick test books the reported price sets the centre, and the reported size sets both ordinary quantities there. This gives a check on quantity handling and the external tick-to-price conversion at observed venue outcomes. Neither losing orders nor the venue's choice among candidate prices can be inferred from these constructed books. The public release excludes raw feeds and the bootstrap library of licensed central volumes.
