//! Is `test-file/symbols-full` what the committed `RefData~4~681.zip`
//! archives hold?
//!
//! The directory is the plant's symbol library unpacked — gitignored, so a
//! machine has it only if somebody unpacked it there — and until 2026-11-12
//! eleven tests of this crate read their symbols off it and skipped, most of
//! them without a word, where it was absent. They now read the two archives
//! the library is committed in (`tests/support/library_symbols.rs`): the
//! one of `DWG-0202GP06-01_p` first, the one of `TEST02_p` for a name the
//! first does not hold. That is the same reading only if the directory is
//! those archives unpacked in that order, which is what this probe checks,
//! file by file and byte by byte, on a machine that has the directory.
//!
//! It prints, for each archive, how many `.sym` members it holds and how
//! many of them the library takes from it; for the two archives, the names
//! both hold and how many of those differ; and for the directory, the names
//! only it or only the archives hold and the files whose bytes differ. It
//! exits non-zero when the directory and the archives disagree, and says
//! that it compared nothing where the directory is absent.
//!
//! ```powershell
//! cargo run --quiet --example probe_is_the_unpacked_symbol_library_what_the_archives_hold
//! ```
//!
//! On 2026-11-12: 616 members in the first archive, 605 in the second, 603
//! names in both of which 12 differ; the library is 616 + 2 = 618 symbols;
//! the directory holds 618 files, the same names, every one identical.

#[path = "../tests/support/library_symbols.rs"]
mod library_symbols;

#[cfg(not(feature = "refdata-archive"))]
fn main() {
    eprintln!("this probe reads archives: build it with the `refdata-archive` feature");
    std::process::exit(2);
}

#[cfg(feature = "refdata-archive")]
fn main() {
    use pid_parse::refdata_archive::{archive_members, read_archive_file};
    use std::collections::BTreeMap;

    // Each archive's own members.
    let mut per_archive: Vec<BTreeMap<String, Vec<u8>>> = Vec::new();
    for archive in library_symbols::library_archives() {
        let members = match archive_members(&archive, "sym") {
            Ok(members) => members,
            Err(error) => {
                eprintln!("{}: {error}", archive.display());
                std::process::exit(2);
            }
        };
        let mut held = BTreeMap::new();
        for member in &members {
            match read_archive_file(member) {
                Ok(bytes) => {
                    held.insert(member.name().to_owned(), bytes);
                }
                Err(error) => {
                    eprintln!("{}: {error}", member.label().display());
                    std::process::exit(2);
                }
            }
        }
        per_archive.push(held);
    }

    // The library: a name is the first archive's that holds it.
    let mut library: BTreeMap<&str, (usize, &[u8])> = BTreeMap::new();
    for (index, held) in per_archive.iter().enumerate() {
        for (name, bytes) in held {
            library.entry(name).or_insert((index, bytes));
        }
    }
    for (index, archive) in library_symbols::LIBRARY_ARCHIVES.iter().enumerate() {
        let taken = library.values().filter(|(from, _)| *from == index).count();
        println!(
            "{archive}: {} `.sym` members, {taken} of them the library's",
            per_archive[index].len()
        );
    }
    if let [first, second] = per_archive.as_slice() {
        let shared: Vec<&String> = first
            .keys()
            .filter(|name| second.contains_key(*name))
            .collect();
        let differing: Vec<&&String> = shared
            .iter()
            .filter(|name| first[**name] != second[**name])
            .collect();
        println!(
            "names both archives hold: {}, of which {} differ (the first archive's is the library's)",
            shared.len(),
            differing.len()
        );
        for name in &differing {
            println!("    differs: {name}");
        }
        for name in second.keys().filter(|name| !first.contains_key(*name)) {
            println!("    only in the second: {name}");
        }
    }
    println!("the library: {} symbols", library.len());
    let mut wrong = library.len() != library_symbols::LIBRARY_SYMBOLS;
    if wrong {
        println!(
            "    the tests take it for {}",
            library_symbols::LIBRARY_SYMBOLS
        );
    }

    // The directory, where there is one.
    let unpacked = library_symbols::unpacked_library();
    if !unpacked.is_dir() {
        println!(
            "{} is absent: nothing to compare the library with",
            unpacked.display()
        );
        std::process::exit(i32::from(wrong));
    }
    let mut files: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    collect(&unpacked, &unpacked, &mut files);
    println!("{}: {} `.sym` files", unpacked.display(), files.len());
    let only_unpacked: Vec<&String> = files
        .keys()
        .filter(|name| !library.contains_key(name.as_str()))
        .collect();
    let only_archived: Vec<&&str> = library
        .keys()
        .filter(|name| !files.contains_key(**name))
        .collect();
    let differing: Vec<&String> = files
        .iter()
        .filter(|(name, bytes)| {
            library
                .get(name.as_str())
                .is_some_and(|(_, archived)| *archived != bytes.as_slice())
        })
        .map(|(name, _)| name)
        .collect();
    println!(
        "only in the directory: {}; only in the archives: {}; in both with different bytes: {}",
        only_unpacked.len(),
        only_archived.len(),
        differing.len()
    );
    for name in &only_unpacked {
        println!("    only in the directory: {name}");
    }
    for name in &only_archived {
        println!("    only in the archives: {name}");
    }
    for name in &differing {
        println!("    differs: {name}");
    }
    wrong |= !(only_unpacked.is_empty() && only_archived.is_empty() && differing.is_empty());
    println!(
        "{}",
        if wrong {
            "the directory is NOT the archives unpacked"
        } else {
            "the directory is the archives unpacked, byte for byte"
        }
    );
    std::process::exit(i32::from(wrong));
}

/// The `.sym` files under `dir`, keyed by their `/`-separated path below
/// `root`.
#[cfg(feature = "refdata-archive")]
fn collect(
    root: &std::path::Path,
    dir: &std::path::Path,
    out: &mut std::collections::BTreeMap<String, Vec<u8>>,
) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect(root, &path, out);
        } else if path
            .extension()
            .is_some_and(|extension| extension.eq_ignore_ascii_case("sym"))
        {
            let Ok(relative) = path.strip_prefix(root) else {
                continue;
            };
            match std::fs::read(&path) {
                Ok(bytes) => {
                    out.insert(relative.to_string_lossy().replace('\\', "/"), bytes);
                }
                Err(error) => {
                    eprintln!("{}: {error}", path.display());
                    std::process::exit(2);
                }
            }
        }
    }
}
