//! The symbol instance sitting nearest one paper point, with the body
//! primitives it draws, so a stroke that lands mirrored or shifted on the
//! page can be read back to the record that produced it.
//!
//! ```text
//! cargo run -p pid-parse --example probe_body_at -- "<drawing.pid>" <x_mm> <y_mm>
//! ```

use pid_parse::{PidGraphicKind, PidParser};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let path = args.next().expect("usage: <drawing.pid> <x_mm> <y_mm>");
    let x: f64 = args.next().expect("x").parse()?;
    let y: f64 = args.next().expect("y").parse()?;
    let doc = PidParser::new().parse_file(std::path::Path::new(&path))?;
    let geometry = pid_parse::build_normalized_geometry(&doc);
    let mut ranked: Vec<(f64, &pid_parse::PidGraphicEntity)> = Vec::new();
    for entity in &geometry.entities {
        let PidGraphicKind::SymbolInstance { insertion, .. } = &entity.kind else {
            continue;
        };
        let at = [insertion.x * 1000.0, insertion.y * 1000.0];
        let distance = (at[0] - x).hypot(at[1] - y);
        ranked.push((distance, entity));
    }
    ranked.sort_by(|left, right| left.0.total_cmp(&right.0));
    let Some((distance, entity)) = ranked.first().copied() else {
        println!("no symbol instance");
        return Ok(());
    };
    for (other_distance, other) in ranked.iter().take(3) {
        if let PidGraphicKind::SymbolInstance {
            insertion,
            symbol_path,
            definition,
            ..
        } = &other.kind
        {
            println!(
                "  candidate {other_distance:.2} mm: {:?} at ({:.2},{:.2}) definition {definition:?}",
                symbol_path,
                insertion.x * 1000.0,
                insertion.y * 1000.0
            );
            if let Some(body) =
                definition.and_then(|reference| geometry.symbol_definition(reference))
            {
                for (index, (primitive, _, _)) in body.visible_styled().enumerate() {
                    println!("    [{index}] {primitive:?}");
                }
            }
        }
    }
    let PidGraphicKind::SymbolInstance {
        insertion,
        rotation,
        scale,
        definition,
        symbol_path,
    } = &entity.kind
    else {
        unreachable!()
    };
    println!(
        "{:?} at ({:.2},{:.2}) rot {rotation:.4} scale {scale:?} definition {definition:?} distance {distance:.2}",
        symbol_path,
        insertion.x * 1000.0,
        insertion.y * 1000.0
    );
    let Some(definition) = definition.and_then(|reference| geometry.symbol_definition(reference))
    else {
        println!("  no definition");
        return Ok(());
    };
    for (index, (primitive, _, lettering)) in definition.visible_styled().enumerate() {
        println!("  [{index}] {primitive:?} lettering {lettering:?}");
    }
    Ok(())
}
