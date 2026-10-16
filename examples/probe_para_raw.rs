//! Print every JStyleTextPara's style id and its raw horizontal-alignment
//! byte (+35) plus vertical byte (+36), for the real drawing — to check
//! whether the "MPa" para (id 32) really states Center or another enum.

use pid_parse::{ParseOptions, PidParser};

fn main() {
    let file =
        r"D:\work\cad\PID自研软件验证\集注站 凝析油闪蒸单元 一级闪蒸分离器A工艺及自控流程图.pid";
    let parsed = PidParser::with_options(ParseOptions::geometry())
        .parse_file(file)
        .unwrap();
    for (storage, table) in &parsed.style_tables {
        println!("== {storage}");
        for rec in table.records() {
            if rec.type_code == 0x002D {
                // JStyleTextPara: dump bytes 32..40
                println!(
                    "  id={} align={:?} spacing={:?} name={:?}",
                    rec.style_id, rec.text_alignment, rec.line_spacing, rec.name
                );
            }
        }
    }
}
