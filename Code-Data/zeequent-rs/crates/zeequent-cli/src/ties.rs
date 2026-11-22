//! `zeequent ties`: how often does a batch need the tie-breaks?

use std::fmt::Write as _;
use std::path::Path;

use ark_std::rand::{rngs::StdRng, SeedableRng};
use zeequent_core::clear;
use zeequent_types::{Book, Order, OrderKind, OrderSide, Outcome, Params};

use crate::data::Table;
use crate::{option, synthetic_orders};

#[derive(Default)]
struct Stats {
    batches: usize,
    crossed: usize,
    wide_plateau: usize,
    valley_tie: usize,
    widths: Vec<usize>,
}

impl Stats {
    fn record(&mut self, book: &Book, params: &Params) -> Result<(), String> {
        self.batches += 1;
        let (outcome, witness) = clear(book, params).map_err(|e| format!("{e:?}"))?;
        if let Outcome::Cross(r) = outcome {
            self.crossed += 1;
            let width = r.d - r.c + 1;
            let valley = (r.c..=r.d).filter(|i| witness.delta[*i] == r.v_min_delta).count();
            self.wide_plateau += (width > 1) as usize;
            self.valley_tie += (valley > 1) as usize;
            self.widths.push(width);
        }
        Ok(())
    }

    fn row(&mut self, label: &str, out: &mut String) {
        let pct = |part: usize| {
            if self.crossed == 0 { 0.0 } else { 100.0 * part as f64 / self.crossed as f64 }
        };
        self.widths.sort_unstable();
        let median = self.widths.get(self.widths.len() / 2).copied().unwrap_or(0);
        let max = self.widths.last().copied().unwrap_or(0);
        println!(
            "| {label} | {} | {} | {:.1}% | {:.1}% | {median} | {max} |",
            self.batches, self.crossed, pct(self.wide_plateau), pct(self.valley_tie)
        );
        writeln!(
            out,
            "{label},{},{},{:.2},{:.2},{median},{max}",
            self.batches, self.crossed, pct(self.wide_plateau), pct(self.valley_tie)
        )
        .unwrap();
    }
}

struct Trade {
    seconds: f64,
    side: OrderSide,
    cents: u64,
    size: u64,
}

/// Seconds since midnight from a timestamp like `2026-06-01 09:30:00.139815971-04:00`.
fn seconds_of_day(timestamp: &str) -> Option<f64> {
    let time = timestamp.split([' ', 'T']).nth(1)?;
    let mut parts = time.split(':');
    let hours: f64 = parts.next()?.parse().ok()?;
    let minutes: f64 = parts.next()?.parse().ok()?;
    let seconds: f64 = parts
        .next()?
        .split(['-', '+', 'Z'])
        .next()?
        .parse()
        .ok()?;
    Some(hours * 3600.0 + minutes * 60.0 + seconds)
}

fn load_trades(path: &Path) -> Result<Vec<Trade>, String> {
    let text = std::fs::read_to_string(path)
        .map_err(|e| format!("cannot read {}: {e}", path.display()))?;
    let mut lines = text.lines();
    let header: Vec<&str> = lines.next().ok_or("empty file")?.split(',').collect();
    let index = |name: &str| {
        header.iter().position(|h| h.trim() == name).ok_or(format!("no {name} column"))
    };
    let (ts, side, price, size) = (index("ts_recv")?, index("side")?, index("price")?, index("size")?);
    let mut trades = Vec::new();
    for line in lines {
        let cells: Vec<&str> = line.split(',').collect();
        let side = match cells.get(side).map(|s| s.trim()) {
            Some("B") => OrderSide::Buy,
            Some("A") => OrderSide::Sell,
            _ => continue,
        };
        let parsed = (|| {
            Some(Trade {
                seconds: seconds_of_day(cells.get(ts)?)?,
                side,
                cents: (cells.get(price)?.trim().parse::<f64>().ok()? * 100.0).round() as u64,
                size: cells.get(size)?.trim().parse().ok()?,
            })
        })();
        trades.push(parsed.ok_or_else(|| format!("bad row: {line}"))?);
    }
    Ok(trades)
}

pub fn run(args: &[String]) -> Result<(), String> {
    let mut csv = String::from(
        "group,batches,crossed,plateau_wider_than_one_pct,valley_tie_pct,median_plateau_width,max_plateau_width\n",
    );
    println!("Percentages are of crossing batches.\n");
    println!("| group | batches | crossing | plateau > 1 tick | valley > 1 tick | median plateau width | max width |");
    println!("|---|---|---|---|---|---|---|");

    // Real trades, cut into batches.
    if let Some(path) = option(args, "--trades") {
        let trades = load_trades(Path::new(path))?;
        let lo = trades.iter().map(|t| t.cents).min().ok_or("no classified trades")?;
        let hi = trades.iter().map(|t| t.cents).max().ok_or("no classified trades")?;
        let n_ticks = (hi - lo + 1) as usize;
        let params = Params::new(n_ticks, 32).map_err(|e| format!("{e:?}"))?;
        let start = trades.iter().map(|t| t.seconds).fold(f64::INFINITY, f64::min).floor();
        let end = trades.iter().map(|t| t.seconds).fold(0.0, f64::max);
        let intervals: Vec<f64> = option(args, "--intervals")
            .unwrap_or("1,5,10,30,60,300")
            .split(',')
            .filter_map(|v| v.trim().parse().ok())
            .collect();
        for interval in intervals {
            let count = ((end - start) / interval).floor() as usize + 1;
            let mut books: Vec<Book> = (0..count).map(|_| Book::new(n_ticks)).collect();
            let mut used = vec![false; count];
            for trade in &trades {
                let batch = ((trade.seconds - start) / interval).floor() as usize;
                let order = Order {
                    side: trade.side,
                    kind: OrderKind::Limit { tick: (trade.cents - lo) as usize },
                    qty: trade.size,
                };
                books[batch].add(&order).map_err(|e| format!("{e:?}"))?;
                used[batch] = true;
            }
            let mut stats = Stats::default();
            for (book, used) in books.iter().zip(&used) {
                // Batches with no orders at all are not counted.
                if *used {
                    stats.record(book, &params)?;
                }
            }
            stats.row(&format!("real trades / {interval} s batches / {n_ticks} ticks"), &mut csv);
        }
    }

    // Whole order-book files.
    if let Some(files) = option(args, "--books") {
        for file in files.split(',') {
            let path = Path::new(file.trim());
            let table = Table::load(path)?;
            let mut book = Book::new(table.rows.len());
            book.bids = table.column("Bids++").ok_or("no Bids++ column")?;
            book.asks = table.column("Asks++").ok_or("no Asks++ column")?;
            let params = Params::new(book.n_ticks(), 32).map_err(|e| format!("{e:?}"))?;
            let mut stats = Stats::default();
            stats.record(&book, &params)?;
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or(file);
            stats.row(&format!("file {name}"), &mut csv);
        }
    }

    // Synthetic sparse-to-dense flow: random orders around a mid price.
    let trials: usize = option(args, "--trials").and_then(|v| v.parse().ok()).unwrap_or(2000);
    let mut rng = StdRng::seed_from_u64(2026);
    for n_ticks in [100usize, 1000] {
        let params = Params::new(n_ticks, 32).map_err(|e| format!("{e:?}"))?;
        for orders in [5usize, 20, 100, 500, 2000, 10000] {
            let mut stats = Stats::default();
            for _ in 0..trials {
                let mut book = Book::new(n_ticks);
                for order in synthetic_orders(n_ticks, orders, 5000, &mut rng) {
                    book.add(&order).map_err(|e| format!("{e:?}"))?;
                }
                stats.record(&book, &params)?;
            }
            stats.row(&format!("synthetic / {orders} orders / {n_ticks} ticks"), &mut csv);
        }
    }

    if let Some(path) = option(args, "--csv") {
        std::fs::write(path, &csv).map_err(|e| format!("cannot write {path}: {e}"))?;
        println!("\nwrote {path}");
    }
    Ok(())
}
