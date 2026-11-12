//! The plant's whole symbol library, for a test that reads a symbol the
//! committed sample under `test-file/symbols` does not hold.
//!
//! The library is committed, packed. `RefData~4~681.zip` of the backup
//! package `test-file/backup-test/DWG-0202GP06-01_p` holds 616 `.sym`
//! members, and the archive of the same name in `TEST02_p` holds two more
//! that the first does not (`sewage pool.sym`, `Cone Roof Parametric
//! Tank.sym`). Read in that order — a name both hold is the first's — the
//! two are the 618 files of `test-file/symbols-full`, byte for byte: that
//! directory is the same library unpacked on a machine that has done so, and
//! it is gitignored (`examples/probe_is_the_unpacked_symbol_library_what_the_archives_hold.rs`
//! compares them). So a test that reads a member out of the archives reads
//! what it used to read off the directory, on every checkout rather than on
//! the machines that unpacked one.
//!
//! A build without the `refdata-archive` feature opens no archive and reads
//! the directory. Where a symbol can be had from neither, the caller is
//! given `None` after one line that starts `skipping: ` — the way every soft
//! skip of this crate's tests is spelled, so that a run with `--nocapture`
//! can be searched for what it did not check.
//!
//! A test includes this file with
//! `#[path = "support/library_symbols.rs"] mod library_symbols;`.

#![allow(dead_code)]

use std::path::{Path, PathBuf};

/// The archives that hold the library, in the order they are searched,
/// relative to the crate root.
pub const LIBRARY_ARCHIVES: [&str; 2] = [
    "test-file/backup-test/DWG-0202GP06-01_p/RefData~4~681.zip",
    "test-file/backup-test/TEST02_p/RefData~4~681.zip",
];

/// The same library unpacked, where a machine has it; relative to the crate
/// root.
pub const UNPACKED_LIBRARY: &str = "test-file/symbols-full";

/// How many `.sym` files the library holds.
pub const LIBRARY_SYMBOLS: usize = 618;

fn crate_root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}

/// [`LIBRARY_ARCHIVES`] as paths, for a `SymbolLibrary::with_archives`.
pub fn library_archives() -> Vec<PathBuf> {
    LIBRARY_ARCHIVES
        .iter()
        .map(|archive| crate_root().join(archive))
        .collect()
}

/// [`UNPACKED_LIBRARY`] as a path.
pub fn unpacked_library() -> PathBuf {
    crate_root().join(UNPACKED_LIBRARY)
}

/// What is said of the archives when a symbol is in none of them, and when
/// none of them could be read at all.
#[cfg(feature = "refdata-archive")]
const NOT_ARCHIVED: (&str, &str) = (
    "the committed archives hold no such member",
    "the committed archives are not in this checkout",
);
/// What a build that reads no archive says of them, either way.
#[cfg(not(feature = "refdata-archive"))]
const NOT_ARCHIVED: (&str, &str) = (
    "a build without the `refdata-archive` feature reads no archive",
    "a build without the `refdata-archive` feature reads no archive",
);

/// The bytes of the symbol at `relative` — its path inside the library,
/// `/`-separated, as `Piping/Valves/Angle/2-Way Angle Globe Valve.sym`.
///
/// `None`, after a `skipping: ` line, where neither the archives nor an
/// unpacked library hold it.
pub fn library_symbol(relative: &str) -> Option<Vec<u8>> {
    #[cfg(feature = "refdata-archive")]
    for archive in library_archives() {
        if let Ok(bytes) = pid_parse::refdata_archive::read_archive_member(&archive, relative) {
            return Some(bytes);
        }
    }
    let unpacked = unpacked_library();
    if let Ok(bytes) = std::fs::read(unpacked.join(relative)) {
        return Some(bytes);
    }
    eprintln!(
        "skipping: {relative} is not under {}, and {}",
        unpacked.display(),
        NOT_ARCHIVED.0
    );
    None
}

/// Every `.sym` of the library with its bytes, sorted by its path inside the
/// library.
///
/// `None`, after a `skipping: ` line, where neither the archives nor an
/// unpacked library are in reach.
pub fn every_library_symbol() -> Option<Vec<(String, Vec<u8>)>> {
    let mut symbols: std::collections::BTreeMap<String, Vec<u8>> =
        std::collections::BTreeMap::new();
    #[cfg(feature = "refdata-archive")]
    for archive in library_archives() {
        let Ok(members) = pid_parse::refdata_archive::archive_members(&archive, "sym") else {
            continue;
        };
        for member in members {
            if symbols.contains_key(member.name()) {
                continue;
            }
            let bytes = pid_parse::refdata_archive::read_archive_file(&member)
                .unwrap_or_else(|error| panic!("{}: {error}", member.label().display()));
            symbols.insert(member.name().to_owned(), bytes);
        }
    }
    let unpacked = unpacked_library();
    if symbols.is_empty() {
        collect_unpacked(&unpacked, &unpacked, &mut symbols);
    }
    if symbols.is_empty() {
        eprintln!(
            "skipping: no symbol library: {} holds no `.sym`, and {}",
            unpacked.display(),
            NOT_ARCHIVED.1
        );
        return None;
    }
    Some(symbols.into_iter().collect())
}

/// The `.sym` files under `dir`, keyed by their path below `root`.
fn collect_unpacked(
    root: &Path,
    dir: &Path,
    out: &mut std::collections::BTreeMap<String, Vec<u8>>,
) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_unpacked(root, &path, out);
        } else if path
            .extension()
            .is_some_and(|extension| extension.eq_ignore_ascii_case("sym"))
        {
            let Ok(relative) = path.strip_prefix(root) else {
                continue;
            };
            let bytes =
                std::fs::read(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display()));
            out.insert(relative.to_string_lossy().replace('\\', "/"), bytes);
        }
    }
}
