//! Does `igTextBox`'s tail come in two kinds, and does this corpus use both?
//!
//! The native Load (`radsrvitem.dll!sub_56498C00`) branches on `Src[13]`,
//! which is record byte 26 = **payload +20** -- the field the format guide
//! calls the "style-tail tag" and leaves unexplained. The branch is not
//! cosmetic:
//!
//! ```text
//!   if ( Src[13] == 1 )      apply kind 1 to (Src + body_len + 28)
//!   else if ( Src[13] == 2 ) apply kind 2 to (Src + body_len + 28)
//!   else                     return E_INVALIDARG
//! ```
//!
//! `Src + body_len + 28` in bytes is `payload + 22 + body_len`, which is
//! exactly where the shipped decoder starts reading the placement tail. So
//! `+20` selects **how those 36 bytes are meant to be read**, and the decoder
//! implements only one reading.
//!
//! That matters for correctness, not just completeness: the insertion point
//! and the text direction both come out of that tail. If any record in the
//! corpus carries kind 2 and the tail means something else there, those
//! records are being placed or rotated wrong today.
//!
//! This probe answers the only question that decides whether to act: **does
//! this corpus contain kind 2 at all?**
//!
//! ```powershell
//! cargo run --example probe_igtextbox_tail_kind
//! ```

use std::collections::BTreeMap;
use std::io::Read;
use std::path::{Path, PathBuf};

use pid_parse::parsers::sheet_records::{
    decode_igtextbox_at, sheet_record_starts, PSM_ENVELOPE_LEN, PSM_TYPE_CODE_IGTEXTBOX,
};

/// Payload offset of the tail-kind tag (record byte 26).
const TAIL_KIND_AT: usize = 20;
/// Payload offset of the shape discriminator, for the cross-tab.
const SUB_TYPE_AT: usize = 18;

const FIXTURES: &[&str] = &[
    "test-file/DWG-0201GP06-01.pid",
    "test-file/DWG-0202GP06-01.pid",
    "test-file/\u{5DE5}\u{827A}\u{7BA1}\u{9053}\u{53CA}\u{4EEA}\u{8868}\u{6D41}\u{7A0B}-1.pid",
    "test-file/D06.pid",
    "test-file/export-test/publish-data/A01/A01.pid",
    "test-file/export-test/publish-data/DWG-0202GP06-01/DWG-0202GP06-01.pid",
];

fn u16_at(d: &[u8], at: usize) -> Option<u16> {
    let s = d.get(at..at + 2)?;
    Some(u16::from_le_bytes([s[0], s[1]]))
}

fn u32_at(d: &[u8], at: usize) -> Option<u32> {
    let s = d.get(at..at + 4)?;
    Some(u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
}

fn sheet_streams(path: &Path) -> Vec<Vec<u8>> {
    let Ok(file) = std::fs::File::open(path) else {
        return Vec::new();
    };
    let Ok(mut cfb) = cfb::CompoundFile::open(file) else {
        return Vec::new();
    };
    let names: Vec<PathBuf> = cfb
        .walk()
        .filter(cfb::Entry::is_stream)
        .map(|e| e.path().to_path_buf())
        .filter(|p| p.to_string_lossy().contains("Sheet"))
        .collect();
    let mut out = Vec::new();
    for name in names {
        let Ok(mut stream) = cfb.open_stream(&name) else {
            continue;
        };
        let mut bytes = Vec::new();
        if stream.read_to_end(&mut bytes).is_ok() {
            out.push(bytes);
        }
    }
    out
}

fn main() {
    let mut cross: BTreeMap<(u16, u16), usize> = BTreeMap::new();
    let mut accepted_cross: BTreeMap<(u16, u16), usize> = BTreeMap::new();
    let mut total = 0usize;

    for fixture in FIXTURES {
        let path = Path::new(fixture);
        if !path.exists() {
            continue;
        }
        for bytes in sheet_streams(path) {
            for at in sheet_record_starts(&bytes) {
                let Some(raw) = bytes.get(at..at + 2) else {
                    continue;
                };
                if u16::from_le_bytes([raw[0], raw[1]]) & 0x3FFF != PSM_TYPE_CODE_IGTEXTBOX {
                    continue;
                }
                let Some(b) = bytes.get(at + 2..at + 6) else {
                    continue;
                };
                let btf = u32::from_le_bytes([b[0], b[1], b[2], b[3]]) as usize;
                let Some(payload) = bytes.get(at + PSM_ENVELOPE_LEN..at + PSM_ENVELOPE_LEN + btf)
                else {
                    continue;
                };
                let (Some(sub), Some(kind)) =
                    (u16_at(payload, SUB_TYPE_AT), u16_at(payload, TAIL_KIND_AT))
                else {
                    continue;
                };
                total += 1;
                *cross.entry((sub, kind)).or_default() += 1;
                if decode_igtextbox_at(&bytes, at).is_some() {
                    *accepted_cross.entry((sub, kind)).or_default() += 1;
                }
                let _ = u32_at(payload, 0);
            }
        }
    }

    println!("=== igTextBox: shape (+18) x tail kind (+20) ===");
    println!("chain records: {total}\n");
    println!("{:>6} {:>10} {:>10} {:>10}", "shape", "tail kind", "records", "accepted");
    for ((sub, kind), count) in &cross {
        println!(
            "{sub:>6} {kind:>10} {count:>10} {:>10}",
            accepted_cross.get(&(*sub, *kind)).copied().unwrap_or(0)
        );
    }

    let kinds: std::collections::BTreeSet<u16> = cross.keys().map(|(_, k)| *k).collect();
    println!("\n=== verdict ===");
    println!("tail kinds present: {kinds:?}");
    if kinds.len() == 1 {
        println!(
            "One kind only. The shipped decoder's single tail reading covers every\n\
             record this corpus has, so nothing is being misplaced today -- but the\n\
             other kind exists in the format and a drawing that used it would be\n\
             read wrong, silently. Worth a guard rather than a decoder."
        );
    } else {
        println!(
            "Both kinds present. The shipped decoder reads one tail layout for all\n\
             of them, so the records of the other kind may be placed or rotated\n\
             wrong right now. This needs the kind-2 tail layout read out of the\n\
             native code before anything else."
        );
    }
}
