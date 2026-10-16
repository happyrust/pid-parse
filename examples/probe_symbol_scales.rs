//! Scan all igSymbol2d records in W.pid: read the 7th double after the
//! matrix tag (the uniform scale) and list every record where it != 1.0.

use cfb::CompoundFile;
use std::io::Read;

const PSM_HEADER_LEN: usize = 6;
const MATRIX_TAG: [u8; 4] = [0x02, 0x00, 0xA7, 0x50];

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = r"D:\work\cad\PID自研软件验证\I井场 注采阀组工艺及自控流程图.pid";
    let doc = pid_parse::PidParser::new().parse_file(path)?;
    let geometry = pid_parse::build_normalized_geometry(&doc);
    let path_of = |oid: u32| -> Option<String> {
        geometry.entities.iter().find_map(|e| {
            if e.graphic_oid != Some(oid) {
                return None;
            }
            if let pid_parse::PidGraphicKind::SymbolInstance { symbol_path, .. } = &e.kind {
                symbol_path.clone()
            } else {
                None
            }
        })
    };
    let mut cfb = CompoundFile::open(std::fs::File::open(path)?)?;
    let mut bytes = Vec::new();
    cfb.open_stream("/Sheet6")?.read_to_end(&mut bytes)?;
    let mut off = 0usize;
    let mut n = 0usize;
    let mut scaled = 0usize;
    while off + PSM_HEADER_LEN + 16 <= bytes.len() {
        let type_word = u16::from_le_bytes([bytes[off], bytes[off + 1]]);
        if type_word & 0x3FFF != 0x00CE {
            off += 1;
            continue;
        }
        let btf = u32::from_le_bytes(bytes[off + 2..off + 6].try_into()?) as usize;
        if !(113..=400).contains(&btf) || off + PSM_HEADER_LEN + btf > bytes.len() {
            off += 1;
            continue;
        }
        let p = &bytes[off + PSM_HEADER_LEN..off + PSM_HEADER_LEN + btf];
        let oid = u32::from_le_bytes(p[0..4].try_into()?);
        let Some(tag_at) = p
            .get(..64usize.min(p.len()))
            .and_then(|head| head.windows(4).position(|w| w == MATRIX_TAG))
        else {
            off += PSM_HEADER_LEN + btf;
            continue;
        };
        let m_at = tag_at + 4;
        let d = |i: usize| -> Option<f64> {
            p.get(m_at + i * 8..m_at + i * 8 + 8)
                .map(|c| f64::from_le_bytes(c.try_into().unwrap()))
        };
        let (Some(m0), Some(m1), Some(m2), Some(m3), Some(ix), Some(iy), Some(sc)) =
            (d(0), d(1), d(2), d(3), d(4), d(5), d(6))
        else {
            off += PSM_HEADER_LEN + btf;
            continue;
        };
        n += 1;
        if (sc - 1.0).abs() > 1e-9 {
            scaled += 1;
            let path = path_of(oid);
            println!(
                "oid={oid} m=[{m0:.3},{m1:.3},{m2:.3},{m3:.3}] ins=({ix:.4},{iy:.4}) scale={sc:.6} path={path:?}"
            );
        }
        off += PSM_HEADER_LEN + btf;
    }
    println!("records={n} with_scale_ne_1={scaled}");
    Ok(())
}
