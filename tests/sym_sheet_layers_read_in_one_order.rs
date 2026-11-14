//! A symbol file's layer table reads in one order, and the file says which.
//!
//! A `.sym` keeps a `JSheetLayer` table in the `PSMcluster0` of each of its
//! storages, and [`SymbolGeometry::sheet_layers`] is those tables one after
//! another. The reader used to gather them in a hash map keyed by storage
//! and list the map's values, so the tables came out in whatever order that
//! map's hasher gave them: a file with two storages whose tables differ
//! read one way or the other from one call to the next, and from one
//! process to the next. Nothing drawn depends on the order — a primitive's
//! layer resolves through its own storage's table — but two readings of
//! one file compared unequal and serialized differently.
//!
//! The order is now the file's: storages in path order, the root first,
//! and within a storage the order its `PSMcluster0` lists the layers in.
//! Held here on every symbol of the committed `RefData~4~681.zip`:
//!
//! * every reading of a file is that order, worked out from the file's
//!   own streams rather than by the reader; a file whose storages keep
//!   tables that differ — the only kind whose layers can be listed in more
//!   than one order — is read four times, the rest once;
//! * the test runs itself again as another process, which reads the same
//!   files as often, and the two processes' readings are the same to the
//!   byte.
//!
//! [`SymbolGeometry::sheet_layers`]: pid_parse::symbol_library::SymbolGeometry::sheet_layers
#![cfg(feature = "refdata-archive")]

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::Path;
use std::process::Command;

use pid_parse::parsers::sheet_layers::decode_sheet_layers;
use pid_parse::refdata_archive::{archive_members, read_archive_file};
use pid_parse::symbol_library::read_symbol_geometry_from_bytes;
use pid_parse::PidPackage;

/// The committed symbol library of a backup package.
const ARCHIVE: &str = "test-file/backup-test/DWG-0202GP06-01_p/RefData~4~681.zip";

/// Set in the process this test starts: that run prints what it read and
/// starts no process of its own.
const ASKED_BY_ANOTHER_PROCESS: &str = "PID_PARSE_SYM_LAYER_ORDER_ASKED";

/// What the line a run prints its readings' digest on starts with.
const DIGEST_LINE: &str = "sym layer order digest: ";

/// How many times a file whose layers can be listed in more than one order
/// is read. The rest are read once.
const READS: usize = 4;

/// The symbol files of [`ARCHIVE`]. Exact: an archive that changed moves
/// this and the next.
const SYMBOL_FILES: usize = 616;
/// Of those, the ones that keep a table in more than one storage, not all
/// of them the same table: the files whose layers can be listed in more
/// than one order.
const FILES_WITH_AN_ORDER_TO_GET_WRONG: usize = 40;

/// One layer as the comparison holds it: oid, layer number, name.
type Layer = (u32, u32, String);

/// The storage a stream lives in.
fn storage_of(stream_path: &str) -> &str {
    stream_path.rfind('/').map_or("", |cut| &stream_path[..cut])
}

/// The layer table of each storage of a symbol file, storages in path
/// order: read from the file's streams, not through the symbol reader.
fn tables_by_storage(bytes: &[u8]) -> Vec<Vec<Layer>> {
    let package = PidPackage::from_bytes(bytes).expect("a symbol file is a compound file");
    let mut tables: Vec<(&str, Vec<Layer>)> = package
        .streams
        .iter()
        .filter(|(path, _)| path.rsplit('/').next() == Some("PSMcluster0"))
        .map(|(path, raw)| {
            (
                storage_of(path),
                decode_sheet_layers(&raw.data)
                    .into_iter()
                    .map(|layer| (layer.oid, layer.layer_number, layer.name))
                    .collect(),
            )
        })
        .collect();
    tables.sort_by(|a, b| a.0.cmp(b.0));
    tables.into_iter().map(|(_, table)| table).collect()
}

#[test]
fn a_symbol_files_layers_read_in_the_order_of_its_storages() {
    let members =
        archive_members(Path::new(ARCHIVE), "sym").expect("the committed archive is readable");
    let mut digest = DefaultHasher::new();
    let mut files = 0usize;
    let mut with_an_order = 0usize;
    let mut out_of_order: Vec<String> = Vec::new();
    for member in &members {
        let label = member.label().display().to_string();
        let bytes = read_archive_file(member).unwrap_or_else(|error| panic!("{label}: {error}"));
        let tables = tables_by_storage(&bytes);
        let in_storage_order: Vec<Layer> = tables.iter().flatten().cloned().collect();
        let mut distinct: Vec<&Vec<Layer>> =
            tables.iter().filter(|table| !table.is_empty()).collect();
        distinct.dedup();
        let has_an_order = distinct.len() > 1;
        files += 1;
        if has_an_order {
            with_an_order += 1;
        }
        let reads = if has_an_order { READS } else { 1 };
        let mut wrong = 0usize;
        for _ in 0..reads {
            let body = read_symbol_geometry_from_bytes(&bytes)
                .unwrap_or_else(|error| panic!("{label}: {error}"));
            let read: Vec<Layer> = body
                .sheet_layers
                .iter()
                .map(|layer| (layer.oid, layer.layer_number, layer.name.clone()))
                .collect();
            (&label, &read).hash(&mut digest);
            if read != in_storage_order {
                wrong += 1;
            }
        }
        if wrong > 0 {
            out_of_order.push(format!("{label}: {wrong} of {reads} readings"));
        }
    }
    let digest = format!("{:016x}", digest.finish());

    if std::env::var_os(ASKED_BY_ANOTHER_PROCESS).is_some() {
        println!("{DIGEST_LINE}{digest}");
        return;
    }

    // The same files read by another process: this test, run on its own.
    let other = Command::new(std::env::current_exe().expect("the test's own executable"))
        .args([
            "--exact",
            "a_symbol_files_layers_read_in_the_order_of_its_storages",
            "--nocapture",
            "--test-threads",
            "1",
        ])
        .env(ASKED_BY_ANOTHER_PROCESS, "1")
        .output()
        .expect("the test starts itself as another process");
    let stdout = String::from_utf8_lossy(&other.stdout);
    let theirs = stdout
        .lines()
        .find_map(|line| line.split(DIGEST_LINE).nth(1))
        .unwrap_or_else(|| {
            panic!(
                "the other process printed no digest: {stdout}\n{}",
                String::from_utf8_lossy(&other.stderr)
            )
        })
        .trim()
        .to_string();

    eprintln!(
        "{files} symbol files, {with_an_order} with tables that differ between storages (read \
         {READS} times here and {READS} times by another process; the rest once): {} read out \
         of their storages' order here; digests {digest} and {theirs}",
        out_of_order.len()
    );
    assert!(
        out_of_order.is_empty() && theirs == digest,
        "{} of {files} symbol files read their layers in an order other than their storages' \
         (the first of them: {:#?}); this process's readings digest to {digest} and another \
         process's to {theirs}",
        out_of_order.len(),
        &out_of_order[..out_of_order.len().min(8)]
    );
    assert_eq!(
        (files, with_an_order),
        (SYMBOL_FILES, FILES_WITH_AN_ORDER_TO_GET_WRONG),
        "the symbol files of {ARCHIVE}, and those whose storages keep different tables"
    );
}
