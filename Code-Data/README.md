# Zeequent

Zeequent lets a public verifier check the clearing tick of a frequent batch auction while the exchange keeps the orders private. The exchange receives orders in plaintext. A batch starts with an empty book and accepts additions until it closes.

The clearing rule first maximizes executable **ordinary** volume, then minimizes absolute imbalance. Of the remaining ticks, it chooses the lowest at which long-side volume at that tick covers the imbalance. Eligible short-side imbalance-only (IO) orders can then trade. All executions use the same price. The instrument, batch deadline, and mapping from tick to quoted price are venue parameters supplied outside the proof.

## Run the example

From the repository root, run these commands with Cargo installed:

```sh
cd zeequent-rs
cargo test --locked
cargo run --release -p zeequent-cli -- demo
```

The demo exercises both a crossing batch and a no-cross batch. The hand-worked CSV includes one sell IO at tick 4. With that order, ordinary volume is 10 at clearing tick 5 and total executable volume is 11. The Noir CSV converter reads the IO columns. To reproduce the full example in Rust, submit the orders through `order` or construct the four book columns in the library. The Rust `data` command reads only ordinary bid and ask volumes.

## What the verifier checks

| Component | Public statement |
|---|---|
| Rust opening and log | Four commitments start empty. Each subsequent log entry identifies the column and its new commitment. The verifier recomputes the hash chain. The participant compares the returned digest with the one agreed for the batch. |
| Trader's private update receipt | The encoded column, tick and quantity match a particular transition in that history. A trader compares the fields with their request and checks membership using `check-receipt --dir`. |
| Rust update-validity proof | Each logged difference represents a bounded non-negative contribution at one domain position, including zero. The honest writer appends four entries per order. The proof checks individual entries. |
| Rust closing proof | A crossing receipt follows from all four final columns. A no-cross proof establishes zero ordinary executable volume from the final bid and ask columns. |
| Noir circuit | A closing result follows from a finished book identified by a salted digest. Both circuits hash all four columns. They have no order log. |

The log, private receipts and proofs are additions made by Zeequent to support verification. A common published digest is essential for different participants to check the same history. The exchange controls admission. Neither implementation proves arrival times, trader identity, integer allocations or settlement. The opening, log and proofs share commitments, and privacy of their complete joint transcript remains an open proof obligation.

## Code, data and measurements

- [Rust implementation](zeequent-rs/README.md) and [independent Noir circuit](zeequent-noir-standalone/README.md).
- [Data and reproduction instructions](data/README.md), [day selection](data/SAMPLING_PLAN.md), and the [eight-tick example](examples/worked_book.csv).
- [Clearing constraints](docs/fixedConstraints.md), [clearing-rule proof](docs/CORRECTNESS_PROOF.md), and [security of the committed book](docs/SECURITY_BOOK.md).
- [Rust implementation notes](docs/IMPLEMENTATION.md), [Noir implementation notes](docs/IMPLEMENTATION_NOIR.md), [comparison](docs/COMPARISON.md), and [recorded benchmarks](results/BENCHMARKS.md).

The release includes scripts, aggregate replay statistics and uniform-book timing samples. The study used 32 reconstructed Nasdaq books, 25 calibrated synthetic books, and 362 constructed books based on observed Cboe Europe periodic-auction outcomes. Raw feeds, calibrated volume arrays and per-auction market records are external inputs. At 1,000 ticks on one laptop thread the median Rust clearing-proof time was 1.34 s on calibrated books. The 6,124-byte proof took 29 ms to decode and verify. These timings concern the clearing proof alone.
