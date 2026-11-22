# Zeequent clearing in Noir

The standalone circuit verifies the clearing rule for a finished book. It takes four private arrays of unsigned 32-bit volumes and a private salt. The crossing circuit returns a Poseidon2 digest and eight receipt fields. The no-cross circuit returns the digest and checks that ordinary bids and asks have zero executable volume at every tick. Both digests cover all four arrays.

The circuit checks the same price rule as the Rust implementation. It has no running commitment log or per-order update receipts. A verifier must know which digest, circuit and price grid to associate with a batch.

## Prove a book

The evaluation used `nargo` 1.0.0 and Barretenberg (`bb`) 6.0.0 on Linux under WSL2. The scripts require Bash, Python 3, standard GNU utilities and `/usr/bin/time`. From this directory:

```sh
nargo test --package zeequent_lib
bash prove.sh /path/to/book.csv
```

Supply one CSV row per tick, in ascending price order. Use `Bids++` and `Asks++` for ordinary volume and optionally `IO_Bids` and `IO_Asks` for imbalance-only volume. Absent volume columns default to zero in the converter. Values should be non-negative integers that fit in 32 bits. The converter uses row position as the tick and ignores external price labels. Market orders use the appropriate grid endpoint.

In the release repository, `bash prove.sh ../examples/worked_book.csv` gives the receipt `(10, 2, 5, 5, 2, false, 10, 1)`. Its entries are maximum ordinary volume, left and right plateau endpoints, clearing tick, minimum imbalance, long-side flag, ordinary long volume at the selected tick, and eligible short-side IO volume. The book trades 11 units before rounding. For a balanced result the flag is false and both rationing fields are zero.

## Scripts

| Command | Operation |
|---|---|
| `book_to_toml.py BOOK.csv OUT.toml` | Writes the private witness input, draws a fresh salt and selects the crossing or no-cross circuit. `--salt N` fixes the salt for a reproducible example. |
| `prove.sh BOOK.csv [WORK_DIR]` | Recreates its work directory, copies the circuit, sets the tick count, compiles, generates the witness and key, proves and verifies. Use a disposable directory. `EXECUTE_ONLY=1` stops after witness generation. |
| `bench.sh OUT.csv RUNS THREADS BOOK.csv...` | Compiles each book size as needed and records witness, key, proof and verification timings. The script uses `$HOME/zq_noir_bench_work` as disposable working space. |

The witness input contains the plaintext arrays and salt. The digest is public. A secret, high-entropy salt is intended to prevent testing guessed books against the digest. The project-specific sponge construction has no separate security analysis.

## Circuit structure

`lib/src/lib.nr` defines the order book and clearing rule. The unconstrained `find_receipt` proposes the fields. `check_clearing` constrains every accepted result. `clearing/src/main.nr` and `no_cross/src/main.nr` select the public statement. The tick count is a compile-time parameter, so verifying keys depend on the circuit and size.

At 1,000 ticks the crossing circuit used 100,357 gates and produced a 14,656-byte proof. On one laptop thread the measured median times on calibrated books were 1.00 s for witness generation and 6.43 s for proving. These figures exclude the Rust protocol's order collection and update proof.
