# G Extended Benchmarks

All figures come from the setup of Section 5.2: one laptop on mains power, medians of 20 runs for the Rust implementation and of 10 runs per book for the Noir circuit, each in a single sitting. Times are in milliseconds. For the Rust implementation, a time covers a function call, and "verify" excludes decoding the proof, which is listed separately. For the Noir circuit, a time covers a whole command.

## G.1 The Clearing Proof Against Ticks and Volume Width

**Table 22.** Uniform books, 250 orders per batch, one thread.

| $N$ | $n$ | $k_{in}$ | Commitments | Proof (B) | Prove | Verify | Decode | Update-validity prove | No-cross prove | Batch check |
|---|---|---|---|---|---|---|---|---|---|---|
| 100 | 128 | 16 | 70 | 4,908 | 264 | 9.2 | 15.3 | 378 | 92 | 77 |
| 100 | 128 | 32 | 97 | 6,636 | 349 | 10.2 | 21.1 | 415 | 111 | 78 |
| 250 | 256 | 16 | 52 | 3,756 | 298 | 8.7 | 11.5 | 384 | 118 | 78 |
| 250 | 256 | 32 | 79 | 5,484 | 448 | 9.6 | 17.5 | 449 | 152 | 80 |
| 500 | 512 | 16 | 62 | 4,396 | 582 | 9.0 | 13.5 | 381 | 190 | 77 |
| 500 | 512 | 32 | 89 | 6,124 | 781 | 9.7 | 19.0 | 422 | 234 | 78 |
| 1,000 | 1,024 | 16 | 62 | 4,396 | 998 | 9.3 | 14.0 | 382 | 320 | 78 |
| 1,000 | 1,024 | 32 | 89 | 6,124 | 1,366 | 10.0 | 19.3 | 424 | 402 | 79 |
| 2,000 | 2,048 | 16 | 62 | 4,396 | 1,803 | 9.2 | 14.1 | 625 | 568 | 80 |
| 2,000 | 2,048 | 32 | 89 | 6,124 | 2,462 | 10.0 | 19.6 | 693 | 715 | 81 |
| 4,000 | 4,096 | 16 | 62 | 4,396 | 3,330 | 9.1 | 13.7 | 1,102 | 1,047 | 85 |
| 4,000 | 4,096 | 32 | 89 | 6,124 | 4,506 | 10.1 | 19.2 | 1,218 | 1,303 | 87 |
| 8,000 | 8,192 | 16 | 62 | 4,396 | 6,367 | 8.9 | 13.6 | 1,950 | 1,950 | 95 |
| 8,000 | 8,192 | 32 | 89 | 6,124 | 8,723 | 9.8 | 18.9 | 2,196 | 2,484 | 95 |

**Table 23.** The same runs on eight threads.

| $N$ | $k_{in}$ | Prove | Verify | Decode | Update-validity prove | No-cross prove | Batch check |
|---|---|---|---|---|---|---|---|
| 100 | 16 | 98 | 6.4 | 3.6 | 122 | 41 | 52 |
| 100 | 32 | 117 | 6.7 | 5.0 | 131 | 50 | 53 |
| 250 | 16 | 115 | 6.2 | 2.9 | 113 | 56 | 51 |
| 250 | 32 | 154 | 6.4 | 3.9 | 122 | 72 | 51 |
| 500 | 16 | 192 | 6.4 | 3.3 | 115 | 84 | 51 |
| 500 | 32 | 236 | 6.7 | 4.6 | 123 | 102 | 52 |
| 1,000 | 16 | 291 | 6.4 | 3.6 | 114 | 114 | 52 |
| 1,000 | 32 | 376 | 6.7 | 4.8 | 122 | 128 | 52 |
| 2,000 | 16 | 484 | 6.5 | 3.7 | 174 | 178 | 54 |
| 2,000 | 32 | 615 | 6.8 | 4.7 | 190 | 215 | 54 |
| 4,000 | 16 | 865 | 6.6 | 3.6 | 314 | 314 | 60 |
| 4,000 | 32 | 1,081 | 6.6 | 4.6 | 314 | 354 | 60 |
| 8,000 | 16 | 1,590 | 6.7 | 3.6 | 532 | 533 | 70 |
| 8,000 | 32 | 2,026 | 6.8 | 4.6 | 549 | 618 | 69 |

A domain of 256 has fewer columns than the domain of 512 because its accumulated width, 24 or 40 bits, is a multiple of 8 and needs no scaled top limb; the domain of 128 has more because its limbs are 7 bits wide. With 250 orders the update-validity proof runs on a domain of $\max(n, 1024)$ rows, so its cost is flat up to $n = 1024$ and then follows $n$. Eight threads on four physical cores speed the clearing proof up by 3.6 at $n = 1024$ and by 4.3 at $n = 8192$.

## G.2 Spread

**Table 24.** $N = 1000$, 32-bit volumes, 250 orders: median and interquartile range over 20 runs.

| Metric | One thread | Eight threads |
|---|---|---|
| Clearing proof, prove | 1,366 [1,356–1,377] | 376 [359–384] |
| Clearing proof, verify | 10.0 [9.5–10.3] | 6.7 [6.6–6.8] |
| Clearing proof, decode | 19.3 [18.8–19.6] | 4.8 [4.5–5.2] |
| Update-validity proof, prove | 424 [419–428] | 122 [121–123] |
| Update-validity proof, verify | 43.0 [42.4–44.0] | 20.5 [20.0–20.8] |
| Batch check | 79.3 [77.5–80.3] | 52.0 [51.5–52.3] |

## G.3 Calibrated and Real Books

**Table 25.** The clearing proof on the calibrated books (five per size) and on the 32 replayed books, 32-bit volumes. Brackets give the range of the per-book medians.

| Books | $N$ | $n$ | Prove, one thread | Prove, eight threads | Verify + decode, one thread |
|---|---|---|---|---|---|
| Calibrated | 100 | 128 | 343 [341–345] | 117 [111–130] | 30.6 |
| Calibrated | 250 | 256 | 414 [408–420] | 150 [134–156] | 26.3 |
| Calibrated | 500 | 512 | 769 [765–771] | 232 [228–235] | 28.6 |
| Calibrated | 1,000 | 1,024 | 1,343 [1,339–1,351] | 373 [363–382] | 28.8 |
| Calibrated | 2,000 | 2,048 | 2,437 [2,433–2,447] | 626 [608–633] | 28.6 |
| Real | 601–662 | 1,024 | 1,348 [1,334–1,363] | 367 [353–381] | 29.5 |

At $n=1024$, measured clearing proof generation times differ by less than 3% between the uniform, calibrated, and replayed books.

## G.4 Operations Outside the Proofs

**Table 26.** Per-order and per-batch operations, $N = 1000$, 32-bit volumes.

| Operation | One thread | Eight threads | Published size |
|---|---|---|---|
| Record an order (exchange) | 0.98 | 0.99 | 416 B on the board |
| Check a receipt (trader) | 0.24 | 0.24 | 192 B receipt |
| Open a batch | 13.3 | 9.6 | 1,576 B |
| Check an opening | 19.1 | 18.5 | |
| Run the clearing rule | 0.07 | 0.08 | |

One core records about 1,000 orders per second.

## G.5 The Noir Circuit

**Table 27.** The circuit on the calibrated books (median over five books and ten runs each, interquartile range in brackets) and on the 32 replayed books. The proof is 14,656 bytes and verification takes 27 to 30 ms in every row.

| Books | $N$ | Gates | Threads | Witness | Key | Prove | Prover memory |
|---|---|---|---|---|---|---|---|
| Calibrated | 100 | 12,757 | 1 | 720 | 344 | 1,052 [1,044–1,064] | 36 MB |
| | | | 8 | 702 | 191 | 525 [518–540] | 45 MB |
| Calibrated | 250 | 27,357 | 1 | 764 | 683 | 1,994 [1,981–2,012] | 57 MB |
| | | | 8 | 739 | 288 | 764 [754–777] | 68 MB |
| Calibrated | 500 | 51,666 | 1 | 844 | 1,181 | 3,541 [3,532–3,567] | 96 MB |
| | | | 8 | 820 | 456 | 1,159 [1,145–1,173] | 106 MB |
| Calibrated | 1,000 | 100,357 | 1 | 1,001 | 2,213 | 6,427 [6,396–6,480] | 157 MB |
| | | | 8 | 989 | 763 | 1,898 [1,879–1,927] | 180 MB |
| Calibrated | 2,000 | 197,666 | 1 | 1,337 | 4,260 | 12,478 [12,400–12,716] | 305 MB |
| | | | 8 | 1,315 | 1,470 | 3,359 [3,333–3,420] | 326 MB |
| Real | 601–662 | 61,521–67,434 | 1 | 877 | 1,439 | 4,195 [4,115–4,300] | 116 MB |
| | | | 8 | 858 | 537 | 1,316 [1,297–1,352] | 129 MB |

The key column gives the cost of generating the verification key once per book size. Witness generation does not use the additional threads.

**Table 28.** Ratio of the circuit's proving time, with witness generation, to ours, on the same books.

| $N$ | One thread | Eight threads |
|---|---|---|
| 100 | 5.2 | 10.5 |
| 250 | 6.7 | 10.0 |
| 500 | 5.7 | 8.5 |
| 1,000 | 5.5 | 7.7 |
| 2,000 | 5.7 | 7.5 |
| Real, 601–662 | 3.8 | 5.9 |

These ratios include process startup for Noir. Noir runs under WSL2 and uses a different transcript hash. We did not isolate the cost of each difference.
