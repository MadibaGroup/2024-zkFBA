# Clearing-proof benchmarks

Both implementations use the same integer clearing rule. Rust binds the receipt to four previously committed book columns. Noir hashes the finished book and a salt inside the circuit. Only Rust maintains the log and verifies its updates. We compare the clearing proofs on the same 25 calibrated books and 32 Nasdaq replays.

Measurements used one AMD Ryzen 3 7320U laptop (four physical cores, eight hardware threads). Rust ran natively on Windows, with 20 repetitions per configuration. Noir ran under WSL2, with 10. Rust times cover function calls. Noir times include process startup. Compilation and key generation are excluded from the proving comparison. The ratios include witness generation where indicated.

## Calibrated synthetic books

Calibrated books, 32-bit volumes, five books per size, medians. "Noir total" is witness generation plus proving.

One thread:

| Ticks | Custom: prove | Noir: prove | Noir: witness | Noir total | Prove ratio | Total ratio |
|---|---|---|---|---|---|---|
| 100 | 343 ms | 1,052 ms | 720 ms | 1,772 ms | 3.1 | 5.2 |
| 250 | 414 ms | 1,994 ms | 764 ms | 2,758 ms | 4.8 | 6.7 |
| 500 | 769 ms | 3,541 ms | 844 ms | 4,385 ms | 4.6 | 5.7 |
| 1,000 | 1,343 ms | 6,427 ms | 1,001 ms | 7,428 ms | 4.8 | 5.5 |
| 2,000 | 2,437 ms | 12,478 ms | 1,337 ms | 13,815 ms | 5.1 | 5.7 |

Eight threads:

| Ticks | Custom: prove | Noir: prove | Noir: witness | Noir total | Prove ratio | Total ratio |
|---|---|---|---|---|---|---|
| 100 | 117 ms | 525 ms | 702 ms | 1,227 ms | 4.5 | 10.5 |
| 250 | 150 ms | 764 ms | 739 ms | 1,503 ms | 5.1 | 10.0 |
| 500 | 232 ms | 1,159 ms | 820 ms | 1,979 ms | 5.0 | 8.5 |
| 1,000 | 373 ms | 1,898 ms | 989 ms | 2,887 ms | 5.1 | 7.7 |
| 2,000 | 626 ms | 3,359 ms | 1,315 ms | 4,674 ms | 5.4 | 7.5 |

Sizes and verification at 1,000 ticks:

| | Custom | Noir | Ratio |
|---|---|---|---|
| Proof | 6,124 B (4,396 B with 16-bit volumes) | 14,656 B | 2.4 |
| Verification, 1 thread | 28.8 ms (9.7 ms checking, 19.1 ms decoding) | 27 ms | 0.9 |
| Verification, 8 threads | 11.5 ms (6.8 ms checking, 4.7 ms decoding) | 29 ms | 2.5 |
| Peak prover process memory (separate measurements) | approximately 20 MB | 180 MB (8 threads), 157 MB (1 thread) | |

## Replayed Nasdaq books

The 32 books span 601–662 ticks. Each uses a 1,024-row Rust domain.

| | Custom | Noir | Ratio |
|---|---|---|---|
| Prove, 1 thread | 1,348 ms | 4,195 ms (+ 877 ms witness) | 3.1 (3.8 with witness) |
| Prove, 8 threads | 367 ms | 1,316 ms (+ 858 ms witness) | 3.6 (5.9 with witness) |
| Proof | 6,124 B | 14,656 B | 2.4 |
| Size of the statement | 1,024 rows | 61,521 to 67,434 gates | |

The comparison does not isolate compiler, transcript, operating-system or process-startup costs. Receipt agreement was checked on the six fields saved by the Rust CSV benchmark. Independent calculation reproduces the two additional rationing fields reported by Noir. The separate Cboe exercise uses constructed, balanced books and does not measure tie frequency.
