#!/usr/bin/env python3
"""Fetch real AAPL market data for the ZK-FBA real-data benchmark.

Adapted from ~/zk_fba_csv_full/GetData.py. Two changes from the original:

1. Narrowed the query window from the full 9:30-16:00 session to just
   9:30:00-9:35:00 ET (still the busiest few minutes of the day, right at
   the open) so the Databento historical pull stays small/cheap instead of
   downloading a full day and throwing most of it away client-side.
2. Pulls two schemas instead of one: mbp-10 (top-of-book quotes, for the
   "order book" dataset) and trades (individual executions, for the
   "n=100 trades" dataset).

Output: two raw CSVs in this directory,
  raw_quotes_AAPL.csv  -- mbp-10 snapshots, NY-local timestamps
  raw_trades_AAPL.csv  -- trade prints, NY-local timestamps
build_real_csv.py turns these into the 20-column order-book format the
Rust/Noir circuits expect.
"""
from datetime import datetime
from pathlib import Path

import databento as db
from zoneinfo import ZoneInfo

target_symbol = "AAPL"
target_date = "2026-06-01"

ny_tz = ZoneInfo("America/New_York")
start = datetime.strptime(f"{target_date} 09:30:00", "%Y-%m-%d %H:%M:%S").replace(tzinfo=ny_tz)
end = datetime.strptime(f"{target_date} 09:35:00", "%Y-%m-%d %H:%M:%S").replace(tzinfo=ny_tz)

HERE = Path(__file__).parent
client = db.Historical("db-6r3jSVELk5KhJWPCN333Th8npx8tv")


def fetch(schema: str, out_name: str, cap: int):
    data = client.timeseries.get_range(
        dataset="XNAS.ITCH",
        schema=schema,
        symbols=[target_symbol],
        start=start,
        end=end,
    )
    df = data.to_df()
    if not df.empty:
        df.index = df.index.tz_convert("America/New_York") if df.index.tz else df.index.tz_localize("UTC").tz_convert("America/New_York")
    df_capped = df.head(cap)
    out_path = HERE / out_name
    df_capped.to_csv(out_path)
    print(f"wrote {out_path}: {len(df_capped)} rows (of {len(df)} in window)")


if __name__ == "__main__":
    fetch("mbp-10", "raw_quotes_AAPL.csv", cap=200)   # need >=100 usable rows after filtering
    # The first ~150 trades of the session are dominated by the NASDAQ opening
    # cross (side='N', unclassified) -- pull more so build_real_csv.py has
    # enough classified (B/A) prints left after filtering those out.
    fetch("trades", "raw_trades_AAPL.csv", cap=1000)
