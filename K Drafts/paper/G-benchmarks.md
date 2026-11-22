# Appendix G. Benchmark measurements

## Timing conventions

We report three distinct experiments. Calibrated books compare our two clearing implementations across grid sizes. Selected replays exercise the same price rule on books with observed price ties. Uniform books vary the grid, entry width or order count under controlled inputs. The measurements below use the machine and software environments in [Section 7](07-implementations-and-evaluation.md).

**Table G.1.** Clearing generation in seconds on one and eight threads, five calibrated books at each size. For our Rust implementation each cell is the median of the five per-book proving medians, with 20 repetitions per book. For our Noir implementation each cell adds the median witness-command time to the median proof-command time across 50 runs (ten per book). Verification-key generation is timed separately.

| Ticks | Padded rows (Rust) | Our Rust implementation, 1 thread | Our Rust implementation, 8 threads | Our Noir implementation, 1 thread | Our Noir implementation, 8 threads |
| --- | --- | --- | --- | --- | --- |
| 100 | 128 | 0.343 | 0.117 | 1.772 | 1.227 |
| 250 | 256 | 0.413 | 0.150 | 2.758 | 1.502 |
| 500 | 512 | 0.768 | 0.232 | 4.385 | 1.979 |
| 1,000 | 1,024 | 1.343 | 0.373 | 7.428 | 2.887 |
| 2,000 | 2,048 | 2.437 | 0.626 | 13.815 | 4.674 |

At 1,000 ticks on one thread the witness median was 1,000.5 ms and the proof-command median was 6,427 ms. Their sum was 7,427.5 ms. Summing the two durations within each run first gives a median of 7,442.5 ms. The former convention reproduces the reported 7.43 s. The separate verification-key command had a median of 2,213 ms. Process startup is included in the command times. Our Rust implementation times native function calls.

The 32 selected replays span 601–662 ticks. All use 1,024 rows in our Rust implementation, whose one-thread median of per-book proving medians was 1.348 s. The 362 Cboe-derived books were measured separately, with five Rust runs per book and thread configuration and one eight-thread Noir run per book. Their balanced, single-price construction makes them a different workload from the plateau-enriched replays.

## Variation across repeated measurements

**Table G.2.** Lower quartile / median / upper quartile in milliseconds for our Rust implementation. The uniform-book configuration has 1,000 ticks, 32-bit entries and 250 orders, with 20 runs per thread count.

| Operation | One thread (ms) | Eight threads (ms) |
| --- | --- | --- |
| Crossing proof generation | 1,355.5 / 1,365.9 / 1,376.8 | 358.6 / 375.6 / 384.3 |
| Crossing proof decoding | 18.8 / 19.3 / 19.6 | 4.5 / 4.8 / 5.2 |
| Crossing proof verification after decoding | 9.5 / 10.0 / 10.3 | 6.6 / 6.7 / 6.8 |
| Update proof generation | 419.3 / 423.8 / 427.8 | 120.6 / 121.5 / 122.7 |
| Update proof verification after decoding | 42.4 / 43.0 / 44.0 | 20.0 / 20.5 / 20.8 |
| Full check on decoded artifacts | 77.5 / 79.3 / 80.3 | 51.5 / 52.0 / 52.3 |

The crossing-verification row assumes decoded points, whereas the 29 ms crossing-verification figure in the main text includes decoding and uses calibrated books. The full batch-check row includes the empty opening, log replay and both proof checks on decoded inputs. It excludes network distribution and agreement on a common final digest.

## Grid size and entry width

**Table G.3.** Crossing-proof size and median proving time for our Rust implementation. These uniform-book runs use 250 orders, one thread and the stated per-cell quantity width.

| Ticks | 16-bit proof (B) | 32-bit proof (B) | 16-bit proving (s) | 32-bit proving (s) |
| --- | --- | --- | --- | --- |
| 100 | 4,908 | 6,636 | 0.264 | 0.349 |
| 250 | 3,756 | 5,484 | 0.298 | 0.448 |
| 500 | 4,396 | 6,124 | 0.582 | 0.781 |
| 1,000 | 4,396 | 6,124 | 0.998 | 1.366 |
| 2,000 | 4,396 | 6,124 | 1.803 | 2.462 |
| 4,000 | 4,396 | 6,124 | 3.330 | 4.506 |
| 8,000 | 4,396 | 6,124 | 6.367 | 8.723 |

The size decreases from 100 to 250 ticks because the change in domain also changes limb width and the number of lookup expressions. At 250 ticks the domain has 256 rows, eight-bit limbs and accumulated widths of 24 or 40 bits, both multiples of eight. Above 500 ticks the rows shown keep the same column count within each width even as the number of domain evaluations increases. Proof size is constant over those configurations with the same layout; proving time grows.

For the 1,000-tick, 32-bit no-cross configuration the separate outcome proof has 20 column-and-quotient commitments, occupies 1,612 bytes and takes a median of 402 ms to generate on one thread. IO deposits can be present in a no-cross batch; the update proof still processes them. [Table 4](07-implementations-and-evaluation.md#table-4) reports the cost of increasing the log length at fixed price resolution. Its sum of component medians describes after-close proving work and does not measure sustained exchange throughput.
