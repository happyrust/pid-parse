//! Where do the newly-decoded igSymbol2d placements land, and do their
//! bodies resolve (embedded definition cache or library path)?
use pid_parse::geometry::{build_normalized_geometry, PidGraphicKind};
use pid_parse::{ParseOptions, PidParser};

const FILES: &[&str] = &[
    r"D:\work\cad\PID自研软件验证\I井场 注采阀组工艺及自控流程图.pid",
    r"D:\work\cad\PID自研软件验证\集注站 凝析油闪蒸单元 一级闪蒸分离器B工艺及自控流程图.pid",
    r"D:\work\cad\PID自研软件验证\集注站 烃水露点控制单元（一） 低温分离器A工艺及自控流程图.pid",
];

fn main() {
    for file in FILES {
        let parsed = PidParser::with_options(ParseOptions::geometry())
            .parse_file(file)
            .expect("parse");
        let geo = build_normalized_geometry(&parsed);
        println!("\n========== {file}");
        println!("  symbol_definitions={}", geo.symbol_definitions.len());
        for e in &geo.entities {
            if let PidGraphicKind::SymbolInstance {
                insertion,
                symbol_path,
                rotation,
                scale,
                definition,
            } = &e.kind
            {
                let has_def = definition
                    .and_then(|d| geo.symbol_definition(d))
                    .map(|d| d.primitives.len());
                println!(
                    "  {:?} @{:.3},{:.3} rot={:.2} scale={:?} path={:?} def_prims={:?} conf={:?}",
                    e.id,
                    insertion.x,
                    insertion.y,
                    rotation,
                    scale,
                    symbol_path,
                    has_def,
                    e.confidence
                );
            }
        }
    }
}
