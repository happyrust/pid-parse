//! What each `igBoundary2d` decodes as, one line a record, so that two
//! builds can be held against each other.
//!
//! A change to the boundary grammar is safe when every record the family
//! decoded before decodes to the same thing after. This probe prints, for
//! every `igBoundary2d` the library decodes on the sheet chains of the
//! documents it is given, the record's place and a hash of its every field
//! — the `Debug` text of the decoded record, and the bit pattern of every
//! coordinate beside it, since `Debug` prints a double's shortest text and
//! two builds must agree on the bits. The lines are sorted, so two outputs
//! compare with any line diff: a line only in the first is a record that
//! changed or went, a line only in the second one that changed or came.
//!
//! ```text
//! cargo run --example probe_what_each_boundary_decodes_as > before.txt   # on the old build
//! cargo run --example probe_what_each_boundary_decodes_as > after.txt    # on the new one
//! cargo run --example probe_what_each_boundary_decodes_as -- --members > members.txt
//! ```
//!
//! `--members` prints a line for each member instead — the segments of a
//! record that share a tag offset — with the member's tag, whether a conic
//! member is a circle's or an ellipse's, and a hash of the bits of its
//! vertices: a change meant for one kind of member can then be held to
//! having moved no other.
//!
//! With no directory it reads what
//! `probe_what_curve_each_boundary_member_relists` reads: `test-file` and,
//! where this machine has them, the package and acceptance corpora and the
//! share — each directory's `.pid` and `.igr` files and the `.igr` / `.pid`
//! members of its `RefData~*.zip` and `PlantData~*.zip`, each distinct file
//! once. The count goes to stderr.
//!
//! 2026-11-12, when the family learnt a loop of curves that names its
//! members: 111 distinct documents, 1 703 records before and 1 743 after;
//! all 1 703 lines the same, the 40 new ones the logo fills of `A0-W` to
//! `A3-W`, `.igr` and `.pid`.
//!
//! The same day, when an arc of an ellipse took the vertices its own record
//! is drawn as instead of chords of a quarter radian: of 5 969 members the
//! 5 937 that are no ellipse's arc the same to the bit — 5 513 lines, 13
//! point runs, the circle, all 378 arcs of a circle, 32 B-splines — and the
//! 32 that are, changed (a stripe's from one chord to eight segments, a
//! cap's from eight chords to 24); by record, 1 711 the same and 32
//! changed, all of them among the 40 that were new that day.

use pid_parse::parsers::sheet_records::decode_igboundaries;
use std::collections::BTreeSet;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};

/// A file's length and a hash of its bytes: what tells two copies of one
/// document from two documents.
fn fingerprint(bytes: &[u8]) -> (usize, u64) {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    bytes.hash(&mut hasher);
    (bytes.len(), hasher.finish())
}

/// The `.pid` / `.igr` files under `dir` and the archives whose members are
/// read beside them.
fn documents_under(dir: &Path, files: &mut Vec<PathBuf>, archives: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut paths: Vec<PathBuf> = entries.filter_map(Result::ok).map(|e| e.path()).collect();
    paths.sort();
    for path in paths {
        if path.is_dir() {
            // A machine-local symbol library a checkout may link in.
            if path.file_name().is_some_and(|name| name == "symbols-full") {
                continue;
            }
            documents_under(&path, files, archives);
        } else if path
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("pid") || e.eq_ignore_ascii_case("igr"))
        {
            files.push(path);
        } else if path.file_name().and_then(|n| n.to_str()).is_some_and(|n| {
            (n.starts_with("RefData~") || n.starts_with("PlantData~")) && n.ends_with(".zip")
        }) {
            archives.push(path);
        }
    }
}

/// What kind of member opens at `tag_offset` of a boundary's payload: its
/// tag byte, and for a conic whether it is a circle's — the major semi-axis
/// `(radius, 0)`, the ratio one — or an ellipse's.
fn member_kind(payload: &[u8], tag_offset: usize) -> String {
    let double = |at: usize| {
        payload
            .get(at..at + 8)
            .and_then(|bytes| bytes.try_into().ok())
            .map(f64::from_le_bytes)
    };
    match payload.get(tag_offset).copied() {
        // An arc: two parameters, the centre, the major semi-axis vector at
        // `+33`, the ratio at `+49`. A whole ellipse: the centre, the vector
        // at `+17`, the ratio at `+33`.
        Some(tag @ (0x74 | 0x73)) => {
            let vector = if tag == 0x74 { 33 } else { 17 };
            let circle = double(tag_offset + vector + 8) == Some(0.0)
                && double(tag_offset + vector + 16) == Some(1.0);
            format!(
                "0x{tag:02X} of {}",
                if circle { "a circle" } else { "an ellipse" }
            )
        }
        Some(tag) => format!("0x{tag:02X}"),
        None => "no tag".to_string(),
    }
}

/// One line for each boundary the library decodes on the document's sheet
/// chains — or, with `by_member`, one for each member of each: the segments
/// that share a tag offset.
fn lines_of(label: &str, bytes: &[u8], by_member: bool, lines: &mut Vec<String>) {
    let Ok(package) = pid_parse::PidPackage::from_bytes(bytes) else {
        lines.push(format!("{label}|DOES NOT PARSE"));
        return;
    };
    let name = Path::new(label)
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let (len, hash) = fingerprint(bytes);
    for (stream, raw) in &package.streams {
        if !stream
            .rsplit('/')
            .next()
            .is_some_and(|leaf| leaf.starts_with("Sheet"))
        {
            continue;
        }
        for record in decode_igboundaries(raw.data.as_slice()) {
            if by_member {
                let payload = raw
                    .data
                    .get(record.byte_range.start + 6..record.byte_range.end)
                    .unwrap_or_default();
                let mut at = 0usize;
                while at < record.segments.len() {
                    let tag_offset = record.segments[at].tag_offset;
                    let run = record.segments[at..]
                        .iter()
                        .take_while(|segment| segment.tag_offset == tag_offset)
                        .count();
                    let mut hasher = std::collections::hash_map::DefaultHasher::new();
                    for segment in &record.segments[at..at + run] {
                        for value in [
                            segment.start.0,
                            segment.start.1,
                            segment.end.0,
                            segment.end.1,
                        ] {
                            value.to_bits().hash(&mut hasher);
                        }
                    }
                    lines.push(format!(
                        "{name}#{len:x}{hash:016x}|{stream}|+{}|oid {} member +{tag_offset}|{run} \
                         segment(s)|{}|{:016x}",
                        record.byte_range.start,
                        record.oid,
                        member_kind(payload, tag_offset),
                        hasher.finish()
                    ));
                    at += run;
                }
                continue;
            }
            let mut hasher = std::collections::hash_map::DefaultHasher::new();
            format!("{record:?}").hash(&mut hasher);
            for segment in &record.segments {
                for value in [
                    segment.start.0,
                    segment.start.1,
                    segment.end.0,
                    segment.end.1,
                ] {
                    value.to_bits().hash(&mut hasher);
                }
            }
            record.anchor.0.to_bits().hash(&mut hasher);
            record.anchor.1.to_bits().hash(&mut hasher);
            // The document by name and by its own bytes: two files of one
            // name are two documents.
            lines.push(format!(
                "{name}#{len:x}{hash:016x}|{stream}|+{}|oid {}|{} segment(s)|loops {:?}|{} \
                 reference(s)|closed {}|{:016x}",
                record.byte_range.start,
                record.oid,
                record.segments.len(),
                record.loop_segment_counts,
                record.member_refs.len(),
                record.is_closed_loop(1e-9),
                hasher.finish()
            ));
        }
    }
}

fn main() {
    let mut by_member = false;
    let mut dirs: Vec<PathBuf> = Vec::new();
    for arg in std::env::args().skip(1) {
        if arg == "--members" {
            by_member = true;
        } else {
            dirs.push(PathBuf::from(arg));
        }
    }
    if dirs.is_empty() {
        dirs.push(Path::new(env!("CARGO_MANIFEST_DIR")).join("test-file"));
        if let Some(dir) = std::env::var_os("PID_TEST_DATA_DIR") {
            dirs.push(PathBuf::from(dir));
        }
        dirs.push(PathBuf::from(r"D:\work\plant-code\cad\pid-test-data"));
        dirs.push(PathBuf::from(r"D:\work\cad\pid-test-data"));
        dirs.push(PathBuf::from(r"D:\work\cad\PID自研软件验证"));
    }
    let mut seen: BTreeSet<(usize, u64)> = BTreeSet::new();
    let mut lines = Vec::new();
    let mut documents = 0usize;
    for dir in &dirs {
        if !dir.is_dir() {
            eprintln!("{} is absent; skipped", dir.display());
            continue;
        }
        let mut files = Vec::new();
        let mut archives = Vec::new();
        documents_under(dir, &mut files, &mut archives);
        for path in &files {
            let Ok(bytes) = std::fs::read(path) else {
                continue;
            };
            if !seen.insert(fingerprint(&bytes)) {
                continue;
            }
            documents += 1;
            lines_of(&path.display().to_string(), &bytes, by_member, &mut lines);
        }
        for archive in &archives {
            for extension in ["igr", "pid"] {
                let Ok(members) = pid_parse::refdata_archive::archive_members(archive, extension)
                else {
                    continue;
                };
                for member in members {
                    let Ok(bytes) = pid_parse::refdata_archive::read_archive_file(&member) else {
                        continue;
                    };
                    if !seen.insert(fingerprint(&bytes)) {
                        continue;
                    }
                    documents += 1;
                    lines_of(
                        &member.label().display().to_string(),
                        &bytes,
                        by_member,
                        &mut lines,
                    );
                }
            }
        }
    }
    lines.sort();
    for line in &lines {
        println!("{line}");
    }
    eprintln!(
        "{documents} distinct document(s), {} {}",
        lines.len(),
        if by_member {
            "member(s) of the igBoundary2d the library decodes"
        } else {
            "decoded igBoundary2d"
        }
    );
}
