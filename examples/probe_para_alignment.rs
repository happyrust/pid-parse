//! Dump every igTextBox's paragraph-style id, its stated horizontal
//! alignment, and the lettering resolution — to see which records state
//! Center vs Left for the "MPa" labels vs the bubble texts.

use pid_parse::style_link::text_styles_for_document;
use pid_parse::{ParseOptions, PidParser};

fn main() {
    let file =
        r"D:\work\cad\PID自研软件验证\集注站 凝析油闪蒸单元 一级闪蒸分离器A工艺及自控流程图.pid";
    let parsed = PidParser::with_options(ParseOptions::geometry())
        .parse_file(file)
        .unwrap();
    let styles = text_styles_for_document(&parsed);
    for sheet in &parsed.sheet_streams {
        let Some(geom) = &sheet.geometry else {
            continue;
        };
        for rec in &geom.decoded_igtextboxes {
            let key = (sheet.path.clone(), rec.oid);
            let resolved = styles.get(&key).and_then(|s| s.effective());
            let x = rec.trailing_double_1 * 1000.0;
            let y = rec.trailing_double_2 * 1000.0;
            if !(x > 340.0 && x < 410.0 && y > 415.0 && y < 445.0) {
                continue;
            }
            let align = resolved.as_ref().and_then(|s| s.alignment);
            let font = resolved.as_ref().and_then(|s| s.font_name.clone());
            println!(
                "oid={} idx={} subtype={} xy=({:.2},{:.2}) align={:?} font={:?} text={:?} runs={:?}",
                rec.oid, rec.index, rec.text_sub_type, x, y, align, font, rec.text, rec.runs
            );
        }
    }
}
