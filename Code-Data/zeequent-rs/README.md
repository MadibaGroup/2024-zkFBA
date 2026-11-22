# Zeequent in Rust

The Rust workspace implements the auction rule, an updatable commitment to the four book columns, and separate proofs for the logged updates and the closing outcome. The clearing receipt is public. A trader's update receipt is private and refers to a particular transition in one column.

## Build and test

The benchmarks used rustc 1.100.0, arkworks 0.4 and the release profile. From this directory:

```sh
cargo test --locked
cargo build --locked --release -p zeequent-cli --features parallel
./target/release/zeequent demo
```

On Windows the executable is `target/release/zeequent.exe`. The commands below use `zeequent` for that executable. The `parallel` feature enables Rayon. Set `RAYON_NUM_THREADS` to choose the thread count.

## One batch, step by step

A batch lives in a directory. Each command reads and writes files there. The example uses a local test setup (`--insecure`); deployment requires an independently trusted reference string.

```
# everyone, once: keys for 100 price ticks and 16-bit volumes
zeequent setup --ticks 100 --kin 16 --insecure --out setup.bin

# exchange: open an empty book
zeequent open --setup setup.bin --dir batch

# anyone: check that the book starts empty
zeequent check-opening --setup setup.bin --dir batch

# exchange: record orders as they arrive
zeequent order --setup setup.bin --dir batch --side buy  --tick 52 --qty 300
zeequent order --setup setup.bin --dir batch --side sell --tick 50 --qty 200
zeequent order --setup setup.bin --dir batch --side sell --type io --tick 49 --qty 40

# trader: check the receipt of the first order against the public log
zeequent check-receipt --setup setup.bin --receipt batch/receipts/0001.bin --dir batch

# exchange: clear the batch and write the outcome and both proofs
zeequent close --setup setup.bin --dir batch

# anyone: check the whole batch
zeequent verify --setup setup.bin --dir batch

# trader: compute a fill from the public outcome
zeequent fill --setup setup.bin --dir batch --side buy --tick 52 --qty 300
```

The example clears at tick 52. Ordinary volume is 200, the initial imbalance is 100, and 40 units of eligible sell IO trade. The 300-unit buy order receives 240 units. The verifier reports 12 logged steps.

## Interpreting a successful check

At opening the exchange proves that all four columns are empty. For each accepted order it changes one column by the encoded quantity and reblinds the other three. The honest writer posts all four new commitments in fixed order. The log records the column, resulting commitment and digest. Live publication exposes its cadence. It authenticates no arrival timestamp.

`check-receipt` verifies the commitment equation. With `--dir` it also checks the chain and looks up the transition. Compare the receipt fields with the submitted order and compare the history with the common final digest. Two receipts for the same transition identify one contribution. A market buy or sell has the same encoding as an ordinary limit order at its respective grid endpoint.

`verify` checks the empty opening, the log, the update proof and the outcome proof. The update proof admits a zero step and checks positions on the padded domain. It does not establish ownership, order grouping or complete admission. At a cross, all four final columns are bound to the eight-field receipt. A no-cross proof establishes zero ordinary executable volume and uses only the two ordinary final commitments. The verifier prints the digest it checked. Distribution of a common head, instrument, grid and close time is external.

The proof fixes quantities and ratios before rounding. `fill` truncates each fractional entitlement separately. Assigning residual whole units and settling executions require a venue rule.

## Command reference

```
zeequent demo
zeequent setup --ticks N --kin K [--max-orders M] (--ptau FILE | --insecure) --out SETUP
zeequent open --setup SETUP --dir DIR
zeequent check-opening --setup SETUP --dir DIR
zeequent order --setup SETUP --dir DIR --side buy|sell [--type limit|market|io] [--tick T] --qty Q
zeequent check-receipt --setup SETUP --receipt FILE [--dir DIR]
zeequent close --setup SETUP --dir DIR
zeequent verify --setup SETUP --dir DIR
zeequent fill --setup SETUP --dir DIR --side buy|sell [--type limit|market|io] [--tick T] --qty Q

zeequent bench [--ticks 100,1000] [--kin 16,32] [--orders 250] [--runs 5] [--csv FILE] [--raw FILE]
zeequent data --files A.csv,B.csv [--kin 16] [--runs 3] [--csv FILE]
zeequent ties [--trades RAW.csv] [--intervals 1,5,10] [--books A.csv,B.csv] [--trials 2000] [--csv FILE]
zeequent export-noir --file BOOK.csv --out DIR
```

## Batch files

| File | Written by | Public | Content |
|---|---|---|---|
| setup file | `setup` | Yes | Keys |
| `state.bin` | `open`, `order` | **No** | The plaintext book and every blinding value |
| `opening.bin` | `open` | Yes | Four commitments with proofs that the book starts empty (1,576 bytes) |
| `board.bin` | `open`, `order` | Yes | The commitment log: four hash-chained entries per order (416 bytes per order) |
| `book.bin` | `open`, `order` | Yes | The four current commitments (264 bytes) |
| `receipts/NNNN.bin` | `order` | To the trader | Column, tick, quantity, blinding, commitment before and after (192 bytes); numbered from 0001 |
| `outcome.txt` | `close` | Yes | The clearing receipt as `key=value` lines, or `outcome=no-cross` |
| `proof.bin` | `close` | Yes | The clearing or no-cross proof |
| `updates.bin` | `close` | Yes | The update-validity proof |

Keep `state.bin` and individual update receipts private. The log and closing artifacts are public. Running `close` again on the same honest private state regenerates the same proof bytes.

## CSV inputs

The `data` subcommand expects `Bids++` and `Asks++` on rows in ascending price order. `Price` supplies external labels and defaults to the row index when absent. It sets IO volumes to zero even if IO columns are present. It also checks optional reference columns for depth, minimum, maximum ordinary volume and clearing price. Proofs bind the tick index. Converting it to an external price uses the file's labels.

`export-noir` reads optional `IO_Bids` and `IO_Asks` and writes the legacy book and receipt TOML files. The standalone Noir circuit uses its own CSV converter and a private salt.

## Library entry points

| Function | Module | Use |
|---|---|---|
| `clear` | `clearing` | Runs the clearing rule on a plaintext book; returns the receipt and the witness |
| `BookState::open`, `apply` | `book` | The exchange's state: opens a book, records an order, returns the update receipt |
| `verify_book_opening`, `verify_update` | `book` | Checks of the opening and of a receipt |
| `Board` | `book` | The public log: `append_order`, `verify`, `contains`, `deltas` |
| `prove`, `verify` | `proof` | The clearing proof |
| `prove_no_cross`, `verify_no_cross` | `proof` | The no-cross proof |
| `prove_updates`, `verify_updates` | `updates` | The update-validity proof |
| `verify_batch` | `batch` | Checks the supplied batch and returns its log digest |
| `fill` | `clearing` | Integer entitlement from the order and receipt |

Use `setup --insecure` only for local experiments. `setup --ptau` imports ceremony parameters. Consistency checks cannot establish that the setup secrets were destroyed. Use one distinct closing transcript per opening. Individual blinded log differences hide their encoded values, while the public outcome can reveal information about a thin book. A full simulation argument for the shared log, openings and proofs remains open.

In the 1,000-tick, 32-bit benchmark the clearing proof occupies 6,124 bytes and the update proof 1,700 bytes. The 0.98 ms order measurement times the in-memory commitment update. The 0.24 ms receipt measurement times the equation alone. The batch-check timer starts with decoded artifacts.
