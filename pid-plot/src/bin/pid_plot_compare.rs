//! Compare a reference plot with our plot and write a report plus an overlay.

use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::ExitCode;

use pid_plot::compare::{compare_plots, template_masks, write_overlay};
use pid_plot::read::read_plot;

fn main() -> ExitCode {
    let mut args = env::args().skip(1);
    let (Some(reference_path), Some(our_path), Some(out_dir)) =
        (args.next(), args.next(), args.next())
    else {
        eprintln!("usage: pid_plot_compare <reference.pdf> <ours.pdf> <out-dir>");
        return ExitCode::from(2);
    };
    let reference_bytes = match fs::read(&reference_path) {
        Ok(bytes) => bytes,
        Err(error) => {
            eprintln!("read {reference_path}: {error}");
            return ExitCode::from(1);
        }
    };
    let our_bytes = match fs::read(&our_path) {
        Ok(bytes) => bytes,
        Err(error) => {
            eprintln!("read {our_path}: {error}");
            return ExitCode::from(1);
        }
    };
    let reference = match read_plot(&reference_bytes) {
        Ok(ink) => ink,
        Err(error) => {
            eprintln!("{reference_path}: {error}");
            return ExitCode::from(1);
        }
    };
    let ours = match read_plot(&our_bytes) {
        Ok(ink) => ink,
        Err(error) => {
            eprintln!("{our_path}: {error}");
            return ExitCode::from(1);
        }
    };
    let mut masks = template_masks(reference.page_mm);
    if let Some(mask_json) = args.next() {
        match fs::read_to_string(&mask_json) {
            Ok(text) => match serde_json::from_str::<Vec<pid_plot::model::MissingSymbolBox>>(&text)
            {
                Ok(boxes) => {
                    for box_ in boxes {
                        masks.push(pid_plot::compare::MaskRect {
                            label: box_.path,
                            min_mm: box_.min_mm,
                            max_mm: box_.max_mm,
                        });
                    }
                }
                Err(error) => {
                    eprintln!("masks {mask_json}: {error}");
                    return ExitCode::from(1);
                }
            },
            Err(error) => {
                eprintln!("read {mask_json}: {error}");
                return ExitCode::from(1);
            }
        }
    }
    let report = compare_plots(&reference, &ours, &masks);
    let stem = PathBuf::from(&our_path)
        .file_stem()
        .map(|stem| stem.to_string_lossy().into_owned())
        .unwrap_or_else(|| "compare".to_string());
    let out = PathBuf::from(out_dir);
    if let Err(error) = fs::create_dir_all(&out) {
        eprintln!("mkdir {}: {error}", out.display());
        return ExitCode::from(1);
    }
    let json_path = out.join(format!("{stem}.json"));
    let png_path = out.join(format!("{stem}.png"));
    let json = match serde_json::to_string_pretty(&report) {
        Ok(json) => json,
        Err(error) => {
            eprintln!("json: {error}");
            return ExitCode::from(1);
        }
    };
    if let Err(error) = fs::write(&json_path, json) {
        eprintln!("write {}: {error}", json_path.display());
        return ExitCode::from(1);
    }
    if let Err(error) = write_overlay(&png_path, &reference, &ours, &report.fit) {
        eprintln!("overlay {}: {error}", png_path.display());
        return ExitCode::from(1);
    }
    println!(
        "coverage {:.4} extra_mm {:.1} text {}/{} fit_ok {} {}",
        report.coverage,
        report.extra_length_mm,
        report.text_matched,
        report.text_reference,
        report.fit.within_threshold,
        report.fit.finding.unwrap_or_default()
    );
    ExitCode::SUCCESS
}
