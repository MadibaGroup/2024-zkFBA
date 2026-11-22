//! `zeequent data`: run batches on order-book CSV files.

use std::fmt::Write as _;
use std::path::Path;

use ark_std::rand::{rngs::StdRng, SeedableRng};
use zeequent_core::{
    prove, prove_no_cross, required_srs_degree, verify, verify_no_cross, BookState, Proof,
};
use zeequent_kzg::Srs;
use zeequent_types::{Order, OrderKind, OrderSide, Outcome, Params};

use crate::{median, ms, option, threads, timed};

pub(crate) struct Table {
    header: Vec<String>,
    pub(crate) rows: Vec<Vec<u64>>,
}

impl Table {
    pub(crate) fn load(path: &Path) -> Result<Self, String> {
        let text = std::fs::read_to_string(path)
            .map_err(|e| format!("cannot read {}: {e}", path.display()))?;
        let mut lines = text.lines().filter(|line| !line.trim().is_empty());
        let header: Vec<String> = lines
            .next()
            .ok_or("empty file")?
            .trim_start_matches('\u{feff}')
            .split(',')
            .map(|name| name.trim().to_string())
            .collect();
        let rows = lines
            .enumerate()
            .map(|(index, line)| {
                line.split(',')
                    .map(|cell| {
                        // Cells are integers, sometimes written as "12.0".
                        cell.trim()
                            .parse::<f64>()
                            .ok()
                            .filter(|v| *v >= 0.0 && v.fract() == 0.0)
                            .map(|v| v as u64)
                            .ok_or_else(|| format!("row {}: bad cell {cell:?}", index + 2))
                    })
                    .collect::<Result<Vec<u64>, String>>()
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self { header, rows })
    }

    pub(crate) fn column(&self, name: &str) -> Option<Vec<u64>> {
        let index = self.header.iter().position(|h| h == name)?;
        self.rows.iter().map(|row| row.get(index).copied()).collect()
    }
}

fn verdict(matches: Option<bool>) -> &'static str {
    match matches {
        Some(true) => "match",
        Some(false) => "DIFFER",
        None => "n/a",
    }
}

/// `data --files A.csv,B.csv [--kin 16] [--runs 3] [--csv OUT]`
pub fn run(args: &[String]) -> Result<(), String> {
    let files: Vec<&str> = option(args, "--files").ok_or("missing --files")?.split(',').collect();
    let k_in: u32 = option(args, "--kin").and_then(|v| v.parse().ok()).unwrap_or(16);
    let runs: usize = option(args, "--runs").and_then(|v| v.parse().ok()).unwrap_or(3).max(1);

    println!("threads = {}, runs = {runs} (median), k_in = {k_in}. Times in ms.\n", threads());
    println!("| file | N | n | V_max | plateau | p* (price) | imbalance | depths | Min | MCV | clearing price | commitments | proof bytes | prove | verify | decode |");
    println!("|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|");
    let mut csv = String::from(
        "file,n_ticks,n,k_in,k,threads,runs,outcome,v_max,c,d,p_star,p_star_price,v_min_delta,side,depths_match,min_match,mcv_match,clearing_price_match,commitments,proof_bytes,prove_ms,verify_ms,decode_ms\n",
    );

    for file in files {
        let path = Path::new(file.trim());
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or(file);
        let table = Table::load(path)?;
        let column = |c: &str| table.column(c);
        let bids = column("Bids++").ok_or(format!("{name}: no Bids++ column"))?;
        let asks = column("Asks++").ok_or(format!("{name}: no Asks++ column"))?;
        let prices = column("Price").unwrap_or_else(|| (0..bids.len() as u64).collect());
        let n_ticks = bids.len();
        let params = Params::new(n_ticks, k_in).map_err(|e| format!("{name}: {e:?}"))?;

        let mut rng = StdRng::seed_from_u64(1);
        let (srs, key) =
            Srs::insecure_setup_for_domain(required_srs_degree(&params), params.n, &mut rng)
                .ok_or("no evaluation domain of that size")?;
        let vk = srs.verifier_key();

        let (mut prove_t, mut verify_t, mut decode_t) = (vec![], vec![], vec![]);
        let mut summary = None;
        for _ in 0..runs {
            let (mut state, _) = BookState::open(&params, &srs, &key, &mut rng)
                .map_err(|e| format!("{name}: {e:?}"))?;
            for tick in 0..n_ticks {
                for (side, qty) in [(OrderSide::Buy, bids[tick]), (OrderSide::Sell, asks[tick])] {
                    if qty > 0 {
                        let order = Order { side, kind: OrderKind::Limit { tick }, qty };
                        state
                            .apply(&order, &vk, &key, &mut rng)
                            .map_err(|e| format!("{name}: tick {tick}: {e:?}"))?;
                    }
                }
            }
            let (outcome, witness) = state.clear().map_err(|e| format!("{name}: {e:?}"))?;
            let (proof, t) = timed(|| match &outcome {
                Outcome::Cross(r) => prove(&params, r, &witness, &state.columns, &srs, &key, &mut rng),
                Outcome::NoCross => prove_no_cross(&params, &witness, &state.columns, &srs, &key, &mut rng),
            });
            let proof = proof.map_err(|e| format!("{name}: proving failed: {e:?}"))?;
            prove_t.push(t);
            let bytes = proof.to_bytes();
            let (decoded, t) = timed(|| Proof::from_bytes(&bytes));
            decode_t.push(t);
            let decoded = decoded.ok_or("proof does not decode")?;
            let book = state.commitments();
            let (result, t) = timed(|| match &outcome {
                Outcome::Cross(r) => verify(&params, r, &book, &decoded, &vk),
                Outcome::NoCross => verify_no_cross(&params, &book, &decoded, &vk),
            });
            verify_t.push(t);
            result.map_err(|e| format!("{name}: verification failed: {e:?}"))?;

            // Compare with the spreadsheet's own derived columns.
            let same = |c: &str, ours: &[u64]| column(c).map(|theirs| theirs[..] == ours[..n_ticks]);
            let depths = match (same("Bid_Depth", &witness.acc_b), same("Ask_Depth", &witness.acc_a)) {
                (Some(a), Some(b)) => Some(a && b),
                _ => None,
            };
            let min = same("Min_Bid_Ask", &witness.min);
            summary = Some((outcome, depths, min, proof.num_commitments(), bytes.len()));
        }

        let (outcome, depths, min, commitments, proof_bytes) = summary.expect("at least one run");
        let times = [ms(median(&mut prove_t)), ms(median(&mut verify_t)), ms(median(&mut decode_t))];
        let first = |c: &str| column(c).and_then(|values| values.first().copied());
        match outcome {
            Outcome::Cross(r) => {
                let price = prices[r.p_star];
                let mcv = first("MCV").map(|v| v == r.v_max);
                // A Clearing_Price of 0 in every row means there is nothing to compare.
                let clearing = column("Clearing_Price")
                    .filter(|values| price == 0 || values.iter().any(|v| *v != 0))
                    .and_then(|values| values.first().map(|v| *v == price));
                println!(
                    "| {name} | {n_ticks} | {} | {} | {}..{} | {} ({price}) | {} {} | {} | {} | {} | {} | {commitments} | {proof_bytes} | {:.1} | {:.1} | {:.1} |",
                    params.n, r.v_max, r.c, r.d, r.p_star, r.v_min_delta,
                    if r.v_min_delta == 0 { "" } else if r.side.bit() == 0 { "buy" } else { "sell" },
                    verdict(depths), verdict(min), verdict(mcv), verdict(clearing),
                    times[0], times[1], times[2]
                );
                writeln!(
                    csv,
                    "{name},{n_ticks},{},{k_in},{},{},{runs},cross,{},{},{},{},{price},{},{},{},{},{},{},{commitments},{proof_bytes},{:.3},{:.3},{:.3}",
                    params.n, params.k, threads(), r.v_max, r.c, r.d, r.p_star, r.v_min_delta,
                    r.side.bit(), verdict(depths), verdict(min), verdict(mcv), verdict(clearing),
                    times[0], times[1], times[2]
                )
                .unwrap();
            }
            Outcome::NoCross => {
                println!(
                    "| {name} | {n_ticks} | {} | 0 | no cross | | | {} | {} | | | {commitments} | {proof_bytes} | {:.1} | {:.1} | {:.1} |",
                    params.n, verdict(depths), verdict(min), times[0], times[1], times[2]
                );
                writeln!(
                    csv,
                    "{name},{n_ticks},{},{k_in},{},{},{runs},no-cross,0,,,,,,,{},{},,,{commitments},{proof_bytes},{:.3},{:.3},{:.3}",
                    params.n, params.k, threads(), verdict(depths), verdict(min),
                    times[0], times[1], times[2]
                )
                .unwrap();
            }
        }
    }

    if let Some(path) = option(args, "--csv") {
        std::fs::write(path, &csv).map_err(|e| format!("cannot write {path}: {e}"))?;
        println!("\nwrote {path}");
    }
    Ok(())
}
