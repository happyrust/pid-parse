//! Dump JSite37311's symbol-information chain and dimensions.

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = r"D:\work\cad\PID自研软件验证\I井场 注采阀组工艺及自控流程图.pid";
    let doc = pid_parse::PidParser::new().parse_file(path)?;
    for site in &doc.jsites {
        if site.name != "JSite37311" {
            continue;
        }
        let Some(info) = &site.symbol_information else {
            continue;
        };
        println!("== symbol_informations ==");
        for s in &info.symbol_informations {
            println!(
                "  oid={} parent={} extents=({:.6},{:.6}) vars={:?}",
                s.oid,
                s.parent_ref,
                s.extents.0,
                s.extents.1,
                s.variables
                    .iter()
                    .map(|v| format!("{}={}@{}", v.name, v.value, v.value_ref))
                    .collect::<Vec<_>>()
            );
        }
        println!("== double_values ==");
        for v in &info.double_values {
            println!("  oid={} parent={} value={}", v.oid, v.parent_ref, v.value);
        }
        println!("== variable_groups ==");
        for g in &info.variable_groups {
            println!("  oid={} members={:?}", g.oid, g.members);
        }
        println!("== relations ==");
        for r in &info.relations {
            println!(
                "  oid={} sig={:?} ops={:?} formula={:?}",
                r.oid, r.signature, r.operands, r.formula
            );
        }
        if let Some(n) = &site.nested_geometry {
            println!("== dimensions ({}) ==", n.dimensions.len());
            for d in &n.dimensions {
                println!("  {:?}", d);
            }
        }
    }
    Ok(())
}
