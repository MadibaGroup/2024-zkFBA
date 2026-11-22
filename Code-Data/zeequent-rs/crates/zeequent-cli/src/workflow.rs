//! File-based commands: one batch lives in a directory.

use std::fs;
use std::path::{Path, PathBuf};

use ark_std::rand::rngs::OsRng;
use zeequent_core::{
    commitments_from_bytes, commitments_to_bytes, fill, prove, prove_no_cross, prove_updates,
    required_srs_degree, required_srs_degree_updates, update_domain_size, verify_book_opening,
    verify_update, BatchError, Board, BookOpening, BookState, Proof, PublishedBatch, UpdateProof,
    UpdateReceipt,
};
use zeequent_kzg::codec::{Reader, Writer};
use zeequent_kzg::{setup_from_bytes, setup_to_bytes, DomainKey, Srs};
use zeequent_types::{Order, OrderKind, OrderSide, Outcome, Params};

use crate::option;

const SETUP_MAGIC: &[u8; 8] = b"ZQSETUP2";

type CommandResult = Result<(), String>;

fn required<'a>(args: &'a [String], name: &str) -> Result<&'a str, String> {
    option(args, name).ok_or_else(|| format!("missing {name}"))
}

fn number<T: std::str::FromStr>(args: &[String], name: &str) -> Result<T, String> {
    required(args, name)?
        .parse()
        .map_err(|_| format!("{name} is not a valid number"))
}

fn read(path: &Path) -> Result<Vec<u8>, String> {
    fs::read(path).map_err(|e| format!("cannot read {}: {e}", path.display()))
}

fn write(path: &Path, bytes: &[u8]) -> CommandResult {
    fs::write(path, bytes).map_err(|e| format!("cannot write {}: {e}", path.display()))
}

struct Setup {
    params: Params,
    srs: Srs,
    key: DomainKey,
    /// Size of the step domain of the update-validity proof.
    update_size: usize,
    insecure: bool,
}

impl Setup {
    /// The Lagrange-basis key of the step domain.
    fn update_key(&self) -> Result<DomainKey, String> {
        if self.update_size == self.key.n {
            Ok(self.key.clone())
        } else {
            DomainKey::lagrange_only(self.update_size, &self.srs)
                .ok_or_else(|| "the setup is too small for the update-validity proof".to_string())
        }
    }
}

fn load_setup(args: &[String]) -> Result<Setup, String> {
    let path = PathBuf::from(required(args, "--setup")?);
    let bytes = read(&path)?;
    let parse = || {
        let mut r = Reader::new(&bytes, SETUP_MAGIC)?;
        let n_ticks = usize::try_from(r.u64()?).ok()?;
        let k_in = u32::try_from(r.u64()?).ok()?;
        let insecure = r.u64()? == 1;
        let update_size = usize::try_from(r.u64()?).ok()?;
        let (srs, key) = setup_from_bytes(r.bytes()?)?;
        r.finish()?;
        let params = Params::new(n_ticks, k_in).ok()?;
        (key.n == params.n).then_some(Setup { params, srs, key, update_size, insecure })
    };
    let setup = parse().ok_or_else(|| format!("{} is not a valid setup file", path.display()))?;
    if setup.insecure {
        eprintln!("warning: this setup was generated locally and is insecure; use it for tests only");
    }
    Ok(setup)
}

fn dir(args: &[String]) -> Result<PathBuf, String> {
    Ok(PathBuf::from(required(args, "--dir")?))
}

fn load_state(setup: &Setup, dir: &Path) -> Result<BookState, String> {
    BookState::from_bytes(&setup.params, &read(&dir.join("state.bin"))?)
        .ok_or_else(|| "state.bin is malformed or belongs to another setup".to_string())
}

fn load_opening(dir: &Path) -> Result<BookOpening, String> {
    BookOpening::from_bytes(&read(&dir.join("opening.bin"))?)
        .ok_or_else(|| "opening.bin is malformed".to_string())
}

fn load_board(dir: &Path) -> Result<Board, String> {
    Board::from_bytes(&read(&dir.join("board.bin"))?)
        .ok_or_else(|| "board.bin is malformed".to_string())
}

fn load_outcome(dir: &Path) -> Result<Outcome, String> {
    let text = String::from_utf8(read(&dir.join("outcome.txt"))?)
        .map_err(|_| "outcome.txt is not text".to_string())?;
    Outcome::from_text(&text).ok_or_else(|| "outcome.txt is malformed".to_string())
}

/// `setup --ticks N --kin K [--max-orders M] (--ptau FILE | --insecure) --out FILE`
pub fn setup(args: &[String]) -> CommandResult {
    let params = Params::new(number(args, "--ticks")?, number(args, "--kin")?)
        .map_err(|e| format!("invalid parameters: {e:?}"))?;
    // Every order logs four steps, one per column.
    let max_orders: usize = match option(args, "--max-orders") {
        Some(_) => number(args, "--max-orders")?,
        None => params.n / 4,
    };
    let update_size = update_domain_size(params.n, 4 * max_orders);
    let degree = required_srs_degree(&params).max(required_srs_degree_updates(update_size));
    let insecure = args.iter().any(|a| a == "--insecure");
    let (srs, key) = match (option(args, "--ptau"), insecure) {
        (Some(path), false) => Srs::from_ptau(Path::new(path), degree, params.n)
            .map_err(|e| format!("cannot import {path}: {e:?}"))?,
        (None, true) => Srs::insecure_setup_for_domain(degree, params.n, &mut OsRng)
            .ok_or("no evaluation domain of that size")?,
        _ => return Err("give exactly one of --ptau FILE or --insecure".to_string()),
    };
    let mut w = Writer::new(SETUP_MAGIC);
    w.u64(params.n_ticks as u64);
    w.u64(params.k_in as u64);
    w.u64(insecure as u64);
    w.u64(update_size as u64);
    w.bytes(&setup_to_bytes(&srs, &key));
    let out = PathBuf::from(required(args, "--out")?);
    write(&out, &w.finish())?;
    println!(
        "setup written to {}: N = {}, n = {}, k_in = {}, k = {}, at most {} orders per batch{}",
        out.display(),
        params.n_ticks,
        params.n,
        params.k_in,
        params.k,
        update_size / 4,
        if insecure { " (INSECURE local setup)" } else { " (from ceremony file)" }
    );
    Ok(())
}

/// `open --setup FILE --dir DIR`
pub fn open(args: &[String]) -> CommandResult {
    let setup = load_setup(args)?;
    let dir = dir(args)?;
    fs::create_dir_all(dir.join("receipts")).map_err(|e| e.to_string())?;
    if dir.join("state.bin").exists() {
        return Err(format!("{} already holds a batch", dir.display()));
    }
    let (state, opening) = BookState::open(&setup.params, &setup.srs, &setup.key, &mut OsRng)
        .map_err(|e| format!("cannot open the book: {e:?}"))?;
    write(&dir.join("state.bin"), &state.to_bytes())?;
    write(&dir.join("opening.bin"), &opening.to_bytes())?;
    write(&dir.join("book.bin"), &commitments_to_bytes(&state.commitments()))?;
    let board = Board::new();
    write(&dir.join("board.bin"), &board.to_bytes())?;
    println!("batch opened in {}", dir.display());
    println!("log digest: {}", board.head_digest(&opening));
    Ok(())
}

/// `check-opening --setup FILE --dir DIR`
pub fn check_opening(args: &[String]) -> CommandResult {
    let setup = load_setup(args)?;
    let opening = BookOpening::from_bytes(&read(&dir(args)?.join("opening.bin"))?)
        .ok_or("opening.bin is malformed")?;
    if verify_book_opening(&opening, &setup.srs.verifier_key(), &setup.key) {
        println!("the book was opened empty: VALID");
        Ok(())
    } else {
        Err("the opening proof is INVALID".to_string())
    }
}

fn parse_order(args: &[String]) -> Result<Order, String> {
    let side = match required(args, "--side")? {
        "buy" => OrderSide::Buy,
        "sell" => OrderSide::Sell,
        _ => return Err("--side must be buy or sell".to_string()),
    };
    let kind = match option(args, "--type").unwrap_or("limit") {
        "limit" => OrderKind::Limit { tick: number(args, "--tick")? },
        "io" => OrderKind::Io { tick: number(args, "--tick")? },
        "market" => OrderKind::Market,
        _ => return Err("--type must be limit, market or io".to_string()),
    };
    Ok(Order { side, kind, qty: number(args, "--qty")? })
}

/// `order --setup FILE --dir DIR --side buy|sell [--type limit|market|io] [--tick T] --qty Q`
pub fn order(args: &[String]) -> CommandResult {
    let setup = load_setup(args)?;
    let dir = dir(args)?;
    if dir.join("proof.bin").exists() {
        return Err("this batch is closed".to_string());
    }
    let mut state = load_state(&setup, &dir)?;
    if state.history.len() + 4 > setup.update_size {
        return Err(format!("this batch is full ({} orders)", setup.update_size / 4));
    }
    let order = parse_order(args)?;
    let receipt = state
        .apply(&order, &setup.srs.verifier_key(), &setup.key, &mut OsRng)
        .map_err(|e| format!("order refused: {e:?}"))?;

    let count = fs::read_dir(dir.join("receipts")).map_err(|e| e.to_string())?.count();
    let receipt_path = dir.join("receipts").join(format!("{:04}.bin", count + 1));
    write(&receipt_path, &receipt.to_bytes())?;
    let opening = load_opening(&dir)?;
    let mut board = load_board(&dir)?;
    board.append_order(&opening, &state.commitments());
    write(&dir.join("state.bin"), &state.to_bytes())?;
    write(&dir.join("book.bin"), &commitments_to_bytes(&state.commitments()))?;
    write(&dir.join("board.bin"), &board.to_bytes())?;
    println!(
        "order recorded: {:?} tick {} qty {}; receipt at {}",
        receipt.column,
        receipt.tick,
        receipt.qty,
        receipt_path.display()
    );
    println!(
        "order {} of at most {}; log digest: {}",
        board.entries.len() / 4,
        setup.update_size / 4,
        board.head_digest(&opening)
    );
    Ok(())
}

/// `check-receipt --setup FILE --receipt FILE [--dir DIR]`
pub fn check_receipt(args: &[String]) -> CommandResult {
    let setup = load_setup(args)?;
    let receipt = UpdateReceipt::from_bytes(&read(Path::new(required(args, "--receipt")?))?)
        .ok_or("the receipt file is malformed")?;
    println!(
        "receipt: {:?} tick {} qty {}",
        receipt.column, receipt.tick, receipt.qty
    );
    if !verify_update(&receipt, &setup.srs.verifier_key(), &setup.key) {
        return Err("the update receipt is INVALID".to_string());
    }
    println!("receipt commitment equation: VALID");
    // With --dir, also check that this update is part of the published log.
    if let Some(dir) = option(args, "--dir") {
        let dir = PathBuf::from(dir);
        let opening = load_opening(&dir)?;
        let board = load_board(&dir)?;
        if board.verify(&opening).is_none() {
            return Err("the commitment log is INVALID (its digests do not chain)".to_string());
        }
        if !board.contains(&opening, &receipt) {
            return Err("this update is NOT in the published commitment log".to_string());
        }
        println!("this update is in the published commitment log: VALID");
    }
    Ok(())
}

/// `close --setup FILE --dir DIR`
pub fn close(args: &[String]) -> CommandResult {
    let setup = load_setup(args)?;
    let dir = dir(args)?;
    let state = load_state(&setup, &dir)?;
    let (outcome, witness) = state.clear().map_err(|e| format!("clearing failed: {e:?}"))?;
    // Randomness comes from the private state, so a repeated close gives the same files.
    let mut rng = state.prover_rng(b"clearing");
    let proof = match &outcome {
        Outcome::Cross(receipt) => prove(
            &setup.params, receipt, &witness, &state.columns, &setup.srs, &setup.key, &mut rng,
        ),
        Outcome::NoCross => prove_no_cross(
            &setup.params, &witness, &state.columns, &setup.srs, &setup.key, &mut rng,
        ),
    }
    .map_err(|e| format!("proving failed: {e:?}"))?;
    let updates = prove_updates(
        &setup.params,
        &state.history,
        &setup.srs,
        &setup.key,
        &setup.update_key()?,
        &mut state.prover_rng(b"updates"),
    )
    .map_err(|e| format!("proving the updates failed: {e:?}"))?;
    let update_bytes = updates.to_bytes();
    let bytes = proof.to_bytes();
    write(&dir.join("outcome.txt"), outcome.to_text().as_bytes())?;
    write(&dir.join("proof.bin"), &bytes)?;
    write(&dir.join("updates.bin"), &update_bytes)?;
    print!("{}", outcome.to_text());
    if let Outcome::Cross(receipt) = &outcome {
        println!(
            "admitted_io={}\nresidual={}\nv_exec={}",
            receipt.admitted_io(),
            receipt.residual(),
            receipt.v_exec()
        );
    }
    println!("proof: {} commitments, {} bytes", proof.num_commitments(), bytes.len());
    println!(
        "update-validity proof: {} logged steps, {} commitments, {} bytes",
        state.history.len(),
        updates.proof.num_commitments() + 1,
        update_bytes.len()
    );
    Ok(())
}

/// `verify --setup FILE --dir DIR`
pub fn verify_batch(args: &[String]) -> CommandResult {
    let setup = load_setup(args)?;
    let dir = dir(args)?;
    let vk = setup.srs.verifier_key();
    // Reject a setup whose points are not consistent.
    if !setup.srs.is_consistent(&setup.key, &mut OsRng) {
        return Err("the setup file is INVALID (its points are not consistent)".to_string());
    }
    println!("the setup is internally consistent");

    let opening = load_opening(&dir)?;
    let board = load_board(&dir)?;
    let outcome = load_outcome(&dir)?;
    let proof = Proof::from_bytes(&read(&dir.join("proof.bin"))?).ok_or("proof.bin is malformed")?;
    let updates = UpdateProof::from_bytes(&read(&dir.join("updates.bin"))?)
        .ok_or("updates.bin is malformed")?;
    let batch = PublishedBatch {
        opening: &opening,
        board: &board,
        outcome: &outcome,
        proof: &proof,
        updates: &updates,
    };
    let verified =
        match zeequent_core::verify_batch(&setup.params, setup.update_size, &batch, &vk, &setup.key)
        {
            Ok(verified) => verified,
            Err(BatchError::Opening) => return Err("the opening proof is INVALID".to_string()),
            Err(BatchError::Log) => {
                return Err("the commitment log is INVALID (its digests do not chain)".to_string())
            }
            Err(BatchError::Updates(e)) => {
                return Err(format!("the update-validity proof is INVALID ({e:?})"))
            }
            Err(BatchError::Outcome(e)) => {
                return Err(format!("the clearing proof is INVALID ({e:?})"))
            }
        };
    // The four commitments published beside the log must be its head.
    let book = commitments_from_bytes(&read(&dir.join("book.bin"))?).ok_or("book.bin is malformed")?;
    if book != verified.book {
        return Err("book.bin is not the head of the commitment log".to_string());
    }
    println!(
        "book opened empty; {} logged steps; log digest {}",
        verified.steps,
        verified.log_digest
    );
    println!("logged steps encode bounded non-negative contributions: VALID");
    println!("outcome proof for this log: VALID");
    println!("compare the log digest above with the one the exchange published");
    Ok(())
}
/// `fill --setup FILE --dir DIR --side buy|sell [--type limit|market|io] [--tick T] --qty Q`
pub fn fill_for(args: &[String]) -> CommandResult {
    let setup = load_setup(args)?;
    let order = parse_order(args)?;
    match load_outcome(&dir(args)?)? {
        Outcome::NoCross => println!("fill = 0 (the batch did not cross)"),
        Outcome::Cross(receipt) => println!(
            "fill = {} of {} at tick {}",
            fill(&receipt, setup.params.n_ticks, &order),
            order.qty,
            receipt.p_star
        ),
    }
    Ok(())
}
