//! Print the librarian entry bound to the 0x0034 record's oid, plus the
//! librarian entry for its neighbouring styles for context.

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = r"D:\work\cad\PID自研软件验证\I井场 注采阀组工艺及自控流程图.pid";
    let doc = pid_parse::PidParser::new().parse_file(path)?;
    let table = doc.style_tables.get("/").unwrap();
    // name bound to oid 0x2022 (the 0x0034 record) plus oids of its
    // neighbours for sanity.
    for sid in [24u32, 25, 26, 27, 28] {
        eprintln!(
            "style_id {sid}: name={:?} record={:?}",
            table.name_of_style(sid),
            table.get(sid).map(|r| (r.type_code, r.oid))
        );
    }
    // The librarian's own list, when reachable through records.
    for rec in table.records() {
        if let Some(name) = rec.name.as_deref() {
            if name.to_lowercase().contains("dash") || name.to_lowercase().contains("hydra") {
                eprintln!(
                    "  named {} = {name:?} (type {:#06x})",
                    rec.style_id, rec.type_code
                );
            }
        }
    }
    Ok(())
}
