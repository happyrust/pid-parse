//! Every laid-out text primitive that matches a filter, with where its height,
//! rotation and anchor came from.
//!
//! ```text
//! cargo run -p pid-plot --example probe_text_primitives -- \
//!     "<drawing.pid>" "<text>" ["<corpus>"]
//! ```
//!
//! Set `NEAR=x,y` (paper millimetres, y up) to keep only primitives within
//! `RADIUS` millimetres of that point. Read-only.

use std::env;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use pid_plot::model::{PaperShape, lay_out_drawing};

fn main() -> ExitCode {
    let mut args = env::args().skip(1);
    let (Some(pid), Some(wanted)) = (args.next(), args.next()) else {
        eprintln!("usage: probe_text_primitives <drawing.pid> <text> [corpus]");
        return ExitCode::from(2);
    };
    let corpus = args
        .next()
        .map(PathBuf::from)
        .or_else(|| env::var_os("PID_PARSE_SELF_CHECK_DIR").map(PathBuf::from))
        .unwrap_or_else(|| PathBuf::from(r"D:\work\cad\PID自研软件验证"));
    let near: Option<[f64; 2]> = env::var("NEAR").ok().and_then(|value| {
        let (x, y) = value.split_once(',')?;
        Some([x.trim().parse().ok()?, y.trim().parse().ok()?])
    });
    let radius: f64 = env::var("RADIUS")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(20.0);
    let model = match lay_out_drawing(Path::new(&pid), &corpus) {
        Ok(model) => model,
        Err(error) => {
            eprintln!("{pid}: {error}");
            return ExitCode::from(1);
        }
    };
    println!(
        "text\torigin_mm\theight_mm\trot_deg\tanchor\tline\tsymbol\tstream\tbytes\tconfidence"
    );
    for primitive in &model.primitives {
        let PaperShape::Text {
            origin_mm,
            height_mm,
            value,
            rotation,
            anchor,
            line,
            ..
        } = &primitive.shape
        else {
            continue;
        };
        if value.trim() != wanted {
            continue;
        }
        if let Some(near) = near {
            let distance = (origin_mm[0] - near[0]).hypot(origin_mm[1] - near[1]);
            if distance > radius {
                continue;
            }
        }
        println!(
            "{:?}\t[{:.2},{:.2}]\t{}\t{:.1}\t{anchor:?}\t{line:?}\t{}\t{}\t{}\t{}",
            value,
            origin_mm[0],
            origin_mm[1],
            height_mm.map_or("none".to_string(), |height| format!("{height:.2}")),
            rotation.to_degrees(),
            primitive
                .symbol
                .as_deref()
                .map_or("none".to_string(), |symbol| symbol
                    .rsplit(['/', '\\'])
                    .next()
                    .unwrap_or(symbol)
                    .to_string()),
            primitive.stream_path.as_deref().unwrap_or("none"),
            primitive
                .byte_range
                .map_or("none".to_string(), |range| format!(
                    "{}..{}",
                    range[0], range[1]
                )),
            primitive.confidence,
        );
    }
    ExitCode::SUCCESS
}
