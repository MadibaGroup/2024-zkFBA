# AAPL sampling frame and screening rules

The initial design (17 November 2026) sampled trading days before measuring tie frequencies in reconstructed batches. From 219 sessions between 2 January and 13 November 2026, calendar exclusions left 119 dates. `sample_days.py` shuffles those dates with seed `20261117`. It records the draw order, pass/fail decision and reasons in `sample_days.csv`.

The screen was extended on 18 November after the first replay results were available. It uses event dates, returns, trading ranges and volume. The extension did not change the four days included in the analysis. This timing matters when interpreting the design as a sample of ordinary sessions.

**(a) Scheduled events, by date**

| Factor | Rule | Source of dates |
|---|---|---|
| Producer Price Index, Employment Cost Index, JOLTS | the release day | BLS 2026 release calendar |
| GDP (advance, second, third), Personal Income and Outlays (PCE) | the release day | BEA release schedule |
| FOMC minutes | the release day | Federal Reserve FOMC calendar |
| Jackson Hole symposium | 2026-08-27 and 2026-08-28 | Kansas City Fed |
| Month end | last trading day of each month | computed |
| AAPL earnings | announcement day and next trading day | public earnings calendars |
| Nvidia earnings | announcement day and next trading day | public earnings calendars |
| AAPL ex-dividend | 2026-02-09, 05-11, 08-10, 11-12 | public dividend calendars |
| Apple WWDC keynote; Apple product event | 2026-06-08; 2026-09-09 | press reports |

(Removed earlier, before the draw: FOMC decisions, CPI, Employment Situation, each with neighbours; monthly options expirations; Russell reconstitution; the midterm election with neighbours; early closes; days next to holidays.)

**(b) The stock and the market against their own recent history**

- |close-to-close return| of AAPL or of QQQ above 2 standard deviations of the previous 60 daily returns.
- AAPL volume above 2x or below 0.5x its median over the previous 20 days.
- High-low range of AAPL or of QQQ above 2x its median over the previous 20 days.

**(c) Shocks elsewhere, detected by what moved**

- Any of NVDA, MSFT, GOOGL, AMZN, META, TSLA with |return| above 3 standard deviations, or volume above 3x its 20-day median.
- Any of SMH (semiconductors), USO (crude oil), TLT (long Treasury bonds), GLD (gold) with |return| above 3 standard deviations.

## Analysis sample

Of the 119 dates, 62 passed and 57 failed the final screen. The analysis uses the first four passing draws. The separate 27 August replay failed the screen (the day after Nvidia earnings and the Jackson Hole symposium). It is retained for comparison and excluded from the four-day means and default calibration.

For each interval length, compute the tie rate among crossing batches for each day. The reported means weight the four days equally. The descriptive 95% t intervals use the four daily rates. Pooled rates weight days by their number of crossing batches and therefore can differ. The selection thresholds limit inference to the sampled stock and screened days.

The twelve-symbol daily-bar file used for screening is external to this release. The code uses the available calendar checks alone if that file is missing. The checked-in day list preserves the full study screen.
