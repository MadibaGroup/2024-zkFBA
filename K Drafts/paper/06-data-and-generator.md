# 6. Order flow and a calibrated synthetic generator

To test the price tie-break we need books with losing orders and with several ticks that attain the same executable volume. Generating each cell independently makes books of any width easy to obtain, but changes how often several adjacent ticks have the same executable volume. We built a generator from reconstructed Nasdaq order flow and used Cboe's published outcomes for a separate check on prices and sizes observed in live auctions.

## Sampling and replay

We reconstructed Nasdaq TotalView-ITCH order activity for AAPL using the Databento `XNAS.ITCH` dataset ([13](references.md#ref-13), [23](references.md#ref-23)). The sampling objective was to limit the influence of major shocks on calibration. We excluded scheduled event windows, shuffled the eligible dates with a fixed seed and screened them using market and stock indicators. Of 219 trading sessions in the frame, 119 remained after the calendar exclusions and 62 passed the final screen. We took the first four passing dates in shuffled order. The screen was extended after initial replay results became available. The extension left the four analysis dates unchanged. [Appendix F](F-data.md) gives the dates, thresholds and role of the excluded comparison day.

This is a conditional distribution of order flow on screened sessions. Screening reduced exposure to specified events and large observed moves. The estimates describe AAPL on these sessions; the screen gives no basis for treating that distribution as market-neutral or representative of all trading days. An auction operating through an earnings release or a market-wide selloff can face a different book.

We partition each regular session into nonoverlapping intervals of 0.1, 1, 10 or 60 seconds. The replay retains intervals with at least one trade whose aggressor side can be determined. At each price the bid input combines resting bid depth at interval end, displayed bid quantity executed in the interval, and buyer-initiated trade quantity. The ask input is constructed symmetrically. This counts the resting and aggressive sides of an identified trade where appropriate. Sub-cent buy prices round down and sell prices round up on the one-cent grid. We exclude trade messages without an identified aggressor side. Orders canceled before the interval ends do not contribute resting depth.

We apply our rule to these reconstructed books. This is a counterfactual exercise on a continuous market: the participants submitted their orders under continuous execution. The replay therefore cannot measure the tie rate that would prevail if those participants moved to an FBA. Including resting interest matters even within this counterfactual. Executed quantity alone omits the losing interest that differentiates adjacent candidate prices.

At one second the mean across the four days was 3.63% of crossing intervals with several volume-maximizing ticks. The corresponding mean with multiple minimum-imbalance ticks was 0.32%. Across all four interval lengths, 546 of 156,950 crossing intervals had several minimum-imbalance ticks. In 462 of these, more than one tick met the at-price condition and the lowest-tick convention resolved the tie. The totals combine four partitions of the same sessions; they are counts of reconstructed batches, not independent auction observations. We retained 32 plateau books across the five replayed days, including one screened-out day, to exercise the prover. This selected set is appropriate for checking tie handling and inappropriate as a denominator for estimating prevalence.

## Generating order books

We fit each interval length separately. First, we locate the clearing tick in each crossing replay book and align the books at that tick. Within 100 ticks on either side we estimate, for bids and asks separately, the probability that a cell is occupied and the conditional volume quantiles of occupied cells. Farther from the centre, we pool resting bid interest below and ask interest above the centre into distance bands. These quantities describe the marginals. They do not identify the dependence between buy and sell volume at neighbouring prices.

We compare three ways to draw a book. The *independent* version samples each cell's occupancy and its positive quantity from its fitted marginal. The *occupancy* version copies the joint occupancy pattern within five ticks of the centre from a replayed batch, then draws the positive volumes separately. The *bootstrap* version draws one replayed batch and copies both sides' quantities at offsets $-10$ through $+10$ together. For that draw it also retains two totals: bid volume above the copied block and ask volume below it. It rescales the corresponding generated volumes to those totals. When rescaling leaves an integer remainder, the generator adds it to the largest cell in the block. If a block is empty and its target positive, the generator places the target at its boundary cell.

Those two totals have a direct interpretation in [Eq. (1)](02-auction-rule.md#eq-depth). Every bid above the central block contributes to demand throughout that block; every ask below it contributes to supply throughout. How far away each unit rests matters less for a local price tie than whether the units are present at all. The bootstrap preserves the observed local curves while allowing the rest of the grid to vary. Near-grid volumes that would lie beyond a small requested book are folded into the appropriate endpoint on the executable side. A seed, grid width, batch duration and calibration specify a reproducible generation run. The resulting books are tick aggregates, not synthetic streams of order arrivals.

<a id="table-2"></a>

**Table 2.** One-second replay and generated books. Percentages use crossing books as the denominator. The replay row pools the four analysis days. Each generator was run on 20,000 books of 601 ticks.

| Source | Volume plateau (%) | Tied minimum (%) | Median $V_{\max}$ |
| --- | --- | --- | --- |
| Nasdaq replay | 3.79 | 0.36 | 120 |
| Independent cells | 14.60 | 2.15 | 132 |
| Joint occupancy | 6.11 | 0.20 | 119 |
| Joint volumes and outer totals | 4.06 | 0.32 | 120 |

[Table 2](06-data-and-generator.md#table-2) shows why resampling volumes jointly matters. Independent draws give nearly four times the observed plateau frequency. Copying occupancy alone improves the volume-plateau rate but underrepresents the secondary ties. The bootstrap matches the pooled one-second rate more closely. This is a check on preservation of a calibration statistic: the generator samples from the same days used for comparison. It is not out-of-sample evidence for how order flow evolves. For the timing experiment we drew five bootstrap books at each of 100, 250, 500, 1,000 and 2,000 ticks. Market-derived benchmark books contain no IO orders; the worked example and implementation tests exercise those branches.

## A deployed periodic auction

Zhang and Ibikunle document Cboe Europe’s sub-second periodic auctions ([32](references.md#ref-32)). Cboe’s participant FAQ describes a randomized call phase of up to 100 ms and specifies the venue’s pricing and allocation rules ([11](references.md#ref-11)). We examine one instrument-day of the BXE feed described by Cboe DataShop ([10](references.md#ref-10)). The following counts are our measurements for Orange SA on 1 September 2020. For the 362 periodic summaries, flagged executions matched by timestamp sum to the reported auction sizes. In total, 400 executions traded 238,921 shares. The median auction size was 433.5 shares. An execution count does not identify how many traders or losing orders entered the auction.

For each summary we construct a 101-tick book on a EUR 0.001 grid centred on its reported price and put the reported volume on both sides at the centre. This construction deliberately yields one maximizing tick, zero imbalance and no IO. Our Rust and Noir implementations both returned tick 50 at the reported quantity for all 362 books. Mapping the tick back to euros recovers the published price by construction. Running the code on these inputs exercises quantities and external price labels observed in an operating FBA. The public feed cannot establish whether our rule would have selected the venue's price on the actual, undisclosed book.
