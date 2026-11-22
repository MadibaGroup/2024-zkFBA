#!/usr/bin/env python3
"""Convert an order-book CSV to the circuit's input file."""
import argparse
import csv
import secrets


def column(rows, name):
    if name not in rows[0]:
        return [0] * len(rows)
    return [int(float(row[name])) for row in rows]


def crosses(bids, asks):
    demand, supply = sum(bids), 0
    for bid, ask in zip(bids, asks):
        supply += ask
        if min(demand, supply) > 0:
            return True
        demand -= bid
    return False


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("book", help="CSV with Bids++ and Asks++ columns, one row per tick")
    parser.add_argument("out", help="Prover.toml to write")
    parser.add_argument("--salt", type=int, default=None)
    args = parser.parse_args()

    with open(args.book, newline="", encoding="utf-8-sig") as f:
        rows = list(csv.DictReader(f))
    book = {
        "bids": column(rows, "Bids++"),
        "asks": column(rows, "Asks++"),
        "io_bids": column(rows, "IO_Bids"),
        "io_asks": column(rows, "IO_Asks"),
    }
    salt = args.salt if args.salt is not None else secrets.randbelow(2**250)

    with open(args.out, "w", newline="\n") as f:
        f.write(f'salt = "{salt}"\n\n[book]\n')
        for name, values in book.items():
            cells = ", ".join(f'"{v}"' for v in values)
            f.write(f"{name} = [{cells}]\n")

    print(f"ticks={len(rows)}")
    print("circuit=" + ("clearing" if crosses(book["bids"], book["asks"]) else "no_cross"))


if __name__ == "__main__":
    main()