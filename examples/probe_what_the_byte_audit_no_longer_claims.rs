//! Which bytes of a `Sheet*` stream `--byte-audit` stopped claiming when the
//! record families began to decode on the stream's own record chain, and
//! what those bytes are.
//!
//! The byte audit marks a byte of a sheet stream consumed when a record
//! family decodes a record over it. Four families used to be found by
//! sliding a decoder over the stream a byte at a time, and decoded "records"
//! that start where the chain starts none: `SubRecord0x0010`,
//! `AttributeFragment`, `JStyleOverride`, and thirteen `igSmartFrame2d`
//! (`docs/analysis/2026-11-10-what-the-sixty-unreferenced-pages-leave-undrawn.md`
//! §3). Those hits claimed bytes. Most of them lie inside a chain record
//! that its own family decodes, and are claimed as before; the rest were
//! claimed by nothing else, and the audit's coverage of the stream fell by
//! exactly them.
//!
//! This probe measures that rest. For every sheet stream the audit runs the
//! families on — the top-level `/Sheet*` of a drawing or a template — it
//! repeats the old walk for the four families through the same public
//! decoders (try every offset, skip what a hit claims), takes the bytes
//! those hits covered, and removes every byte today's audit still marks
//! consumed. What is left is filed under the chain record it lies in: that
//! record's type code, and whether the native reader skips it.
//!
//! ```text
//! cargo run -q --example probe_what_the_byte_audit_no_longer_claims
//! cargo run -q --example probe_what_the_byte_audit_no_longer_claims -- <dir or file> ...
//! ```
//!
//! With no argument it reads the committed drawings and templates under
//! `test-file`, the shelled-out plant backup and the acceptance corpus where
//! this machine has them, and prints one block per corpus.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use pid_parse::parsers::sheet_records::{
    parse_psm_header, IgSmartFrame2dDecoder, PsmRecordDecoder, SheetChain,
};
#[path = "../tests/support/retired_matchers.rs"]
mod retired_matchers;
use pid_parse::{byte_audit_report, PidParser};
use retired_matchers::{AttributeFragmentDecoder, JStyleOverrideDecoder, SubRecord0x0010Decoder};

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
            ["pid", "igr"]
                .iter()
                .any(|known| e.eq_ignore_ascii_case(known))
        }) {
            out.push(path);
        }
    }
}

fn default_roots() -> Vec<(String, PathBuf)> {
    let mut roots = vec![("test-file".to_string(), PathBuf::from("test-file"))];
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
        roots.push(("the plant backup".to_string(), backup));
    }
    let acceptance = std::env::var_os("PID_ACCEPTANCE_CORPUS").map_or_else(
        || PathBuf::from(r"D:\work\cad\pid-test-data\验收数据"),
        PathBuf::from,
    );
    if acceptance.is_dir() {
        roots.push(("the acceptance corpus".to_string(), acceptance));
    }
    roots
}

/// Mark the bytes the old sliding scan of one family claimed: try every
/// offset, and after a hit skip what it claimed. This is the walk
/// `PsmRecordDecoder::scan` made before it read the chain.
fn mark_sliding_hits<D: PsmRecordDecoder>(decoder: &D, data: &[u8], claimed: &mut [bool]) {
    let min_len = decoder.min_record_len().max(1);
    if data.len() < min_len {
        return;
    }
    let max_offset = data.len() - min_len;
    let mut at = 0usize;
    while at <= max_offset {
        match decoder.decode_at(data, at) {
            Some(record) => {
                let advance = decoder.advance_of(&record).max(1);
                let end = (at + advance).min(data.len());
                claimed[at..end].fill(true);
                at += advance;
            }
            None => at += 1,
        }
    }
}

/// What the bytes nothing claims any more lie in.
#[derive(Default)]
struct Reading {
    documents: usize,
    streams: usize,
    stream_bytes: u64,
    /// Streams that lost at least one byte.
    streams_with_a_loss: usize,
    /// Bytes the old scans claimed that today's audit does not.
    lost: u64,
    /// Bytes today's audit marks consumed.
    claimed_now: u64,
    /// The lost bytes by the chain record they lie in: `(type code, skipped)`.
    by_host: BTreeMap<(u16, bool), (usize, u64)>,
    /// Lost bytes that lie in no chain record (the header, or past a stop).
    outside_the_chain: u64,
    /// The streams that lost most, for the report.
    worst: Vec<(u64, String, u64, u64)>,
}

impl Reading {
    fn stream(&mut self, label: &str, path: &str, data: &[u8], consumed: &[(u64, u64)]) {
        self.streams += 1;
        self.stream_bytes += data.len() as u64;
        let mut now = vec![false; data.len()];
        for &(start, end) in consumed {
            let start = usize::try_from(start).unwrap_or(usize::MAX).min(data.len());
            let end = usize::try_from(end).unwrap_or(usize::MAX).min(data.len());
            if start < end {
                now[start..end].fill(true);
            }
        }
        let mut old = vec![false; data.len()];
        mark_sliding_hits(&SubRecord0x0010Decoder, data, &mut old);
        mark_sliding_hits(&AttributeFragmentDecoder, data, &mut old);
        mark_sliding_hits(&JStyleOverrideDecoder, data, &mut old);
        mark_sliding_hits(&IgSmartFrame2dDecoder, data, &mut old);

        let claimed_now = now.iter().filter(|byte| **byte).count() as u64;
        self.claimed_now += claimed_now;
        let lost: Vec<usize> = (0..data.len()).filter(|&at| old[at] && !now[at]).collect();
        if lost.is_empty() {
            return;
        }
        self.streams_with_a_loss += 1;
        self.lost += lost.len() as u64;
        self.worst.push((
            lost.len() as u64,
            format!("{label} {path}"),
            claimed_now,
            data.len() as u64,
        ));

        // The chain record each lost byte lies in.
        let chain = SheetChain::walk(data);
        let mut records: Vec<(usize, usize, u16, bool)> = Vec::new();
        for &at in &chain.starts {
            if let Some(header) = parse_psm_header(data, at) {
                let end = header.body_start + header.bytes_to_follow as usize;
                records.push((at, end, header.type_code, header.native_reader_skips()));
            }
        }
        let mut seen_hosts: BTreeMap<usize, u64> = BTreeMap::new();
        for at in lost {
            let host = records.partition_point(|&(start, _, _, _)| start <= at);
            match host.checked_sub(1).map(|index| records[index]) {
                Some((start, end, _, _)) if at < end => *seen_hosts.entry(start).or_default() += 1,
                _ => self.outside_the_chain += 1,
            }
        }
        for (start, bytes) in seen_hosts {
            let &(_, _, code, skipped) = records
                .iter()
                .find(|record| record.0 == start)
                .expect("a host found above");
            let entry = self.by_host.entry((code, skipped)).or_default();
            entry.0 += 1;
            entry.1 += bytes;
        }
    }

    fn print(&mut self, name: &str) {
        println!(
            "{name}: {} document(s), {} sheet stream(s), {} bytes",
            self.documents, self.streams, self.stream_bytes
        );
        let before = self.claimed_now + self.lost;
        let percent = |bytes: u64| 100.0 * bytes as f64 / self.stream_bytes.max(1) as f64;
        println!(
            "   the audit marks {} bytes consumed ({:.2}%); with the four sliding scans it \
             marked {} ({:.2}%): {} bytes fewer, on {} stream(s)",
            self.claimed_now,
            percent(self.claimed_now),
            before,
            percent(before),
            self.lost,
            self.streams_with_a_loss
        );
        println!("   the bytes no longer claimed lie in:");
        for (&(code, skipped), &(records, bytes)) in &self.by_host {
            println!(
                "      {bytes:>7} byte(s) in {records:>5} record(s) of type 0x{code:04X}{}",
                if skipped {
                    ", superseded copies the native reader skips (0x8000 in the type word)"
                } else {
                    ", live records no family decodes"
                }
            );
        }
        if self.outside_the_chain > 0 {
            println!(
                "      {:>7} byte(s) in no chain record",
                self.outside_the_chain
            );
        }
        self.worst.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
        for (lost, stream, now, len) in self.worst.iter().take(6) {
            let before = now + lost;
            println!(
                "      {stream}: {:.1}% -> {:.1}% ({lost} of {len} bytes)",
                100.0 * before as f64 / *len as f64,
                100.0 * *now as f64 / *len as f64
            );
        }
    }
}

fn read(root: &Path) -> Reading {
    let mut files = Vec::new();
    collect(root, &mut files);
    let mut reading = Reading::default();
    for file in &files {
        let Ok(package) = PidParser::new().parse_package(file) else {
            continue;
        };
        reading.documents += 1;
        let report = byte_audit_report(&package);
        let label = file
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default();
        for (path, raw) in &package.streams {
            // The audit runs the record families on top-level sheets only.
            let top_level_sheet = path
                .strip_prefix('/')
                .is_some_and(|leaf| !leaf.contains('/') && leaf.starts_with("Sheet"));
            if !top_level_sheet {
                continue;
            }
            let consumed: Vec<(u64, u64)> = report
                .traces
                .iter()
                .filter(|trace| trace.stream_path == *path)
                .flat_map(|trace| trace.consumed_ranges.iter())
                .map(|range| (range.start, range.end))
                .collect();
            reading.stream(&label, path, &raw.data, &consumed);
        }
    }
    reading
}

fn main() {
    let args: Vec<PathBuf> = std::env::args().skip(1).map(PathBuf::from).collect();
    let roots: Vec<(String, PathBuf)> = if args.is_empty() {
        default_roots()
    } else {
        args.into_iter()
            .map(|path| (path.display().to_string(), path))
            .collect()
    };
    let mut total = Reading::default();
    for (name, root) in &roots {
        let mut reading = read(root);
        reading.print(name);
        total.documents += reading.documents;
        total.streams += reading.streams;
        total.stream_bytes += reading.stream_bytes;
        total.streams_with_a_loss += reading.streams_with_a_loss;
        total.lost += reading.lost;
        total.claimed_now += reading.claimed_now;
        total.outside_the_chain += reading.outside_the_chain;
        for (host, (records, bytes)) in reading.by_host {
            let entry = total.by_host.entry(host).or_default();
            entry.0 += records;
            entry.1 += bytes;
        }
        total.worst.extend(reading.worst);
    }
    if roots.len() > 1 {
        total.print("in all");
    }
}
