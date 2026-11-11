//! What three sheet families were decoding while no sheet chain held a
//! record of theirs, and where records of their two type codes really live.
//!
//! `SubRecord0x0010`, `AttributeFragment` (both type code `0x0010`) and
//! `JStyleOverride` (`0x0030`) used to be found by sliding a decoder over a
//! `Sheet*` stream byte by byte. Since the families read the stream's own
//! record chain they decode nothing, on any sheet of any corpus
//! (`probe_which_decoded_records_start_off_the_chain` reads zero off-chain
//! decodes and zero records of the three). This probe answers what that
//! leaves open, by measurement:
//!
//! 1. **Where the type codes live.** Every stream that opens with the
//!    cluster magic is walked as a record chain, whatever it is called, and
//!    chain records of `0x0030` and `0x0010` are tallied by the kind of
//!    stream that holds them, with their payload lengths.
//! 2. **What the old hits were.** The three `decode_*_at` entry points still
//!    read at any offset, so the old walk — try every byte, skip what a hit
//!    claims — is repeated here on every sheet stream, and each hit that
//!    does not start a chain record is filed under the record it sits in:
//!    that host's type word and how far into its payload the hit starts.
//! 3. **Whether the host already reads those bytes.** For an attribute
//!    fragment inside a text box, whether the text it extracted is the text
//!    the box's own decoder gives; for a `0x0010` hit on a host's `+12`,
//!    whether the length it claimed is the host's own style id at `+14`; for
//!    a `JStyleOverride` hit, whether it starts on the length word of the
//!    block that closes a `DependencyObject` and runs past that record's
//!    end.
//!
//! ```text
//! cargo run -q --example probe_what_the_chainless_families_matched
//! cargo run -q --example probe_what_the_chainless_families_matched -- <dir or file> ...
//! ```
//!
//! With no argument it reads the committed `test-file` tree (drawings,
//! templates and `.sym` bodies; a machine-local `symbols-full` is left out),
//! the shelled-out plant backup and the acceptance corpus where this machine
//! has them. Exits non-zero if a sheet chain holds a record of either type
//! code, or if a hit is found that its host does not account for in one of
//! the ways above — which is what would make one of the three families a
//! family again.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use pid_parse::parsers::sheet_records::{decode_igtextbox_at, sheet_record_starts};
#[path = "../tests/support/retired_matchers.rs"]
mod retired_matchers;
use pid_parse::PidParser;
use retired_matchers::{
    decode_attribute_fragment_at, decode_jstyle_override_at, decode_sub_record_0x0010_at,
};

/// The word every cluster-family stream opens with.
const CLUSTER_MAGIC: u32 = 0x6C90_F544;
/// `JStyleOverride`.
const OVERRIDE: u16 = 0x0030;
/// The code the two `0x0010` families were filed under.
const SUB_RECORD: u16 = 0x0010;
/// `DependencyObject`, `igTextBox`.
const DEPENDENCY: u16 = 0x00FA;
const TEXT_BOX: u16 = 0x004D;

fn collect(dir: &Path, out: &mut Vec<PathBuf>) {
    if dir.is_file() {
        out.push(dir.to_path_buf());
        return;
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut paths: Vec<PathBuf> = entries.filter_map(Result::ok).map(|e| e.path()).collect();
    paths.sort();
    for path in paths {
        if path.is_dir() {
            // A machine-local symbol library, and this crate's own plots.
            if path
                .file_name()
                .is_some_and(|name| name == "symbols-full" || name == "plot")
            {
                continue;
            }
            collect(&path, out);
        } else if path.extension().is_some_and(|e| {
            ["pid", "igr", "sym"]
                .iter()
                .any(|known| e.eq_ignore_ascii_case(known))
        }) {
            out.push(path);
        }
    }
}

fn default_roots() -> Vec<PathBuf> {
    let mut roots = vec![PathBuf::from("test-file")];
    let data = std::env::var_os("PID_TEST_DATA_DIR").map_or_else(
        || {
            [
                r"D:\work\cad\pid-test-data",
                r"D:\work\plant-code\cad\pid-test-data",
            ]
            .iter()
            .map(PathBuf::from)
            .collect::<Vec<_>>()
        },
        |dir| vec![PathBuf::from(dir)],
    );
    if let Some(backup) = data
        .iter()
        .map(|root| root.join("backup-plant-data"))
        .find(|dir| dir.is_dir())
    {
        roots.push(backup);
    }
    let acceptance = std::env::var_os("PID_ACCEPTANCE_CORPUS").map_or_else(
        || PathBuf::from(r"D:\work\cad\pid-test-data\验收数据"),
        PathBuf::from,
    );
    if acceptance.is_dir() {
        roots.push(acceptance);
    }
    roots
}

fn u16_at(data: &[u8], at: usize) -> Option<u16> {
    data.get(at..at + 2)
        .map(|b| u16::from_le_bytes([b[0], b[1]]))
}

fn u32_at(data: &[u8], at: usize) -> Option<u32> {
    data.get(at..at + 4)
        .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
}

/// The kind of stream a path names: its storage (`root` or `JSite`) and its
/// leaf with any trailing number dropped.
fn stream_kind(path: &str) -> String {
    let parts: Vec<&str> = path.trim_start_matches('/').split('/').collect();
    let strip = |name: &str| {
        name.trim_end_matches(|c: char| c.is_ascii_digit())
            .to_string()
    };
    let leaf = strip(parts.last().copied().unwrap_or_default());
    if parts.len() > 1 {
        format!("{}/{leaf}", strip(parts[0]))
    } else {
        format!("root/{leaf}")
    }
}

/// Every hit of `decode_at` over `data`, walked the way the sliding scan
/// walked: try each byte, and after a hit skip what it claimed.
fn sliding_hits<R>(
    data: &[u8],
    decode_at: impl Fn(&[u8], usize) -> Option<R>,
    claimed: impl Fn(&R) -> usize,
) -> Vec<(usize, R)> {
    let mut hits = Vec::new();
    let mut at = 0usize;
    while at + 6 <= data.len() {
        match decode_at(data, at) {
            Some(record) => {
                let advance = claimed(&record).max(1);
                hits.push((at, record));
                at += advance;
            }
            None => at += 1,
        }
    }
    hits
}

/// A tally of hits by `(host type word, offset into the host's payload)`.
#[derive(Default)]
struct Hosts {
    by_host: BTreeMap<u16, usize>,
    by_offset: BTreeMap<usize, usize>,
    total: usize,
    past_the_host: usize,
}

impl Hosts {
    fn add(&mut self, host_word: u16, offset: usize, overruns: bool) {
        *self.by_host.entry(host_word).or_default() += 1;
        *self.by_offset.entry(offset).or_default() += 1;
        self.total += 1;
        if overruns {
            self.past_the_host += 1;
        }
    }

    fn print(&self, name: &str) {
        println!(
            "{name}: {} hit(s) off the chain, {} of them claiming bytes past the end of the \
             record they sit in",
            self.total, self.past_the_host
        );
        let hosts = self
            .by_host
            .iter()
            .map(|(word, count)| format!("0x{word:04X} x{count}"))
            .collect::<Vec<_>>()
            .join(", ");
        println!("   inside a chain record of type word: {hosts}");
        let mut offsets: Vec<(&usize, &usize)> = self.by_offset.iter().collect();
        offsets.sort_by(|a, b| b.1.cmp(a.1));
        let offsets = offsets
            .iter()
            .map(|(offset, count)| format!("+{offset} x{count}"))
            .collect::<Vec<_>>()
            .join(", ");
        println!("   bytes into that record's payload: {offsets}");
    }
}

#[allow(clippy::too_many_lines)]
fn main() {
    let mut roots: Vec<PathBuf> = std::env::args().skip(1).map(PathBuf::from).collect();
    if roots.is_empty() {
        roots = default_roots();
    }
    let mut files = Vec::new();
    for root in &roots {
        collect(root, &mut files);
    }

    // 1. where the two type codes live
    let mut chains: BTreeMap<String, (usize, usize)> = BTreeMap::new();
    let mut homes: BTreeMap<(u16, String), BTreeMap<usize, usize>> = BTreeMap::new();
    // 2. and 3. what the old hits were
    let mut fragments = Hosts::default();
    let mut sub_records = Hosts::default();
    let mut overrides = Hosts::default();
    let (mut in_boxes, mut box_text, mut not_text) = (0usize, 0usize, Vec::new());
    let (mut on_style_word, mut style_is_length) = (0usize, 0usize);
    let (mut on_block_length, mut override_strays) = (0usize, Vec::new());
    let mut unexplained: Vec<String> = Vec::new();
    let mut documents = 0usize;
    let mut sheets = 0usize;

    for path in &files {
        let Ok(package) = PidParser::new().parse_package(path) else {
            continue;
        };
        documents += 1;
        let name = path
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_default();
        for (stream, raw) in &package.streams {
            let data = raw.data.as_slice();
            if u32_at(data, 0) != Some(CLUSTER_MAGIC) {
                continue;
            }
            let kind = stream_kind(stream);
            // The chain, walked on its own terms.
            let mut records: BTreeMap<usize, (u16, usize)> = BTreeMap::new();
            let mut at = 8usize;
            let mut exact = true;
            while at + 6 <= data.len() {
                let (Some(word), Some(length)) = (u16_at(data, at), u32_at(data, at + 2)) else {
                    break;
                };
                let end = at + 6 + length as usize;
                if length == 0 || end > data.len() {
                    exact = false;
                    break;
                }
                records.insert(at, (word, end));
                at = end;
            }
            exact &= at == data.len();
            let row = chains.entry(kind.clone()).or_default();
            row.0 += 1;
            if !exact {
                row.1 += 1;
            }
            for (start, (word, end)) in &records {
                let code = word & 0x3FFF;
                if code == OVERRIDE || code == SUB_RECORD {
                    *homes
                        .entry((code, kind.clone()))
                        .or_default()
                        .entry(end - start - 6)
                        .or_default() += 1;
                }
            }

            let is_sheet = stream
                .rsplit('/')
                .next()
                .is_some_and(|leaf| leaf.starts_with("Sheet"));
            if !is_sheet {
                continue;
            }
            sheets += 1;
            // The parser's own chain must be the one walked above.
            let starts = sheet_record_starts(data);
            if exact && starts.len() != records.len() {
                unexplained.push(format!(
                    "{name} {stream}: the parser walks {} chain records, this probe {}",
                    starts.len(),
                    records.len()
                ));
            }
            let host_of = |at: usize| {
                records
                    .range(..=at)
                    .next_back()
                    .map(|(s, (w, e))| (*s, *w, *e))
            };

            for (at, fragment) in
                sliding_hits(data, decode_attribute_fragment_at, |r| r.byte_range.len())
            {
                if records.contains_key(&at) {
                    unexplained.push(format!("{name} {stream} +{at}: a fragment on the chain"));
                    continue;
                }
                let Some((host, word, end)) = host_of(at) else {
                    unexplained.push(format!("{name} {stream} +{at}: a fragment in no record"));
                    continue;
                };
                let offset = at - host - 6;
                fragments.add(word, offset, fragment.byte_range.end > end);
                let texts: Vec<&str> = fragment.strings.iter().map(|s| s.text.as_str()).collect();
                if word & 0x3FFF == TEXT_BOX {
                    in_boxes += 1;
                    // A box the native reader skips is still a box to the old
                    // walk; read it with the skip bit cleared.
                    let mut live = data.to_vec();
                    live[host + 1] &= 0x3F;
                    let own = decode_igtextbox_at(&live, host).map(|text_box| text_box.text);
                    if own.as_deref().is_some_and(|text| texts.contains(&text)) {
                        box_text += 1;
                    } else {
                        unexplained.push(format!(
                            "{name} {stream} +{at}: fragment {texts:?} is not its text box's {own:?}"
                        ));
                    }
                } else {
                    not_text.push(format!(
                        "{name} {stream} +{at}: inside 0x{word:04X} at payload +{offset}: {texts:?}"
                    ));
                }
            }

            for (at, record) in
                sliding_hits(data, decode_sub_record_0x0010_at, |r| r.byte_range.len())
            {
                if records.contains_key(&at) {
                    unexplained.push(format!("{name} {stream} +{at}: a 0x0010 on the chain"));
                    continue;
                }
                let Some((host, word, end)) = host_of(at) else {
                    unexplained.push(format!("{name} {stream} +{at}: a 0x0010 in no record"));
                    continue;
                };
                let offset = at - host - 6;
                sub_records.add(word, offset, record.byte_range.end > end);
                if offset == 12 {
                    on_style_word += 1;
                    // The host's `+12` is its sub-type word and `+14` its
                    // style id: the hit's "type code" and "length".
                    if u32_at(data, at + 2) == u32_at(data, host + 6 + 14) {
                        style_is_length += 1;
                    }
                }
            }

            for (at, record) in
                sliding_hits(data, decode_jstyle_override_at, |r| r.byte_range.len())
            {
                if records.contains_key(&at) {
                    unexplained.push(format!(
                        "{name} {stream} +{at}: a JStyleOverride on the chain"
                    ));
                    continue;
                }
                let Some((host, word, end)) = host_of(at) else {
                    unexplained.push(format!(
                        "{name} {stream} +{at}: a JStyleOverride in no record"
                    ));
                    continue;
                };
                overrides.add(word, at - host - 6, record.byte_range.end > end);
                // `<type u16><selector u16><len u16>` then `len` bytes to
                // the record's end: the hit starts on `len`, which is 48.
                let length = usize::from(u16_at(data, at).unwrap_or_default());
                if word & 0x3FFF == DEPENDENCY && at + 2 + length == end {
                    on_block_length += 1;
                } else {
                    override_strays.push(format!(
                        "{name} {stream} +{at}: inside 0x{word:04X} at payload +{}, not on a closing \
                         block's length word",
                        at - host - 6
                    ));
                }
            }
        }
    }

    println!(
        "{documents} document(s) under {:?}; {sheets} sheet stream(s)",
        roots
            .iter()
            .map(|root| root.display().to_string())
            .collect::<Vec<_>>()
    );
    println!(
        "streams that open with the cluster magic (those that do not walk to their last byte):"
    );
    for (kind, (count, broken)) in &chains {
        println!("   {kind}: {count} ({broken})");
    }
    for code in [OVERRIDE, SUB_RECORD] {
        let total: usize = homes
            .iter()
            .filter(|((c, _), _)| *c == code)
            .map(|(_, lengths)| lengths.values().sum::<usize>())
            .sum();
        println!("type code 0x{code:04X}: {total} chain record(s)");
        for ((c, kind), lengths) in &homes {
            if *c != code {
                continue;
            }
            let lengths = lengths
                .iter()
                .map(|(length, count)| format!("{length} bytes x{count}"))
                .collect::<Vec<_>>()
                .join(", ");
            println!("   {kind}: payloads of {lengths}");
        }
    }
    let on_a_sheet: usize = homes
        .iter()
        .filter(|((_, kind), _)| kind.ends_with("/Sheet"))
        .map(|(_, lengths)| lengths.values().sum::<usize>())
        .sum();
    println!("   on sheet chains: {on_a_sheet}");

    println!();
    fragments.print("AttributeFragment");
    println!(
        "   inside a text box: {in_boxes}; the text it extracted is the text the box's own \
         decoder gives: {box_text}"
    );
    println!("   elsewhere: {}", not_text.len());
    for line in &not_text {
        println!("      {line}");
    }
    sub_records.print("SubRecord0x0010");
    println!(
        "   on a host's +12 (its sub-type word): {on_style_word}; the length it claimed is the \
         host's style id at +14: {style_is_length}"
    );
    overrides.print("JStyleOverride");
    println!(
        "   on the length word of the block that closes a DependencyObject: {on_block_length} of {}",
        overrides.total
    );
    for line in override_strays.iter().take(10) {
        println!("      {line}");
    }

    let failed = on_a_sheet > 0
        || !unexplained.is_empty()
        || !override_strays.is_empty()
        || overrides.past_the_host != overrides.total
        || style_is_length != on_style_word;
    for line in unexplained.iter().take(20) {
        println!("unexplained: {line}");
    }
    if failed {
        println!("NOT ALL ACCOUNTED FOR");
        std::process::exit(1);
    }
    println!("every hit is accounted for by the record it sits in; no sheet chain holds either type code");
}
