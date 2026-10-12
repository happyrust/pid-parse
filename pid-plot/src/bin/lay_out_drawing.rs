//! Print symbol coverage for one drawing's paper model.

use std::env;
use std::path::PathBuf;
use std::process::ExitCode;

use pid_plot::model::lay_out_drawing;

fn main() -> ExitCode {
    let mut args = env::args().skip(1);
    let Some(path) = args.next() else {
        eprintln!("usage: lay_out_drawing <drawing.pid> [corpus]");
        return ExitCode::from(2);
    };
    let corpus = args
        .next()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(r"D:\work\cad\PID自研软件验证"));
    match lay_out_drawing(PathBuf::from(&path).as_path(), &corpus) {
        Ok(model) => {
            println!(
                "library {} primitives {} found {}/{} missing {}",
                model.library.join(";"),
                model.primitives.len(),
                model.symbols_found,
                model.symbols_referenced,
                model.missing_symbols.len()
            );
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("{path}: {error}");
            ExitCode::from(1)
        }
    }
}
