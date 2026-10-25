//! List undecoded-but-graphic type codes in W's /Sheet6 — the
//! arrow+leader marks are probably one of these dropped families.

use pid_parse::PidParser;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let doc = PidParser::new()
        .parse_file(r"D:\work\cad\PID自研软件验证\I井场 注采阀组工艺及自控流程图.pid")?;
    for sheet in &doc.sheet_streams {
        if sheet.path != "/Sheet6" {
            continue;
        }
        let g = sheet.geometry.as_ref().unwrap();
        eprintln!("=== undecoded graphic type codes ===");
        for u in &g.undecoded_type_codes {
            if u.is_graphic {
                eprintln!(
                    "  type=0x{:04X} count={} name={:?}",
                    u.type_code, u.count, u.rad_class_name
                );
                let _ = u;
            }
        }
        eprintln!("=== refused graphic ===");
        for r in &g.refused_records {
            if r.is_graphic {
                eprintln!(
                    "  type=0x{:04X} count={} name={:?}",
                    r.type_code, r.count, r.rad_class_name
                );
            }
        }
    }
    Ok(())
}
