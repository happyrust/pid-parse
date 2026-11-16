//! What does staging a SQL Server `.mdf` cost?
//!
//! `pid_parse::publish::open_mdf_as_sqlite` copies the tables of
//! `PUBLISH_TABLES` out of an `.mdf` into an in-memory `SQLite` database, and
//! `open_mtf_dump_as_sqlite` does the same for the image inside an
//! MTF-wrapped `Export.dmp`. This probe reads what that costs: the bytes read
//! from the image, table by table and against the image's size, and the time.
//! The peak memory is read from outside the process, on a run with
//! `--stage-only`, which does nothing but the staging.
//!
//! The byte count comes from walking the tables the way the loader does — one
//! `MdfDatabase` for all of them, every row read — over a source that counts
//! what is read from it, and how many of the page reads were of a page the
//! reader had read before. The loader itself is then run and timed, and the
//! rows it staged are counted against the rows the walk read: a walk that
//! read other rows than the loader staged would be measuring something else.
//!
//! Last it lists what the loader staged: the columns and the rows of each
//! table, and a digest of every value in the order the rows went in. Two runs
//! that list the same staged the same, be they before and after a change to
//! the loader or on an `.mdf` and on the dump that wraps it.
//!
//! ```powershell
//! cargo run --release --example probe_what_staging_an_mdf_costs -- `
//!     <Export.mdf> [--stage-only]
//! cargo run --release --example probe_what_staging_an_mdf_costs -- `
//!     <Export.dmp> --mtf [--stage-only]
//! ```
//!
//! With `--mtf` the input is an MTF dump. The walk runs over the image
//! `mdf_bytes_from_mtf_dump` takes out of it, written to a temporary file;
//! the loader timed is `open_mtf_dump_as_sqlite` on the dump read whole,
//! which is what publishing a package does. `--stage-only` skips the walk and
//! the listing.
//!
//! Exits 2 on a usage error and 1 when the input does not read, a table does
//! not parse, or the walk and the loader disagree about the rows; a
//! measurement, whatever it says, exits 0.

use std::cell::RefCell;
use std::collections::BTreeSet;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::rc::Rc;
use std::time::Instant;

use oxidized_mdf::MdfDatabase;
use pid_parse::publish::mdf_load::PUBLISH_TABLES;
use pid_parse::publish::{mdf_bytes_from_mtf_dump, open_mdf_as_sqlite, open_mtf_dump_as_sqlite};
use rusqlite::Connection;
use sha2::{Digest, Sha256};

const USAGE: &str =
    "usage: probe_what_staging_an_mdf_costs <Export.mdf | Export.dmp --mtf> [--stage-only]";

/// The size of a SQL Server page.
const PAGE_BYTES: u64 = 8192;

fn main() -> ExitCode {
    let mut input = None;
    let mut mtf = false;
    let mut stage_only = false;
    for arg in std::env::args().skip(1) {
        match arg.as_str() {
            "--mtf" => mtf = true,
            "--stage-only" => stage_only = true,
            path if input.is_none() && !path.starts_with("--") => {
                input = Some(PathBuf::from(path));
            }
            _ => {
                eprintln!("{USAGE}");
                return ExitCode::from(2);
            }
        }
    }
    let Some(input) = input else {
        eprintln!("{USAGE}");
        return ExitCode::from(2);
    };
    match run(&input, mtf, stage_only) {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("{message}");
            ExitCode::FAILURE
        }
    }
}

fn run(input: &Path, mtf: bool, stage_only: bool) -> Result<(), String> {
    println!("input  {}", input.display());
    let walked = if stage_only {
        None
    } else if mtf {
        let image = TempImage::out_of(input)?;
        Some(walk(&image.path)?)
    } else {
        Some(walk(input)?)
    };
    let staged = stage(input, mtf)?;
    let Some(walked) = walked else {
        return Ok(());
    };
    let held = held_by(&staged)?;
    compare(&walked, &held)?;
    print_staged(&held);
    Ok(())
}

/// A source that adds what is read from it to a tally its maker keeps.
struct Counting<R> {
    inner: R,
    /// Where the next read begins.
    at: u64,
    tally: Rc<RefCell<Tally>>,
}

/// What has been read from a [`Counting`] source.
#[derive(Default)]
struct Tally {
    bytes: u64,
    /// The reads that began at the first byte of a page.
    page_reads: u64,
    /// The pages those reads began at, each once.
    pages: BTreeSet<u64>,
}

impl<R: Read> Read for Counting<R> {
    fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
        let count = self.inner.read(buffer)?;
        let mut tally = self.tally.borrow_mut();
        if count > 0 && self.at.is_multiple_of(PAGE_BYTES) {
            tally.page_reads += 1;
            tally.pages.insert(self.at / PAGE_BYTES);
        }
        tally.bytes += count as u64;
        self.at += count as u64;
        Ok(count)
    }
}

impl<R: Seek> Seek for Counting<R> {
    fn seek(&mut self, to: SeekFrom) -> std::io::Result<u64> {
        self.at = self.inner.seek(to)?;
        Ok(self.at)
    }
}

/// What reading the rows of one table cost.
struct TableCost {
    name: &'static str,
    /// The bytes read from the image while the rows of this table were read.
    bytes: u64,
    /// The rows read, or `None` for a table the loader skips: one the image
    /// does not hold, or holds without a column.
    rows: Option<usize>,
    millis: f64,
}

/// Reads every table of [`PUBLISH_TABLES`] out of `image` the way
/// `open_mdf_as_sqlite` does, all of them with the one reader, and prints
/// what opening the image and then each table cost.
fn walk(image: &Path) -> Result<Vec<TableCost>, String> {
    let size = std::fs::metadata(image)
        .map_err(|error| format!("{}: {error}", image.display()))?
        .len();
    println!(
        "image  {size} bytes: {} pages of {PAGE_BYTES} and {} bytes over",
        size / PAGE_BYTES,
        size % PAGE_BYTES
    );
    println!();
    println!(
        "{:<20} {:>12} {:>8} {:>7} {:>9}",
        "read", "bytes", "x image", "rows", "ms"
    );
    let started = Instant::now();
    let tally = Rc::new(RefCell::new(Tally::default()));
    let file = File::open(image).map_err(|error| format!("open {}: {error}", image.display()))?;
    let counted = Counting {
        inner: file,
        at: 0,
        tally: Rc::clone(&tally),
    };
    let mut db = MdfDatabase::from_read(counted)
        .map_err(|error| format!("open {}: {error}", image.display()))?;
    // The boot page and the system tables: read before a row is asked for.
    let to_open = tally.borrow().bytes;
    let open_millis = millis_since(started);
    println!(
        "{:<20} {:>12} {:>8.2} {:>7} {:>9.1}",
        "to open the image",
        to_open,
        times(to_open, size),
        "",
        open_millis
    );
    let mut costs = Vec::with_capacity(PUBLISH_TABLES.len());
    for &name in PUBLISH_TABLES {
        let started = Instant::now();
        let read_before = tally.borrow().bytes;
        let staged = db
            .column_names(name)
            .is_some_and(|columns| !columns.is_empty());
        let rows = if staged {
            Some(count_rows(&mut db, name)?)
        } else {
            None
        };
        let cost = TableCost {
            name,
            bytes: tally.borrow().bytes - read_before,
            rows,
            millis: millis_since(started),
        };
        println!(
            "{:<20} {:>12} {:>8.2} {:>7} {:>9.1}",
            cost.name,
            cost.bytes,
            times(cost.bytes, size),
            cost.rows
                .map_or_else(|| "-".to_string(), |rows| rows.to_string()),
            cost.millis
        );
        costs.push(cost);
    }
    let bytes = to_open + costs.iter().map(|cost| cost.bytes).sum::<u64>();
    let rows: usize = costs.iter().filter_map(|cost| cost.rows).sum();
    let millis = open_millis + costs.iter().map(|cost| cost.millis).sum::<f64>();
    println!(
        "{:<20} {:>12} {:>8.2} {:>7} {:>9.1}",
        "all of it",
        bytes,
        times(bytes, size),
        rows,
        millis
    );
    let skipped = costs.iter().filter(|cost| cost.rows.is_none()).count();
    println!(
        "{} tables asked for, {skipped} of them not in the image (`-`): the loader skips those",
        costs.len()
    );
    let tally = tally.borrow();
    println!(
        "{} page reads, {} of them of a page the reader had read before",
        tally.page_reads,
        tally.page_reads - tally.pages.len() as u64
    );
    println!();
    Ok(costs)
}

fn count_rows(db: &mut MdfDatabase, name: &str) -> Result<usize, String> {
    let Some(rows) = db.try_rows(name) else {
        return Ok(0);
    };
    let mut count = 0usize;
    for row in rows {
        row.map_err(|error| format!("{name}: {error}"))?;
        count += 1;
    }
    Ok(count)
}

/// Runs the loader on `input` the way a publish does, says how long it took
/// and returns what it staged.
fn stage(input: &Path, mtf: bool) -> Result<Connection, String> {
    let started = Instant::now();
    if !mtf {
        let staged = open_mdf_as_sqlite(input)
            .map_err(|error| format!("stage {}: {error}", input.display()))?;
        println!("open_mdf_as_sqlite: {:.1} ms", millis_since(started));
        return Ok(staged);
    }
    let dump =
        std::fs::read(input).map_err(|error| format!("read {}: {error}", input.display()))?;
    let read_millis = millis_since(started);
    let staged = open_mtf_dump_as_sqlite(&dump)
        .map_err(|error| format!("stage {}: {error}", input.display()))?;
    let stage_millis = millis_since(started) - read_millis;
    println!(
        "the dump read whole: {} bytes, {read_millis:.1} ms",
        dump.len()
    );
    println!("open_mtf_dump_as_sqlite: {stage_millis:.1} ms");
    Ok(staged)
}

/// What the staged database holds of one table.
struct StagedTable {
    columns: usize,
    rows: usize,
    /// SHA-256 over the names of the columns and then every value of every
    /// row, in the order the rows went in.
    digest: String,
}

/// What `staged` holds of each table of [`PUBLISH_TABLES`], in that order:
/// `None` for a table that was not staged.
fn held_by(staged: &Connection) -> Result<Vec<Option<StagedTable>>, String> {
    PUBLISH_TABLES
        .iter()
        .map(|table| staged_table(staged, table))
        .collect()
}

fn staged_table(staged: &Connection, table: &str) -> Result<Option<StagedTable>, String> {
    let failed = |error: rusqlite::Error| format!("{table}: {error}");
    let held: i64 = staged
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = ?1",
            [table],
            |row| row.get(0),
        )
        .map_err(failed)?;
    if held == 0 {
        return Ok(None);
    }
    let mut statement = staged
        .prepare(&format!("SELECT * FROM \"{table}\" ORDER BY rowid"))
        .map_err(failed)?;
    let columns = statement.column_count();
    let mut hasher = Sha256::new();
    hasher.update((columns as u64).to_le_bytes());
    for name in statement.column_names() {
        add(&mut hasher, Some(name.as_bytes()));
    }
    let mut rows = statement.query([]).map_err(failed)?;
    let mut count = 0usize;
    while let Some(row) = rows.next().map_err(failed)? {
        for column in 0..columns {
            let value: Option<String> = row.get(column).map_err(failed)?;
            add(&mut hasher, value.as_deref().map(str::as_bytes));
        }
        count += 1;
    }
    Ok(Some(StagedTable {
        columns,
        rows: count,
        digest: hex(hasher),
    }))
}

/// Adds `value` to `hasher`, marked as there or not and with its length
/// before it, so that no two runs of values add the same bytes.
fn add(hasher: &mut Sha256, value: Option<&[u8]>) {
    match value {
        None => hasher.update([0u8]),
        Some(bytes) => {
            hasher.update([1u8]);
            hasher.update((bytes.len() as u64).to_le_bytes());
            hasher.update(bytes);
        }
    }
}

fn hex(hasher: Sha256) -> String {
    use std::fmt::Write as _;
    hasher
        .finalize()
        .iter()
        .fold(String::with_capacity(64), |mut hex, byte| {
            let _ = write!(hex, "{byte:02x}");
            hex
        })
}

/// The rows the loader staged against the rows the walk read, table by table.
fn compare(walked: &[TableCost], held: &[Option<StagedTable>]) -> Result<(), String> {
    let mut differences = Vec::new();
    for (cost, held) in walked.iter().zip(held) {
        let staged_rows = held.as_ref().map(|held| held.rows);
        if staged_rows != cost.rows {
            differences.push(format!(
                "  {}: the walk read {:?}, the loader staged {staged_rows:?}",
                cost.name, cost.rows
            ));
        }
    }
    if differences.is_empty() {
        println!("rows: the loader staged what the walk read, table by table");
        Ok(())
    } else {
        Err(format!(
            "the walk and the loader disagree about the rows:\n{}",
            differences.join("\n")
        ))
    }
}

/// Lists what the loader staged, table by table, and one digest over the
/// whole list.
fn print_staged(held: &[Option<StagedTable>]) {
    println!();
    println!(
        "{:<20} {:>7} {:>7}  sha-256 of the column names and the values",
        "staged", "columns", "rows"
    );
    let mut all = Sha256::new();
    let mut rows = 0usize;
    for (&table, held) in PUBLISH_TABLES.iter().zip(held) {
        add(&mut all, Some(table.as_bytes()));
        if let Some(held) = held {
            add(&mut all, Some(held.digest.as_bytes()));
            rows += held.rows;
            println!(
                "{table:<20} {:>7} {:>7}  {}",
                held.columns, held.rows, held.digest
            );
        } else {
            add(&mut all, None);
            println!("{table:<20} {:>7} {:>7}  -", "-", "-");
        }
    }
    println!("{:<20} {:>7} {rows:>7}  {}", "all of them", "", hex(all));
}

/// The image inside an MTF dump, in a temporary file that is removed when
/// this goes out of scope.
struct TempImage {
    path: PathBuf,
}

impl TempImage {
    fn out_of(dump_path: &Path) -> Result<Self, String> {
        let dump = std::fs::read(dump_path)
            .map_err(|error| format!("read {}: {error}", dump_path.display()))?;
        let image = mdf_bytes_from_mtf_dump(&dump)
            .map_err(|error| format!("{}: {error}", dump_path.display()))?;
        println!(
            "dump   {} bytes; the image inside is {} bytes",
            dump.len(),
            image.len()
        );
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |elapsed| elapsed.as_nanos());
        let path = std::env::temp_dir().join(format!(
            "pid-parse-probe-{}-{nanos}.mdf",
            std::process::id()
        ));
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .map_err(|error| format!("create {}: {error}", path.display()))?;
        let image_file = Self { path };
        file.write_all(&image)
            .map_err(|error| format!("write {}: {error}", image_file.path.display()))?;
        Ok(image_file)
    }
}

impl Drop for TempImage {
    fn drop(&mut self) {
        if let Err(error) = std::fs::remove_file(&self.path) {
            eprintln!("could not remove {}: {error}", self.path.display());
        }
    }
}

/// `bytes` as a multiple of the image's `size`.
fn times(bytes: u64, size: u64) -> f64 {
    bytes as f64 / size as f64
}

fn millis_since(started: Instant) -> f64 {
    started.elapsed().as_secs_f64() * 1000.0
}
