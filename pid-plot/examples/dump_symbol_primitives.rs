//! Every paper primitive one symbol instance draws, so a residual reported
//! against a symbol can be read against the shape that produced it.
//!
//! ```text
//! cargo run -p pid-plot --example dump_symbol_primitives -- "<drawing name>" "<symbol basename>"
//! ```
//!
//! Read-only.

use std::path::PathBuf;

use pid_plot::model::{PaperShape, lay_out_drawing};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let name = args
        .next()
        .expect("usage: <drawing name> <symbol basename>");
    let wanted = args
        .next()
        .expect("usage: <drawing name> <symbol basename>");
    let corpus = std::env::var_os("PID_PARSE_SELF_CHECK_DIR").map_or_else(
        || PathBuf::from(r"D:\work\cad\PID自研软件验证"),
        PathBuf::from,
    );
    let model = lay_out_drawing(&corpus.join(format!("{name}.pid")), &corpus)?;
    for primitive in &model.primitives {
        let Some(symbol) = &primitive.symbol else {
            continue;
        };
        let base = symbol.rsplit(['\\', '/']).next().unwrap_or(symbol);
        if wanted != "*" && base != wanted {
            continue;
        }
        let pen = primitive
            .stroke
            .as_ref()
            .map_or_else(String::new, |stroke| {
                format!(
                    " colour {:?} width {:.3} dash {:?}",
                    stroke.rgb, stroke.width_mm, stroke.dash_mm
                )
            });
        println!(
            "{:14} {} {}{}",
            primitive.confidence,
            base,
            primitive.stream_path.as_deref().unwrap_or("-"),
            pen
        );
        print!("{}", render(&primitive.shape));
    }
    Ok(())
}

fn render(shape: &PaperShape) -> String {
    match shape {
        PaperShape::Line { start_mm, end_mm } => format!(
            "  line ({:9.3},{:9.3}) -> ({:9.3},{:9.3})\n",
            start_mm[0], start_mm[1], end_mm[0], end_mm[1]
        ),
        PaperShape::Arc {
            center_mm,
            radius_mm,
            start_angle,
            end_angle,
            clockwise,
        } => format!(
            "  arc  center ({:9.3},{:9.3}) r {:8.3} {:8.4} -> {:8.4} cw={clockwise}\n",
            center_mm[0], center_mm[1], radius_mm, start_angle, end_angle
        ),
        PaperShape::Circle {
            center_mm,
            radius_mm,
        } => format!(
            "  circ center ({:9.3},{:9.3}) r {:8.3}\n",
            center_mm[0], center_mm[1], radius_mm
        ),
        PaperShape::Fill { chain_mm, rgb } => {
            let mut out = format!("  fill {} vertices rgb {:?}\n", chain_mm.len(), rgb);
            for point in chain_mm {
                out.push_str(&format!("       ({:9.3},{:9.3})\n", point[0], point[1]));
            }
            out
        }
        PaperShape::Polyline { points_mm, closed } => {
            let mut out = format!("  poly {} vertices closed {closed}\n", points_mm.len());
            for point in points_mm {
                out.push_str(&format!("       ({:9.3},{:9.3})\n", point[0], point[1]));
            }
            out
        }
        PaperShape::Text {
            origin_mm,
            height_mm,
            value,
            ..
        } => format!(
            "  text ({:9.3},{:9.3}) height {:?} {value:?}\n",
            origin_mm[0], origin_mm[1], height_mm
        ),
    }
}
