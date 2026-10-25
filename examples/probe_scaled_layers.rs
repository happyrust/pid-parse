//! For every scaled (uniform_scale != 1) igSymbol2d in W.pid: print its
//! sheet-layer oid and whether the file's layer table marks it displayed.

use pid_parse::PidParser;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let doc = PidParser::new()
        .parse_file(r"D:\work\cad\PID自研软件验证\I井场 注采阀组工艺及自控流程图.pid")?;
    let geometry = pid_parse::build_normalized_geometry(&doc);
    // map oid -> decoded record for layer + scale
    let mut recs = std::collections::HashMap::new();
    for sheet in &doc.sheet_streams {
        if let Some(g) = &sheet.geometry {
            for r in &g.decoded_igsymbols {
                recs.insert(r.oid, (r.sheet_layer_ref, r.uniform_scale));
            }
        }
    }
    for entity in &geometry.entities {
        let pid_parse::PidGraphicKind::SymbolInstance {
            scale, symbol_path, ..
        } = &entity.kind
        else {
            continue;
        };
        if (scale[0].abs() - 1.0).abs() < 1e-6 {
            continue;
        }
        let name = symbol_path
            .as_deref()
            .and_then(|p| p.rsplit('\\').next())
            .unwrap_or("?");
        let (layer_oid, us) = entity
            .graphic_oid
            .and_then(|o| recs.get(&o).copied())
            .unwrap_or((0, 0.0));
        // resolve layer name via the root document's layer table
        let lname = doc
            .sheet_layers
            .get("/")
            .into_iter()
            .flatten()
            .find(|l| l.oid == layer_oid)
            .map(|l| format!("{} disp={:?}", l.name, l.displayed));
        println!(
            "oid={:?} {name:38} layer_oid={layer_oid} layer={lname:?} scale={us:.4} source_layer={:?}",
            entity.graphic_oid, entity.source_layer
        );
    }
    Ok(())
}
