//! Dump model primitives in a model-mm window to see stamped tick glyphs.

use std::env;
use std::path::PathBuf;
use std::process::ExitCode;

use pid_plot::model::{PaperShape, lay_out_drawing};

fn main() -> ExitCode {
    let mut args = env::args().skip(1);
    let Some(path) = args.next() else {
        eprintln!("usage: probe_ticks <drawing.pid> x_min x_max y_min y_max");
        return ExitCode::from(2);
    };
    let win: Vec<f64> = args
        .by_ref()
        .take(4)
        .filter_map(|a| a.parse().ok())
        .collect();
    if win.len() != 4 {
        eprintln!("need 4 window numbers (model mm)");
        return ExitCode::from(2);
    }
    let corpus = PathBuf::from(r"D:\work\cad\PID自研软件验证");
    let model = match lay_out_drawing(PathBuf::from(&path).as_path(), &corpus) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::from(1);
        }
    };
    let in_win = |x: f64, y: f64| x >= win[0] && x <= win[1] && y >= win[2] && y <= win[3];
    for p in &model.primitives {
        if let PaperShape::Line { start_mm, end_mm } = &p.shape {
            let (sx, sy) = (start_mm[0], start_mm[1]);
            let (ex, ey) = (end_mm[0], end_mm[1]);
            if in_win(sx, sy) || in_win(ex, ey) {
                println!("({sx:9.3},{sy:9.3}) -> ({ex:9.3},{ey:9.3})");
            }
        }
    }
    ExitCode::SUCCESS
}
