//! Which unit-bearing SI values an Oracle `exp` dump states: every non-NULL
//! `T_PIPERUN.SP_INSULTHICKSI` / `SP_SLOPESI`, `T_EQUIPMENT.SP_INSULTHICKSI`
//! / `SP_HEIGHTSI` and `T_VESSEL.SP_VOLUMERATINGSI`, with the row's `SP_ID`.
//!
//! ```text
//! cargo run --example probe_which_si_values_a_dump_states -- <Export.dmp>
//! ```

use pid_parse::publish::oracle_exp::ExpTables;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args()
        .nth(1)
        .ok_or("usage: probe_which_si_values_a_dump_states <Export.dmp>")?;
    let tables = ExpTables::read(&std::fs::read(&path)?)?;
    let runs = tables.pipe_runs()?;
    let equipment = tables.equipment_rows()?;
    let vessels = tables.vessel_rows()?;
    println!(
        "{path}: {} runs, {} equipment rows, {} vessel rows",
        runs.len(),
        equipment.len(),
        vessels.len()
    );
    let mut stated = 0;
    for run in &runs {
        for (column, value) in [
            ("T_PIPERUN.SP_INSULTHICKSI", &run.insulation_thickness_si),
            ("T_PIPERUN.SP_SLOPESI", &run.slope_si),
        ] {
            if let Some(value) = value {
                stated += 1;
                println!("{column} {} = {value}", run.id);
            }
        }
    }
    for row in &equipment {
        for (column, value) in [
            ("T_EQUIPMENT.SP_INSULTHICKSI", &row.insulation_thickness_si),
            ("T_EQUIPMENT.SP_HEIGHTSI", &row.height_si),
        ] {
            if let Some(value) = value {
                stated += 1;
                println!("{column} {} = {value}", row.id);
            }
        }
    }
    for row in &vessels {
        if let Some(value) = &row.volume_rating_si {
            stated += 1;
            println!("T_VESSEL.SP_VOLUMERATINGSI {} = {value}", row.id);
        }
    }
    println!("{stated} values stated");
    Ok(())
}
