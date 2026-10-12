//! Print ink-segment and text-span counts for one rendered plot.

use std::env;
use std::process::ExitCode;

use pid_plot::read::read_plot;

fn main() -> ExitCode {
    let mut args = env::args().skip(1);
    let Some(path) = args.next() else {
        eprintln!("usage: read_reference_plot <plot.pdf>");
        return ExitCode::from(2);
    };
    let bytes = match std::fs::read(&path) {
        Ok(bytes) => bytes,
        Err(error) => {
            eprintln!("read {path}: {error}");
            return ExitCode::from(1);
        }
    };
    match read_plot(&bytes) {
        Ok(ink) => {
            if args.next().as_deref() == Some("--spans") {
                for span in &ink.spans {
                    println!("{}", span.text.replace('\n', "\\n"));
                }
            }
            println!(
                "page_mm {:.2} {:.2} segments {} spans {}",
                ink.page_mm[0],
                ink.page_mm[1],
                ink.segments.len(),
                ink.spans.len()
            );
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("{path}: {error}");
            ExitCode::from(1)
        }
    }
}
