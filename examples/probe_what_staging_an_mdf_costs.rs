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
//! `MdfDatabase` per table, every row read — over a `Read` that counts what
//! it hands out. The loader itself is then run and timed, and the rows it
//! staged are counted against the rows the walk read: a walk that read other
//! rows than the loader staged would be measuring something else.
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
//! which is what publishing a package does. `--stage-only` skips the walk.
//!
//! Exits 2 on a usage error and 1 when the input does not read, a table does
//! not parse, or the walk and the loader disagree about the rows; a
//! measurement, whatever it says, exits 0.

use std::cell::Cell;
use std::fs::File;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::rc::Rc;
use std::time::Instant;

use oxidized_mdf::MdfDatabase;
use pid_parse::publish::mdf_load::PUBLISH_TABLES;
use pid_parse::publish::{mdf_bytes_from_mtf_dump, open_mdf_as_sqlite, open_mtf_dump_as_sqlite};
use rusqlite::Connection;

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
    match walked {
        Some(walked) => compare(&walked, &staged),
        None => Ok(()),
    }
}

/// A `Read` that adds every byte it hands out to a count its maker keeps.
struct Counting<R> {
    inner: R,
    read: Rc<Cell<u64>>,
}

impl<R: Read> Read for Counting<R> {
    fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
        let count = self.inner.read(buffer)?;
        self.read.set(self.read.get() + count as u64);
        Ok(count)
    }
}

/// What reading one table cost.
struct TableCost {
    name: &'static str,
    /// The bytes read before the first row is asked for: the boot page and
    /// the system tables.
    to_open: u64,
    /// The bytes read from there until the last row is out.
    for_the_rows: u64,
    /// The rows read, or `None` for a table the loader skips: one the image
    /// does not hold, or holds without a column.
    rows: Option<usize>,
    millis: f64,
}

/// Reads every table of [`PUBLISH_TABLES`] out of `image` the way
/// `open_mdf_as_sqlite` does, a reader of its own for each, and prints what
/// each read cost.
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
        "{:<20} {:>12} {:>12} {:>13} {:>8} {:>7} {:>9}",
        "table", "to open", "for the rows", "bytes read", "x image", "rows", "ms"
    );
    let mut costs = Vec::with_capacity(PUBLISH_TABLES.len());
    for &name in PUBLISH_TABLES {
        let cost = walk_table(image, name)?;
        let bytes = cost.to_open + cost.for_the_rows;
        let rows = cost
            .rows
            .map_or_else(|| "-".to_string(), |rows| rows.to_string());
        println!(
            "{:<20} {:>12} {:>12} {:>13} {:>8.2} {:>7} {:>9.1}",
            cost.name,
            cost.to_open,
            cost.for_the_rows,
            bytes,
            times(bytes, size),
            rows,
            cost.millis
        );
        costs.push(cost);
    }
    let to_open: u64 = costs.iter().map(|cost| cost.to_open).sum();
    let for_the_rows: u64 = costs.iter().map(|cost| cost.for_the_rows).sum();
    let rows: usize = costs.iter().filter_map(|cost| cost.rows).sum();
    let millis: f64 = costs.iter().map(|cost| cost.millis).sum();
    println!(
        "{:<20} {:>12} {:>12} {:>13} {:>8.2} {:>7} {:>9.1}",
        "all of them",
        to_open,
        for_the_rows,
        to_open + for_the_rows,
        times(to_open + for_the_rows, size),
        rows,
        millis
    );
    let skipped = costs.iter().filter(|cost| cost.rows.is_none()).count();
    println!(
        "{} tables asked for, {skipped} of them not in the image (`-`): the loader skips those",
        costs.len()
    );
    println!();
    Ok(costs)
}

fn walk_table(image: &Path, name: &'static str) -> Result<TableCost, String> {
    let started = Instant::now();
    let read = Rc::new(Cell::new(0u64));
    let file = File::open(image).map_err(|error| format!("open {}: {error}", image.display()))?;
    let counted = Counting {
        inner: file,
        read: Rc::clone(&read),
    };
    let mut db =
        MdfDatabase::from_read(Box::new(counted)).map_err(|error| format!("{name}: {error}"))?;
    let to_open = read.get();
    let staged = db
        .column_names(name)
        .is_some_and(|columns| !columns.is_empty());
    let rows = if staged {
        Some(count_rows(&mut db, name)?)
    } else {
        None
    };
    Ok(TableCost {
        name,
        to_open,
        for_the_rows: read.get() - to_open,
        rows,
        millis: millis_since(started),
    })
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

/// The rows the loader staged against the rows the walk read, table by table.
fn compare(walked: &[TableCost], staged: &Connection) -> Result<(), String> {
    let mut differences = Vec::new();
    for cost in walked {
        let staged_rows = staged_rows(staged, cost.name)?;
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

/// The rows of `table` in the staged database, or `None` when it was not
/// staged.
fn staged_rows(staged: &Connection, table: &str) -> Result<Option<usize>, String> {
    let held: i64 = staged
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = ?1",
            [table],
            |row| row.get(0),
        )
        .map_err(|error| format!("{table}: {error}"))?;
    if held == 0 {
        return Ok(None);
    }
    let rows: i64 = staged
        .query_row(&format!("SELECT COUNT(*) FROM \"{table}\""), [], |row| {
            row.get(0)
        })
        .map_err(|error| format!("{table}: {error}"))?;
    usize::try_from(rows)
        .map(Some)
        .map_err(|error| format!("{table}: {error}"))
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
