#!/usr/bin/env python3
"""Fetch Nasdaq TotalView-ITCH data from Databento for the Zeequent study."""
import argparse
import csv
import json
import os
import sys
import warnings
from datetime import date, datetime, timedelta, timezone
from pathlib import Path

DATASET = "XNAS.ITCH"          # Nasdaq TotalView-ITCH
HERE = Path(__file__).parent
OUT = HERE.parent / "realdata" / "databento"


def sampled_days(count):
    """The first `count` days of the random order that pass the screen."""
    with open(HERE / "sample_days.csv", newline="") as f:
        rows = [row for row in csv.DictReader(f) if row.get("screen", "pass") == "pass"]
    return [date.fromisoformat(row["date"]) for row in rows[:count]]


def main():
    parser = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    parser.add_argument("--symbols", required=True, help="comma-separated, e.g. AAPL,MSFT")
    parser.add_argument("--days", type=int, required=True,
                        help="use the first N days of sample_days.csv that pass the screen")
    parser.add_argument("--schemas", default="mbo,trades,imbalance")
    parser.add_argument("--execute", action="store_true", help="download (spends money)")
    parser.add_argument("--max-usd", type=float, default=None, help="budget; required with --execute")
    args = parser.parse_args()

    key = os.environ.get("DATABENTO_API_KEY")
    if not key:
        sys.exit("DATABENTO_API_KEY is not set")
    if args.execute and args.max_usd is None:
        sys.exit("--execute requires --max-usd")

    import databento as db   # pip install databento
    warnings.simplefilter("ignore")
    client = db.Historical(key)

    OUT.mkdir(parents=True, exist_ok=True)
    manifest_path = OUT / "manifest.json"
    manifest = json.loads(manifest_path.read_text())["requests"] if manifest_path.exists() else []
    known = {entry["file"] for entry in manifest if entry.get("downloaded")}

    quoted_total = spent = 0.0
    for day in sampled_days(args.days):
        start = datetime(day.year, day.month, day.day, tzinfo=timezone.utc)
        end = start + timedelta(days=1)
        for symbol in args.symbols.split(","):
            for schema in args.schemas.split(","):
                name = f"{symbol}_{day.isoformat()}_{schema}.dbn.zst"
                path = OUT / name
                if name in known and path.exists():
                    print(f"{name}: already downloaded, skipped")
                    continue
                common = dict(dataset=DATASET, schema=schema, symbols=[symbol], start=start, end=end)
                cost = float(client.metadata.get_cost(**common))
                quoted_total += cost
                print(f"{name}: quoted ${cost:.4f}", flush=True)
                if not args.execute:
                    continue
                if spent + cost > args.max_usd:
                    print(f"  stopping: this request would exceed the ${args.max_usd:.2f} budget")
                    break
                records = client.metadata.get_record_count(**common)
                # Download to a temporary name and rename when complete.
                part = path.with_name(path.name + ".part")
                if part.exists():
                    part.unlink()
                print(f"  downloading {records:,} records (this can take many minutes)...", flush=True)
                store = client.timeseries.get_range(path=str(part), **common)
                del store
                part.replace(path)
                spent += cost
                manifest.append({
                    "file": name, "dataset": DATASET, "symbol": symbol, "schema": schema,
                    "start": start.isoformat(), "end": end.isoformat(),
                    "quoted_cost_usd": cost, "records": int(records),
                    "bytes": path.stat().st_size, "downloaded": True,
                    "downloaded_at": datetime.now(timezone.utc).isoformat(),
                    "client": f"databento-python {db.__version__}",
                })
                manifest_path.write_text(json.dumps({"requests": manifest}, indent=2))
            else:
                continue
            break
        else:
            continue
        break

    print(f"\nquoted total for new requests: ${quoted_total:.4f}")
    if args.execute:
        print(f"spent this run (sum of quotes for downloaded files): ${spent:.4f}")
        print(f"manifest: {manifest_path}")
    else:
        print("dry run: nothing was downloaded. Add --execute --max-usd N to download.")


if __name__ == "__main__":
    main()
