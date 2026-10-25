//! The arcs a drawing's definition cache carries, with the angles each one
//! was decoded to, so an arc drawn with the wrong sweep can be traced back to
//! the record before the renderer normalizes it.
//!
//! ```text
//! cargo run -p pid-parse --example probe_definition_arcs -- "<drawing.pid>"
//! ```

use pid_parse::symbol_library::SymbolPrimitive;
use pid_parse::PidParser;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args().nth(1).expect("usage: <drawing.pid>");
    let doc = PidParser::new().parse_file(std::path::Path::new(&path))?;
    let geometry = pid_parse::build_normalized_geometry(&doc);
    for definition in &geometry.symbol_definitions {
        for (index, (primitive, _, _)) in definition.visible_styled().enumerate() {
            let SymbolPrimitive::Arc {
                center,
                radius,
                start_angle,
                end_angle,
                ..
            } = primitive
            else {
                continue;
            };
            let sweep = (end_angle - start_angle).to_degrees();
            println!(
                "{:?} prim {index}: centre ({:.4},{:.4}) r {radius:.4} start {:.2}° end {:.2}° sweep {sweep:.2}°",
                definition.reference,
                center.0,
                center.1,
                start_angle.to_degrees(),
                end_angle.to_degrees()
            );
        }
    }
    Ok(())
}
