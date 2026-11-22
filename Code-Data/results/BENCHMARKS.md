# Rust benchmark measurements

The uniform scaling and order-count runs were recorded on 19 November 2026 on an AMD Ryzen 3 7320U laptop (4 cores, 8 hardware threads, Windows 11, mains power). The release build used rustc 1.100.0, Rayon and arkworks 0.4. Each Rust timing is the median of 20 repetitions for its configuration unless otherwise specified. Setup used locally generated test parameters. The batch timer runs `verify_batch` with decoded artifacts in memory. It excludes setup checks, file I/O and distribution of the final digest.

## Reproduce the uniform measurements

Set `RAYON_NUM_THREADS` to 1 or 8 in the shell, then run the parallel release binary. Use a separate output path for each thread count.

```sh
zeequent bench --ticks 100,250,500,1000,2000,4000,8000 --kin 16,32 --orders 250 --runs 20 --csv scaling.csv --raw scaling_raw.csv
zeequent bench --ticks 1000 --kin 32 --orders 1000 --runs 20 --csv orders1000.csv --raw orders1000_raw.csv
zeequent bench --ticks 1000 --kin 32 --orders 2200 --runs 20 --csv orders2200.csv --raw orders2200_raw.csv
```

The `final/` directory contains the raw and summarized uniform timing CSVs, original console tables and run log. Per-market-book CSVs and the raw Noir runs are outside this release. The [data guide](../data/README.md) describes their inputs.

## 3. Synthetic books

### 3.1 Uniform books

N = 1,000 ticks (n = 1,024), 32-bit volumes (k = 42), 250 orders in the batch. Median [p25-p75].

| Metric | 1 thread | 8 threads |
|---|---|---|
| Clearing proof: generate | 1,366 ms [1,356-1,377] | 376 ms [359-384] |
| Clearing proof: verify | 10.0 ms [9.5-10.3] | 6.7 ms [6.6-6.8] |
| Clearing proof: decode | 19.3 ms [18.8-19.6] | 4.8 ms [4.5-5.2] |
| Update-validity proof: generate | 424 ms [419-428] | 122 ms [121-123] |
| Update-validity proof: verify | 43.0 ms [42.4-44.0] | 20.5 ms [20.0-20.8] |
| Update-validity proof: decode | 4.7 ms [4.6-5.1] | 2.4 ms [2.2-2.5] |
| In-memory batch check (`verify_batch`) | 79.3 ms [77.5-80.3] | 52.0 ms [51.5-52.3] |

| Size | Value |
|---|---|
| Clearing proof | 89 commitments, 6,124 bytes |
| Update-validity proof | 17 commitments, 1,700 bytes |
| Both proofs | 106 commitments, 7,824 bytes |
| Opening points per proof | 2 (challenge and its shift on the proof domain) |
| Pairings for the batched polynomial openings | 2 (empty-column checks are separate) |
| Log | 416 bytes per order |

### 3.2 Scaling in the number of ticks, uniform books

250 orders per batch. Median times in ms. “Verify” times the check on a decoded proof. The “Decode” column is separate.

1 thread:

| N | n | k_in | Commitments | Proof (B) | Prove | Verify | Decode | Update prove | No-cross prove | Batch check |
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

8 threads:

| N | n | k_in | Prove | Verify | Decode | Update prove | No-cross prove | Batch check |
|---|---|---|---|---|---|---|---|---|
| 100 | 128 | 16 | 98 | 6.4 | 3.6 | 122 | 41 | 52 |
| 100 | 128 | 32 | 117 | 6.7 | 5.0 | 131 | 50 | 53 |
| 250 | 256 | 16 | 115 | 6.2 | 2.9 | 113 | 56 | 51 |
| 250 | 256 | 32 | 154 | 6.4 | 3.9 | 122 | 72 | 51 |
| 500 | 512 | 16 | 192 | 6.4 | 3.3 | 115 | 84 | 51 |
| 500 | 512 | 32 | 236 | 6.7 | 4.6 | 123 | 102 | 52 |
| 1,000 | 1,024 | 16 | 291 | 6.4 | 3.6 | 114 | 114 | 52 |
| 1,000 | 1,024 | 32 | 376 | 6.7 | 4.8 | 122 | 128 | 52 |
| 2,000 | 2,048 | 16 | 484 | 6.5 | 3.7 | 174 | 178 | 54 |
| 2,000 | 2,048 | 32 | 615 | 6.8 | 4.7 | 190 | 215 | 54 |
| 4,000 | 4,096 | 16 | 865 | 6.6 | 3.6 | 314 | 314 | 60 |
| 4,000 | 4,096 | 32 | 1,081 | 6.6 | 4.6 | 314 | 354 | 60 |
| 8,000 | 8,192 | 16 | 1,590 | 6.7 | 3.6 | 532 | 533 | 70 |
| 8,000 | 8,192 | 32 | 2,026 | 6.8 | 4.6 | 549 | 618 | 69 |



- Proving time is close to linear in n. One thread, 32-bit: 1,366 ms at n = 1,024 and 8,723 ms at n = 8,192, a factor 6.4 for 8 times the domain.
- Proof size is fixed for a given column layout. The measured verification times vary little across the tested domains of at least 512 rows.
- Below n = 256 the limbs are narrower than 8 bits, because the table must fit in the domain, so small books have more columns. n = 256 has fewer than n = 512 because its widths (24 and 40 bits) are exact multiples of 8.
- The update-validity proof depends on the step domain, not on the tick count alone. With 250 orders the step domain is max(n, 1,024), so its cost is flat up to n = 1,024 and then follows n.
- Eight threads give a speed-up of 3.6 on the clearing proof at n = 1,024 and 4.3 at n = 8,192, on four physical cores.

### 3.3 The no-cross proof

Uniform books of the same size, N = 1,000, k_in = 32: 20 commitments, 1,612 bytes; generate 402 ms (1 thread), 128 ms (8 threads); verify 6.6 ms and 5.7 ms; decode 5.0 ms and 1.7 ms. With k_in = 16: 14 commitments, 1,228 bytes. Section 3.2 gives proving times at the other sizes.

### 3.4 Orders per batch

N = 1,000, k_in = 32. Each order takes four rows of the step domain M. The clearing proof does not change with the order count (376 to 379 ms on 8 threads in all three runs).

| Orders | M | Update prove, 1 thr | Update prove, 8 thr | Update verify, 1 thr | Update verify, 8 thr | Batch check, 1 thr | Batch check, 8 thr |
|---|---|---|---|---|---|---|---|
| 250 | 1,024 | 424 ms | 122 ms | 43 ms | 21 ms | 79 ms | 52 ms |
| 1,000 | 4,096 | 1,351 ms | 347 ms | 109 ms | 41 ms | 167 ms | 118 ms |
| 2,200 | 16,384 | 4,405 ms | 1,083 ms | 207 ms | 67 ms | 298 ms | 175 ms |

The update-validity proof is 1,700 bytes at every order count. Verification includes a multi-scalar multiplication over the logged steps.

On eight threads, the sums of the two component medians are 0.50 s at 250 orders, 0.73 s at 1,000 and 1.46 s at 2,200. These are proving costs after the close. Sustained exchange throughput was not measured.

### 3.5 Per-order and per-batch operations

N = 1,000, k_in = 32. These do not depend on the thread count beyond noise, except the book opening.

| Operation | 1 thread | 8 threads |
|---|---|---|
| Apply an order to the four commitments in memory | 0.98 ms | 0.99 ms |
| Verify the receipt equation | 0.24 ms | 0.24 ms |
| Book opening proof: produce | 13.3 ms | 9.6 ms |
| Book opening proof: check | 19.1 ms | 18.5 ms |
| Clearing rule itself (witness receipt) | 0.07 ms | 0.08 ms |

The order timer excludes appending entries, disk writes and distribution. The receipt timer excludes log membership checks.

### 3.6 Calibrated books

32-bit volumes, clearing proof only, median over 5 books [min-max of the per-book medians].

| N | n | Commitments | Proof (B) | Prove, 1 thr (ms) | Prove, 8 thr (ms) | Verify, 1 thr | Verify, 8 thr | Decode, 1 thr | Decode, 8 thr |
|---|---|---|---|---|---|---|---|---|---|
| 100 | 128 | 97 | 6,636 | 343 [341-345] | 117 [111-130] | 10.0 | 6.5 | 20.6 | 4.6 |
| 250 | 256 | 79 | 5,484 | 414 [408-420] | 150 [134-156] | 9.3 | 6.3 | 17.0 | 3.9 |
| 500 | 512 | 89 | 6,124 | 769 [765-771] | 232 [228-235] | 9.6 | 6.5 | 19.0 | 4.4 |
| 1,000 | 1,024 | 89 | 6,124 | 1,343 [1,339-1,351] | 373 [363-382] | 9.7 | 6.8 | 19.1 | 4.7 |
| 2,000 | 2,048 | 89 | 6,124 | 2,437 [2,433-2,447] | 626 [608-633] | 9.6 | 6.8 | 19.0 | 4.7 |

10 of the 25 books clear on a plateau wider than one tick. At 1,000 ticks their one-thread median proving times are 1.34 s and 1.37 s, respectively.

## 4. Replayed Nasdaq books

32 replayed AAPL batches, 601 to 662 ticks, all with n = 1,024, k_in = 32. All 32 cross on a plateau wider than one tick. Clearing proof only: 89 commitments, 6,124 bytes. Median over the 32 books [min-max of the per-book medians, 20 runs each].

| Metric | 1 thread | 8 threads |
|---|---|---|
| Generate | 1,348 ms [1,334-1,363] | 367 ms [353-381] |
| Verify | 10.2 ms [10.0-10.4] | 7.2 ms [7.0-7.4] |
| Decode | 19.3 ms [19.0-19.4] | 4.7 ms [4.6-4.9] |

Median clearing-proof generation on the replayed books is within 3% of the 1,000-tick uniform timing. The update-validity proof was not run on real books: the replay files hold the book at batch close, not the order stream.

## Cboe outcome books

On 362 balanced, constructed books of 101 ticks each, the median of the five-run Rust proving medians was 384 ms on one thread and 112 ms on eight threads. Each proof was 6,636 bytes. The separate Noir measurement used one eight-thread run per book. For the calibrated and replayed comparisons see [COMPARISON.md](../docs/COMPARISON.md).
