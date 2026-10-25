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
        if let PidGraphicKind::SymbolInstance {
            insertion,
            symbol_path,
            ..
        } = &e.kind
        {
            // near the two label positions (0.415,0.502)/(0.415,0.513) or app draw (0.461,0.542)
            if (insertion.x - 0.415).abs() < 0.02 && (insertion.y - 0.507).abs() < 0.02
                || (insertion.x - 0.461).abs() < 0.02 && (insertion.y - 0.542).abs() < 0.02
            {
                println!(
                    "{} @{:.4},{:.4} {:?}",
                    e.id,
                    insertion.x,
                    insertion.y,
                    symbol_path.as_deref().and_then(|p| p.rsplit('\\').next())
                );
            }
        }
    }
    // also: do these textboxes have any grouping/owner fields?
}
