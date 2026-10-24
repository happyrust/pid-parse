//! Inspect the Christmas Tree 02 cached body (site 37311, sheet 1015):
//! does it carry igDimension records, and what do they measure?

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = r"D:\work\cad\PID自研软件验证\I井场 注采阀组工艺及自控流程图.pid";
    let doc = pid_parse::PidParser::new().parse_file(path)?;
    let geometry = pid_parse::build_normalized_geometry(&doc);
    let def = geometry
        .symbol_definitions
        .iter()
        .find(|d| d.reference.site == 37311 && d.reference.sheet == 1015);
    let Some(def) = def else {
        eprintln!("def not found");
        return Ok(());
    };
    println!("def primitives={}", def.primitives.len());
    // dimensions live in the JSite nested geometry, not the definition
    for site in &doc.jsites {
        let has_dim = site
            .nested_geometry
            .as_ref()
            .is_some_and(|n| !n.dimensions.is_empty() || !n.boundaries.is_empty());
        let has_sym = site.symbol_information.is_some();
        if has_dim || has_sym {
            println!(
                "site={} path={:?} sym_info={} boundaries={} dims={}",
                site.name,
                site.symbol_path,
                has_sym,
                site.nested_geometry
                    .as_ref()
                    .map_or(0, |n| n.boundaries.len()),
                site.nested_geometry
                    .as_ref()
                    .map_or(0, |n| n.dimensions.len()),
            );
        }
    }
    Ok(())
}
