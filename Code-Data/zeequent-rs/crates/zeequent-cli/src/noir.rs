//! `zeequent export-noir`: write the legacy book and receipt TOML files. The standalone circuit uses its own CSV converter.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use zeequent_core::clear;
use zeequent_types::{Book, Outcome, Params, Side};

use crate::data::Table;
use crate::option;

fn array(name: &str, values: &[u64]) -> String {
    let cells: Vec<String> = values.iter().map(|v| format!("\"{v}\"")).collect();
    format!("{name} = [{}]\n", cells.join(", "))
}

/// `export-noir --file BOOK.csv --out DIR`
pub fn run(args: &[String]) -> Result<(), String> {
    let path = Path::new(option(args, "--file").ok_or("missing --file")?);
    let out = PathBuf::from(option(args, "--out").ok_or("missing --out")?);
    let table = Table::load(path)?;
    let bids = table.column("Bids++").ok_or("no Bids++ column")?;
    let asks = table.column("Asks++").ok_or("no Asks++ column")?;
    let n_ticks = bids.len();
    let empty = vec![0u64; n_ticks];
    let book = Book {
        bids,
        asks,
        io_bids: table.column("IO_Bids").unwrap_or_else(|| empty.clone()),
        io_asks: table.column("IO_Asks").unwrap_or(empty),
    };
    // The circuit's volumes are u32.
    let params = Params::new(n_ticks, 32).map_err(|e| format!("{e:?}"))?;
    let (outcome, _) = clear(&book, &params).map_err(|e| format!("clearing failed: {e:?}"))?;

    let mut text = String::from("[book]\n");
    text += &array("bids", &book.bids);
    text += &array("asks", &book.asks);
    text += &array("io_bids", &book.io_bids);
    text += &array("io_asks", &book.io_asks);
    std::fs::create_dir_all(&out).map_err(|e| e.to_string())?;
    std::fs::write(out.join("book.toml"), &text).map_err(|e| e.to_string())?;

    let receipt_path = out.join("receipt.toml");
    match outcome {
        Outcome::NoCross => {
            let _ = std::fs::remove_file(&receipt_path);
            println!("ticks={n_ticks}\noutcome=no-cross");
        }
        Outcome::Cross(r) => {
            let mut text = String::from("[receipt]\n");
            writeln!(text, "v_max = \"{}\"", r.v_max).unwrap();
            writeln!(text, "c = \"{}\"", r.c).unwrap();
            writeln!(text, "d = \"{}\"", r.d).unwrap();
            writeln!(text, "p_star = \"{}\"", r.p_star).unwrap();
            writeln!(text, "v_min_delta = \"{}\"", r.v_min_delta).unwrap();
            writeln!(text, "side = {}", r.side == Side::SupplyLong).unwrap();
            writeln!(text, "p_at_price = \"{}\"", r.p_at_price).unwrap();
            writeln!(text, "j_io = \"{}\"", r.j_io).unwrap();
            std::fs::write(&receipt_path, &text).map_err(|e| e.to_string())?;
            println!("ticks={n_ticks}\noutcome=cross");
        }
    }
    Ok(())
}
