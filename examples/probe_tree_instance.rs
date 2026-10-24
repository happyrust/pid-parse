//! List decoded igSymbol2d placements near the wellhead tree in W.pid,
//! with their insertion, rotation and scale, and which cached body they name.

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = r"D:\work\cad\PID自研软件验证\I井场 注采阀组工艺及自控流程图.pid";
    let doc = pid_parse::PidParser::new().parse_file(path)?;
    let geometry = pid_parse::build_normalized_geometry(&doc);
    let mut count = 0usize;
    for entity in &geometry.entities {
        let pid_parse::PidGraphicKind::SymbolInstance {
            insertion,
            rotation,
            scale,
            definition,
            symbol_path,
            ..
        } = &entity.kind
        else {
            continue;
        };
        count += 1;
        if count <= 8 {
            eprintln!(
                "sample ins=({:.4},{:.4}) scale=({:.3},{:.3}) path={:?}",
                insertion.x, insertion.y, scale[0], scale[1], symbol_path
            );
        }
        // Scene mm -> sheet-normalized: A0 landscape 1189x841mm.
        let nx = insertion.x * 841.0;
        let ny = insertion.y * 841.0;
        let interesting = symbol_path.as_deref().is_some_and(|p| {
            p.contains("Christmas") || p.contains("Wellhead") || p.contains("wellhead")
        }) || (125.0..=185.0).contains(&nx) && (125.0..=195.0).contains(&ny);
        if !interesting {
            continue;
        }
        println!(
            "oid={:?} ins_mm=({:.2},{:.2}) rot={:.3} scale=({:.4},{:.4}) def={:?} path={:?}",
            entity.graphic_oid, nx, ny, rotation, scale[0], scale[1], definition, symbol_path
        );
    }
    eprintln!("symbol instances total: {count}");
    Ok(())
}
