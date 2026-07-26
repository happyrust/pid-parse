//! Validate the `igSymbol2d` placement-matrix anchor across every fixture.
//!
//! `probe_igsymbol2d_placement.rs` showed the placement is not at a fixed
//! payload offset: the header between the OID block and the matrix varies
//! in length, which is why the decoder's hard-coded 40 lands mid-field and
//! reads denormal noise. In every record the matrix is preceded by the
//! 4-byte tag `02 00 A7 50`, and is followed by seven doubles whose last
//! element is exactly 1.0 -- an affine 2x3 in homogeneous form.
//!
//! This probe checks three things per record, which is what a decoder rule
//! has to be able to rely on:
//!   1. the tag occurs, and occurs only once, in the payload;
//!   2. the seventh double reads exactly 1.0;
//!   3. the translation pair lands on the sheet (0..1.2).

use std::io::Read;
use std::path::Path;

use cfb::CompoundFile;

const TYPE_IGSYMBOL2D: u16 = 0x00CE;
const PSM_HEADER_LEN: usize = 6;
const MATRIX_TAG: [u8; 4] = [0x02, 0x00, 0xA7, 0x50];
const MATRIX_DOUBLES: usize = 7;

fn read_f64(p: &[u8], at: usize) -> Option<f64> {
    let c = p.get(at..at + 8)?;
    Some(f64::from_le_bytes([
        c[0], c[1], c[2], c[3], c[4], c[5], c[6], c[7],
    ]))
}

fn tag_positions(payload: &[u8]) -> Vec<usize> {
    (0..payload.len().saturating_sub(4))
        .filter(|i| payload[*i..*i + 4] == MATRIX_TAG)
        .collect()
}

fn collect(bytes: &[u8]) -> Vec<(usize, Vec<u8>)> {
    let mut out = Vec::new();
    let mut off = 0usize;
    while off + PSM_HEADER_LEN + 16 <= bytes.len() {
        let type_word = u16::from_le_bytes([bytes[off], bytes[off + 1]]);
        if type_word & 0x3FFF != TYPE_IGSYMBOL2D {
            off += 1;
            continue;
        }
        let btf = u32::from_le_bytes([
            bytes[off + 2],
            bytes[off + 3],
            bytes[off + 4],
            bytes[off + 5],
        ]) as usize;
        if !(113..=200).contains(&btf) || off + PSM_HEADER_LEN + btf > bytes.len() {
            off += 1;
            continue;
        }
        let start = off + PSM_HEADER_LEN;
        out.push((off, bytes[start..start + btf].to_vec()));
        off = start + btf;
    }
    out
}

struct Tally {
    records: usize,
    single_tag: usize,
    homogeneous: usize,
    on_sheet: usize,
}

fn probe(path: &Path, tally: &mut Tally) -> Result<(), Box<dyn std::error::Error>> {
    let mut cfb = CompoundFile::open(std::fs::File::open(path)?)?;
    let mut bytes = Vec::new();
    cfb.open_stream("/Sheet6")?.read_to_end(&mut bytes)?;
    let records = collect(&bytes);
    println!("\n=== {} ({} records) ===", path.display(), records.len());

    for (rec_off, payload) in &records {
        tally.records += 1;
        let tags = tag_positions(payload);
        let mut note = String::new();
        if tags.len() == 1 {
            tally.single_tag += 1;
        } else {
            note.push_str(&format!(" TAGS={tags:?}"));
        }
        let Some(&tag_at) = tags.first() else {
            println!("  @0x{rec_off:06x} len={:3} NO TAG", payload.len());
            continue;
        };
        let base = tag_at + 4;
        let mut d = [0f64; MATRIX_DOUBLES];
        let mut short = false;
        for (i, slot) in d.iter_mut().enumerate() {
            match read_f64(payload, base + i * 8) {
                Some(v) => *slot = v,
                None => {
                    short = true;
                    break;
                }
            }
        }
        if short {
            println!(
                "  @0x{rec_off:06x} len={:3} TRUNCATED at +{base}",
                payload.len()
            );
            continue;
        }
        if d[6] == 1.0 {
            tally.homogeneous += 1;
        } else {
            note.push_str(&format!(" d6={:.6}", d[6]));
        }
        let (x, y) = (d[4], d[5]);
        let on = (0.0..=1.2).contains(&x) && (0.0..=1.2).contains(&y);
        if on {
            tally.on_sheet += 1;
        } else {
            note.push_str(" OFF-SHEET");
        }
        println!(
            "  @0x{:06x} len={:3} tag@+{:<3} m=[{:+.4} {:+.4} {:+.4} {:+.4}] ins=({:.5}, {:.5}) -> ({:7.2}, {:7.2})mm{}",
            rec_off,
            payload.len(),
            tag_at,
            d[0],
            d[1],
            d[2],
            d[3],
            x,
            y,
            x * 1000.0,
            y * 1000.0,
            note
        );
    }
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut tally = Tally {
        records: 0,
        single_tag: 0,
        homogeneous: 0,
        on_sheet: 0,
    };
    for fixture in [
        "test-file/DWG-0201GP06-01.pid",
        "test-file/DWG-0202GP06-01.pid",
        "test-file/工艺管道及仪表流程-1.pid",
        "test-file/D06.pid",
        "test-file/export-test/publish-data/A01/A01.pid",
    ] {
        let path = Path::new(fixture);
        if path.exists() {
            probe(path, &mut tally)?;
        }
    }
    println!(
        "\nTOTAL {} records: exactly-one-tag {}, seventh-double-is-1.0 {}, translation-on-sheet {}",
        tally.records, tally.single_tag, tally.homogeneous, tally.on_sheet
    );
    Ok(())
}
