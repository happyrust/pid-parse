use pid_parse::geometry::{build_normalized_geometry, PidGraphicKind};
use pid_parse::{ParseOptions, PidParser};

fn main() {
    let file =
        r"D:\work\cad\PID自研软件验证\集注站 凝析油闪蒸单元 一级闪蒸分离器A工艺及自控流程图.pid";
    let parsed = PidParser::with_options(ParseOptions::geometry())
        .parse_file(file)
        .unwrap();
    let geo = build_normalized_geometry(&parsed);
    for e in &geo.entities {
        if let PidGraphicKind::Text { .. } = &e.kind {
            let s = format!("{:?}", e.kind);
            if s.contains("HFL-0109101") || s.contains("LFL-0109201") {
                println!("{} conf={:?}", e.id, e.confidence);
                println!("  {s}");
            }
        }
    }
}
