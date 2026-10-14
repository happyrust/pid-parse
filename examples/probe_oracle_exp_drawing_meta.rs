//! Read-only probe for N10 (`_Meta.xml`): inspect `T_DRAWING`,
//! `T_DRAWINGVERSION`, `T_REVISION` (and related tables) in `Export.dmp`,
//! plus the 5 reference `_Meta.xml` files in `D:\work\cad\PID自研软件验证`.
//!
//! Row decoding follows `examples/probe_oracle_exp_pipeline_scan.rs`, with
//! Oracle 7-byte `DATE` cells (type 12) decoded into `YYYY/MM/DD HH:MM:SS`.
//!
//! This probe does not run `imp`, does not connect to Oracle, and does not
//! modify `src/` or `publish/`.
//!
//! ```powershell
//! cargo run --example probe_oracle_exp_drawing_meta
//! ```

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::{Path, PathBuf};

use quick_xml::events::BytesStart;
use quick_xml::Reader;

const DEFAULT_DUMP: &str = r"D:\work\cad\pid-test-data\Export.dmp";
const DEFAULT_REF_DIR: &str = r"D:\work\cad\PID自研软件验证";

const FIVE_STEMS: [&str; 5] = [
    "I井场 注采阀组工艺及自控流程图",
    "集注站 凝析油闪蒸单元 一级闪蒸分离器A工艺及自控流程图",
    "集注站 凝析油闪蒸单元 一级闪蒸分离器B工艺及自控流程图",
    "集注站 烃水露点控制单元（一） 低温分离器A工艺及自控流程图",
    "集注站 烃水露点控制单元（二） 换热器CD及外输阀组工艺及自控流程图",
];

fn main() {
    if let Err(err) = run() {
        eprintln!("probe_oracle_exp_drawing_meta: {err}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let dump_path = args
        .next()
        .map_or_else(|| PathBuf::from(DEFAULT_DUMP), PathBuf::from);
    let ref_dir = args
        .next()
        .map_or_else(|| PathBuf::from(DEFAULT_REF_DIR), PathBuf::from);

    let dump =
        std::fs::read(&dump_path).map_err(|err| format!("read {}: {err}", dump_path.display()))?;
    println!(
        "dump {} bytes ({:.2} MB)",
        dump.len(),
        dump.len() as f64 / 1_048_576.0
    );

    let sites = insert_sites(&dump);
    if sites.is_empty() {
        return Err("no INSERT INTO statements".into());
    }
    let apps = sites
        .iter()
        .find(|(name, _)| name == "APPLICATIONS")
        .ok_or("APPLICATIONS insert not found")?;
    let apps_table = read_table_at(&dump, &apps.0, apps.1)?;
    pin_applications(&apps_table)?;

    // 1. List all tables whose names relate to drawing / version / revision / publish / doc.
    println!("\n=== 1. Relevant Tables in Export.dmp ===");
    let mut tables_by_name: HashMap<String, Table> = HashMap::new();
    for (label, at) in &sites {
        let upper = label.to_ascii_uppercase();
        if upper.contains("DRAWING")
            || upper.contains("VERSION")
            || upper.contains("REVISION")
            || upper.contains("PUBLISH")
            || upper.contains("DOCUMENT")
            || upper == "T_PLANT"
            || upper == "T_PLANTGROUP"
        {
            let table = read_table_at(&dump, label, *at)?;
            let non_empty_cols = table
                .columns
                .iter()
                .enumerate()
                .filter(|(idx, _)| table.rows.iter().any(|r| !matches!(r[*idx], Field::Null)))
                .count();
            println!(
                "table {:<28} rows={:<4} cols={:<3} non_null_cols={}",
                label,
                table.rows.len(),
                table.columns.len(),
                non_empty_cols
            );
            tables_by_name.insert(label.clone(), table);
        }
    }

    // 2. Inspect T_DRAWING in detail.
    let t_drawing = tables_by_name
        .get("T_DRAWING")
        .ok_or("T_DRAWING not found")?;
    println!(
        "\n=== 2. T_DRAWING (rows={}, cols={}) ===",
        t_drawing.rows.len(),
        t_drawing.columns.len()
    );
    for (idx, col) in t_drawing.columns.iter().enumerate() {
        let non_null = t_drawing
            .rows
            .iter()
            .filter(|r| !matches!(r[idx], Field::Null))
            .count();
        let mut distinct = BTreeSet::new();
        for row in &t_drawing.rows {
            if !matches!(row[idx], Field::Null) {
                distinct.insert(field_display(&row[idx]));
            }
        }
        let sample = distinct
            .iter()
            .take(4)
            .cloned()
            .collect::<Vec<_>>()
            .join(" | ");
        println!(
            "  col[{idx:>2}] {:<26} kind={:<4} non_null={:>2}/{} distinct={:>2} sample=[{}]",
            col.name,
            col.kind.label(),
            non_null,
            t_drawing.rows.len(),
            distinct.len(),
            sample
        );
    }

    let mut drawing_by_id: HashMap<String, (String, String, String)> = HashMap::new();
    for row in &t_drawing.rows {
        let sp_id = text_at(row, &t_drawing.columns, "SP_ID");
        let name = text_at(row, &t_drawing.columns, "NAME");
        let dwg_no = text_at(row, &t_drawing.columns, "DRAWINGNUMBER");
        let path = text_at(row, &t_drawing.columns, "PATH");
        drawing_by_id.insert(
            norm_hex(&sp_id),
            (name.clone(), dwg_no.clone(), path.clone()),
        );
        if sp_id.eq_ignore_ascii_case("5633AF4121EE4F4D9FE78E90722CC050")
            || name.starts_with("I井场")
        {
            println!("\n--- T_DRAWING row for I井场 ---");
            for (idx, col) in t_drawing.columns.iter().enumerate() {
                if !matches!(row[idx], Field::Null)
                    || matches!(
                        col.name.as_str(),
                        "REVISION" | "VERSION" | "LASTPUBLISHTIME" | "PATH" | "DRAWINGNUMBER"
                    )
                {
                    println!("  {:<26} = {}", col.name, field_display(&row[idx]));
                }
            }
        }
    }

    println!("\n--- T_DRAWING rows with non-null VERSION, REVISION, or LASTPUBLISHTIME ---");
    let mut any_ver_rev_pub = 0usize;
    for row in &t_drawing.rows {
        let rev = field_display(field_at(row, &t_drawing.columns, "REVISION"));
        let ver = field_display(field_at(row, &t_drawing.columns, "VERSION"));
        let pub_time = field_display(field_at(row, &t_drawing.columns, "LASTPUBLISHTIME"));
        if rev != "NULL" || ver != "NULL" || pub_time != "NULL" {
            any_ver_rev_pub += 1;
            println!(
                "  SP_ID={} NAME={} VERSION={} REVISION={} LASTPUBLISHTIME={}",
                text_at(row, &t_drawing.columns, "SP_ID"),
                text_at(row, &t_drawing.columns, "NAME"),
                ver,
                rev,
                pub_time
            );
        }
    }
    println!("  total rows with VERSION/REVISION/LASTPUBLISHTIME non-null: {any_ver_rev_pub}");

    // 3. Inspect T_DRAWINGVERSION in detail.
    let t_dv = tables_by_name
        .get("T_DRAWINGVERSION")
        .ok_or("T_DRAWINGVERSION not found")?;
    println!(
        "\n=== 3. T_DRAWINGVERSION (rows={}, cols={}) ===",
        t_dv.rows.len(),
        t_dv.columns.len()
    );
    for (idx, col) in t_dv.columns.iter().enumerate() {
        let non_null = t_dv
            .rows
            .iter()
            .filter(|r| !matches!(r[idx], Field::Null))
            .count();
        println!(
            "  col[{idx:>2}] {:<26} kind={:<4} non_null={:>2}/{}",
            col.name,
            col.kind.label(),
            non_null,
            t_dv.rows.len()
        );
    }
    println!("\n--- All {} rows of T_DRAWINGVERSION ---", t_dv.rows.len());
    let mut in_t_drawing_count = 0usize;
    for (row_idx, row) in t_dv.rows.iter().enumerate() {
        let sp_id = text_at(row, &t_dv.columns, "SP_ID");
        let dwg_id = text_at(row, &t_dv.columns, "SP_DRAWINGID");
        let ver_name = text_at(row, &t_dv.columns, "NAME");
        let dwg_no = text_at(row, &t_dv.columns, "DRAWINGNUMBER");
        let date_created = field_display(field_at(row, &t_dv.columns, "DATECREATED"));
        let comments = text_at(row, &t_dv.columns, "SP_COMMENTS");
        let purpose = field_display(field_at(row, &t_dv.columns, "SP_VERSIONPURPOSE"));
        let in_dwg = drawing_by_id.get(&norm_hex(&dwg_id));
        if in_dwg.is_some() {
            in_t_drawing_count += 1;
        }
        let match_label = in_dwg.map_or_else(
            || "NOT_IN_T_DRAWING".to_string(),
            |(n, no, p)| format!("IN_T_DRAWING({n} / {no} / {p})"),
        );
        println!(
            "  [{row_idx:>2}] SP_ID={sp_id} SP_DRAWINGID={dwg_id} [{match_label}] NAME={ver_name:?} NO={dwg_no:?} DATE={date_created} PURPOSE={purpose} COMMENTS={comments:?}"
        );
    }
    println!(
        "  T_DRAWINGVERSION summary: total={} matched_in_T_DRAWING={in_t_drawing_count} not_in_T_DRAWING={}",
        t_dv.rows.len(),
        t_dv.rows.len() - in_t_drawing_count
    );

    // 4. Inspect T_REVISION, T_DRAWINGSITE, T_DRAWINGPROJECT, T_GLOBALDRAWING, T_PLANT.
    for tname in [
        "T_REVISION",
        "T_DRAWINGSITE",
        "T_DRAWINGPROJECT",
        "T_GLOBALDRAWING",
        "T_PLANT",
    ] {
        if let Some(tbl) = tables_by_name.get(tname) {
            let cols: Vec<String> = tbl
                .columns
                .iter()
                .enumerate()
                .map(|(idx, col)| {
                    let non_null = tbl
                        .rows
                        .iter()
                        .filter(|r| !matches!(r[idx], Field::Null))
                        .count();
                    format!("{}:{}(nn={non_null})", col.name, col.kind.label())
                })
                .collect();
            println!(
                "\n=== 4. {tname} (rows={}, cols={}) ===\n  {}",
                tbl.rows.len(),
                tbl.columns.len(),
                cols.join(", ")
            );
        }
    }

    // 5. Parse the 5 reference _Meta.xml files and inspect UIDs + attributes.
    println!("\n=== 5. Reference _Meta.xml Files (5 Drawings) ===");
    let mut meta_docs = Vec::new();
    for stem in FIVE_STEMS {
        let meta_path = ref_dir.join(format!("{stem}_Meta.xml"));
        let pid_path = ref_dir.join(format!("{stem}.pid"));
        let doc = read_meta_xml(&meta_path)?;
        println!("\n--- {stem} ---");
        println!(
            "  Container: CompSchema={} DocUID={} DocName={} Version={:?} ToolID={} ToolSignature={}",
            doc.comp_schema, doc.doc_uid, doc.doc_name, doc.container_version, doc.tool_id, doc.tool_signature
        );
        println!(
            "  DocumentVersion:  UID={:<40} (len={:>2}) Name={:?} DocRevision={:?} DocVersionDate={:?} DocVersion={:?}",
            doc.version_uid,
            doc.version_uid.len(),
            doc.version_name,
            doc.doc_revision,
            doc.doc_version_date,
            doc.doc_version
        );
        println!(
            "  DocumentRevision: UID={:<40} (len={:>2}) Name={:?} MajorRev={:?} MinorRev={:?}",
            doc.revision_uid,
            doc.revision_uid.len(),
            doc.revision_name,
            doc.major_rev,
            doc.minor_rev
        );
        println!(
            "  File:             UID={:<40} (len={:>2}) Name={:?} Description={:?} FilePath={:?}",
            doc.file_uid,
            doc.file_uid.len(),
            doc.file_name,
            doc.file_desc,
            doc.file_path
        );
        for rel in &doc.rels {
            println!(
                "  Rel {:<16}: UID={:<40} (len={:>2}) UID1={} UID2={}",
                rel.def_uid,
                rel.uid,
                rel.uid.len(),
                rel.uid1,
                rel.uid2
            );
        }

        // Check if any of the 6 UIDs appear in the corresponding .pid file
        if pid_path.is_file() {
            let pid_bytes = std::fs::read(&pid_path)
                .map_err(|e| format!("read {}: {e}", pid_path.display()))?;
            let six = doc.six_uids();
            let mut pid_hits = 0usize;
            for (role, uid) in &six {
                for probe in candidate_strings(uid) {
                    if contains_bytes(&pid_bytes, probe.as_bytes())
                        || contains_bytes(&pid_bytes, &utf16_le(&probe))
                        || contains_bytes(&pid_bytes, &utf16_be(&probe))
                    {
                        println!("  [PID HIT] {role} {uid} ({probe}) found in .pid!");
                        pid_hits += 1;
                    }
                }
                for raw16 in candidate_raw16(uid) {
                    if contains_bytes(&pid_bytes, &raw16) {
                        println!("  [PID RAW16 HIT] {role} {uid} found in .pid!");
                        pid_hits += 1;
                    }
                }
            }
            let doc_uid_ascii = contains_bytes(&pid_bytes, doc.doc_uid.as_bytes());
            println!(
                "  .pid byte scan ({} bytes): meta_uid_hits={pid_hits} (sanity DocUID_ascii={doc_uid_ascii})",
                pid_bytes.len()
            );
        }
        meta_docs.push(doc);
    }

    // 6. Search all 30 reference _Meta.xml UIDs across all tables and raw bytes of Export.dmp.
    println!("\n=== 6. Search Reference _Meta.xml UIDs in Export.dmp ===");
    let mut target_hex: HashMap<String, String> = HashMap::new();
    for doc in &meta_docs {
        for (role, uid) in doc.six_uids() {
            for cand in candidate_hex_keys(uid) {
                target_hex.insert(cand, format!("{}:{role}:{uid}", doc.doc_name));
            }
        }
    }

    let mut table_hits = 0usize;
    for (label, at) in &sites {
        let table = read_table_at(&dump, label, *at)?;
        for (r_idx, row) in table.rows.iter().enumerate() {
            for (c_idx, field) in row.iter().enumerate() {
                if let Field::Text(text) = field {
                    let h = norm_hex(text);
                    if !h.is_empty() {
                        if let Some(who) = target_hex.get(&h) {
                            println!(
                                "  [TABLE HIT] {label} row {r_idx} col {} = {text:?} -> {who}",
                                table.columns[c_idx].name
                            );
                            table_hits += 1;
                        }
                    }
                }
            }
        }
    }

    let mut dump_byte_hits = 0usize;
    for doc in &meta_docs {
        for (role, uid) in doc.six_uids() {
            for probe in candidate_strings(uid) {
                if contains_bytes(&dump, probe.as_bytes())
                    || contains_bytes(&dump, &utf16_be(&probe))
                    || contains_bytes(&dump, &utf16_le(&probe))
                {
                    println!("  [DUMP STR HIT] {} {role} {uid} ({probe})", doc.doc_name);
                    dump_byte_hits += 1;
                }
            }
            for raw16 in candidate_raw16(uid) {
                if contains_bytes(&dump, &raw16) {
                    println!("  [DUMP RAW16 HIT] {} {role} {uid}", doc.doc_name);
                    dump_byte_hits += 1;
                }
            }
        }
    }
    let ijc_doc_uid_in_dump = contains_bytes(&dump, &utf16_be("5633AF4121EE4F4D9FE78E90722CC050"));
    println!(
        "  dump scan summary: table_cell_hits={table_hits} raw_byte_hits={dump_byte_hits} (sanity I井场 DocUID utf16be in dump = {ijc_doc_uid_in_dump})"
    );

    // 7. Search for FilePath host/share (`Glpid`, `pidsq`) in Export.dmp, Manifest.txt, and .pid.
    println!("\n=== 7. Search for FilePath (`Glpid-7`, `pidsq`) in Dump / Manifest / .pid ===");
    for needle in ["Glpid", "glpid", "pidsq", "PIDSQ"] {
        let in_dump_ascii = contains_bytes(&dump, needle.as_bytes());
        let in_dump_u16be = contains_bytes(&dump, &utf16_be(needle));
        let in_dump_u16le = contains_bytes(&dump, &utf16_le(needle));
        println!(
            "  dump needle={needle:<8} ascii={in_dump_ascii} utf16be={in_dump_u16be} utf16le={in_dump_u16le}"
        );
    }
    let manifest_path = dump_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("Manifest.txt");
    if let Ok(manifest_bytes) = std::fs::read(&manifest_path) {
        for line in String::from_utf8_lossy(&manifest_bytes).lines() {
            if line.contains("Glpid")
                || line.contains("pidsq")
                || line.starts_with("Server")
                || line.starts_with("Path")
                || line.starts_with("Plant")
                || line.starts_with("Site")
                || line.starts_with("Date")
            {
                println!("  Manifest.txt: {line}");
            }
        }
    }
    let ijc_pid = ref_dir.join(format!("{}.pid", FIVE_STEMS[0]));
    if let Ok(pid_bytes) = std::fs::read(&ijc_pid) {
        for needle in ["Glpid", "glpid", "pidsq", "PIDSQ", "2026/06/22"] {
            let a = contains_bytes(&pid_bytes, needle.as_bytes());
            let le = contains_bytes(&pid_bytes, &utf16_le(needle));
            println!("  I井场.pid needle={needle:<12} ascii={a} utf16le={le}");
        }
    }

    Ok(())
}

#[derive(Debug)]
struct MetaXmlDoc {
    comp_schema: String,
    doc_uid: String,
    doc_name: String,
    container_version: String,
    tool_id: String,
    tool_signature: String,
    version_uid: String,
    version_name: String,
    doc_revision: String,
    doc_version_date: String,
    doc_version: String,
    revision_uid: String,
    revision_name: String,
    major_rev: String,
    minor_rev: String,
    file_uid: String,
    file_name: String,
    file_desc: String,
    file_path: String,
    rels: Vec<MetaRel>,
}

#[derive(Debug)]
struct MetaRel {
    uid: String,
    uid1: String,
    uid2: String,
    def_uid: String,
}

impl MetaXmlDoc {
    fn six_uids(&self) -> Vec<(&'static str, &str)> {
        let mut out = vec![
            ("DocumentVersion", self.version_uid.as_str()),
            ("DocumentRevision", self.revision_uid.as_str()),
            ("File", self.file_uid.as_str()),
        ];
        for rel in &self.rels {
            let label = match rel.def_uid.as_str() {
                "VersionedDoc" => "Rel(VersionedDoc)",
                "RevisedDocument" => "Rel(RevisedDocument)",
                "FileComposition" => "Rel(FileComposition)",
                _ => "Rel(Other)",
            };
            out.push((label, rel.uid.as_str()));
        }
        out
    }
}

fn read_meta_xml(path: &Path) -> Result<MetaXmlDoc, String> {
    let xml =
        std::fs::read_to_string(path).map_err(|err| format!("read {}: {err}", path.display()))?;
    let mut reader = Reader::from_str(&xml);
    reader.config_mut().trim_text(true);
    let mut doc = MetaXmlDoc {
        comp_schema: String::new(),
        doc_uid: String::new(),
        doc_name: String::new(),
        container_version: String::new(),
        tool_id: String::new(),
        tool_signature: String::new(),
        version_uid: String::new(),
        version_name: String::new(),
        doc_revision: String::new(),
        doc_version_date: String::new(),
        doc_version: String::new(),
        revision_uid: String::new(),
        revision_name: String::new(),
        major_rev: String::new(),
        minor_rev: String::new(),
        file_uid: String::new(),
        file_name: String::new(),
        file_desc: String::new(),
        file_path: String::new(),
        rels: Vec::new(),
    };
    let mut depth = 0usize;
    let mut tag = String::new();
    let mut values: BTreeMap<String, String> = BTreeMap::new();
    loop {
        match reader.read_event() {
            Ok(quick_xml::events::Event::Start(start)) => {
                if depth == 0 {
                    for (k, v) in attributes_of(&start) {
                        match k.as_str() {
                            "CompSchema" => doc.comp_schema = v,
                            "DocUID" => doc.doc_uid = v,
                            "DocName" => doc.doc_name = v,
                            "Version" => doc.container_version = v,
                            "ToolID" => doc.tool_id = v,
                            "ToolSignature" => doc.tool_signature = v,
                            _ => {}
                        }
                    }
                } else if depth == 1 {
                    tag = local_name(&start);
                    values.clear();
                } else if depth == 2 {
                    take(&start, &mut values);
                }
                depth += 1;
            }
            Ok(quick_xml::events::Event::Empty(start)) => {
                if depth == 2 {
                    take(&start, &mut values);
                }
            }
            Ok(quick_xml::events::Event::End(_)) => {
                depth = depth.saturating_sub(1);
                if depth == 1 {
                    match tag.as_str() {
                        "DocumentVersion" => {
                            doc.version_uid =
                                values.get("IObject.UID").cloned().unwrap_or_default();
                            doc.version_name =
                                values.get("IObject.Name").cloned().unwrap_or_default();
                            doc.doc_revision = values
                                .get("IDocumentVersion.DocRevision")
                                .cloned()
                                .unwrap_or_default();
                            doc.doc_version_date = values
                                .get("IDocumentVersion.DocVersionDate")
                                .cloned()
                                .unwrap_or_default();
                            doc.doc_version = values
                                .get("IDocumentVersion.DocVersion")
                                .cloned()
                                .unwrap_or_default();
                        }
                        "DocumentRevision" => {
                            doc.revision_uid =
                                values.get("IObject.UID").cloned().unwrap_or_default();
                            doc.revision_name =
                                values.get("IObject.Name").cloned().unwrap_or_default();
                            doc.major_rev = values
                                .get("IDocumentRevision.MajorRev_ForRevise")
                                .cloned()
                                .unwrap_or_default();
                            doc.minor_rev = values
                                .get("IDocumentRevision.MinorRev_ForRevise")
                                .cloned()
                                .unwrap_or_default();
                        }
                        "File" => {
                            doc.file_uid = values.get("IObject.UID").cloned().unwrap_or_default();
                            doc.file_name = values.get("IObject.Name").cloned().unwrap_or_default();
                            doc.file_desc = values
                                .get("IObject.Description")
                                .cloned()
                                .unwrap_or_default();
                            doc.file_path =
                                values.get("IFile.FilePath").cloned().unwrap_or_default();
                        }
                        "Rel" => {
                            doc.rels.push(MetaRel {
                                uid: values.get("IObject.UID").cloned().unwrap_or_default(),
                                uid1: values.get("IRel.UID1").cloned().unwrap_or_default(),
                                uid2: values.get("IRel.UID2").cloned().unwrap_or_default(),
                                def_uid: values.get("IRel.DefUID").cloned().unwrap_or_default(),
                            });
                        }
                        _ => {}
                    }
                }
            }
            Ok(quick_xml::events::Event::Eof) => break,
            Err(err) => return Err(format!("xml {}: {err}", path.display())),
            _ => {}
        }
    }
    Ok(doc)
}

fn candidate_strings(uid: &str) -> Vec<String> {
    let mut out = BTreeSet::new();
    out.insert(uid.to_string());
    out.insert(uid.to_ascii_lowercase());
    if let Some(stripped) = uid.strip_prefix("AAAD") {
        out.insert(stripped.to_string());
        out.insert(stripped.to_ascii_lowercase());
        for hex32 in expand_short_guid_to_32hex(stripped) {
            out.insert(hex32.clone());
            out.insert(hex32.to_ascii_lowercase());
        }
    } else {
        for hex32 in expand_31_or_32_hex(uid) {
            out.insert(hex32.clone());
            out.insert(hex32.to_ascii_lowercase());
        }
    }
    out.into_iter().collect()
}

fn candidate_hex_keys(uid: &str) -> Vec<String> {
    let mut out = BTreeSet::new();
    out.insert(norm_hex(uid));
    if let Some(stripped) = uid.strip_prefix("AAAD") {
        out.insert(norm_hex(stripped));
        for hex32 in expand_short_guid_to_32hex(stripped) {
            out.insert(hex32);
        }
    } else {
        for hex32 in expand_31_or_32_hex(uid) {
            out.insert(hex32);
        }
    }
    out.into_iter().filter(|s| !s.is_empty()).collect()
}

fn candidate_raw16(uid: &str) -> Vec<Vec<u8>> {
    let mut hex32s = BTreeSet::new();
    if let Some(stripped) = uid.strip_prefix("AAAD") {
        for h in expand_short_guid_to_32hex(stripped) {
            hex32s.insert(h);
        }
    } else {
        for h in expand_31_or_32_hex(uid) {
            hex32s.insert(h);
        }
    }
    let mut out = Vec::new();
    for h in hex32s {
        if let Some(bytes) = hex_bytes(&h) {
            if bytes.len() == 16 {
                // Big-endian (straight) and mixed-endian (Windows GUID)
                let mut mixed = bytes.clone();
                mixed[0..4].reverse();
                mixed[4..6].reverse();
                mixed[6..8].reverse();
                out.push(bytes);
                out.push(mixed);
            }
        }
    }
    out
}

/// A hyphenated GUID like `5EA18AC7-67-44C5-AEAF33D4C22EB56` has 4 hyphen-separated
/// fields: Data1 (up to 8 hex), Data2 (up to 4 hex), Data3 (up to 4 hex), and Data4
/// (up to 16 hex, where the first byte of Data4 or subsequent bytes might also have
/// dropped a leading zero).
fn expand_short_guid_to_32hex(s: &str) -> Vec<String> {
    let parts: Vec<&str> = s.split('-').collect();
    if parts.len() != 4 {
        return Vec::new();
    }
    let p1 = format!("{:0>8}", parts[0].to_ascii_uppercase());
    let p2 = format!("{:0>4}", parts[1].to_ascii_uppercase());
    let p3 = format!("{:0>4}", parts[2].to_ascii_uppercase());
    let prefix = format!("{p1}{p2}{p3}");
    let p4 = parts[3].to_ascii_uppercase();
    expand_31_or_32_hex(&format!("{prefix}{p4}"))
}

/// If a hex string is 32 chars, return it; if 31 chars (one byte formatted with `%X`
/// instead of `%02X`), return all 16 possible positions of inserting a leading `0` at
/// an even byte boundary.
fn expand_31_or_32_hex(s: &str) -> Vec<String> {
    let clean = norm_hex(s);
    if clean.len() == 32 {
        return vec![clean];
    }
    if clean.len() == 31 {
        let mut out = Vec::new();
        for byte_idx in 0..16 {
            let pos = byte_idx * 2;
            if pos <= clean.len() {
                let mut candidate = String::with_capacity(32);
                candidate.push_str(&clean[..pos]);
                candidate.push('0');
                candidate.push_str(&clean[pos..]);
                out.push(candidate);
            }
        }
        return out;
    }
    Vec::new()
}

fn norm_hex(text: &str) -> String {
    text.chars()
        .filter(|ch| ch.is_ascii_hexdigit())
        .map(|ch| ch.to_ascii_uppercase())
        .collect()
}

fn pin_applications(table: &Table) -> Result<(), String> {
    let row = table
        .rows
        .first()
        .ok_or("APPLICATIONS has no rows; row format is not pinned")?;
    let id = int_at(row, &table.columns, "ID");
    let name = text_at(row, &table.columns, "NAME");
    let short = text_at(row, &table.columns, "SHORTNAME");
    println!(
        "format_pin APPLICATIONS rows={} id={id:?} name={name:?} short={short:?}",
        table.rows.len()
    );
    if id != Some(1) || name != "SmartPlant PID" || short != "SPPID" {
        return Err(
            "APPLICATIONS row 0 is not (1, SmartPlant PID, SPPID); refusing to scan".into(),
        );
    }
    Ok(())
}

struct Table {
    columns: Vec<Column>,
    rows: Vec<Vec<Field>>,
}

struct Column {
    name: String,
    kind: Kind,
    maxlen: u16,
    form: Option<u16>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Char,
    Num,
    Date,
    Long,
}

impl Kind {
    fn label(self) -> &'static str {
        match self {
            Self::Char => "CHAR",
            Self::Num => "NUM",
            Self::Date => "DATE",
            Self::Long => "LONG",
        }
    }
}

#[derive(Clone)]
enum Field {
    Null,
    Int(i64),
    Text(String),
    Date(String),
    Unscaled,
    Skip,
}

fn field_display(field: &Field) -> String {
    match field {
        Field::Null => "NULL".into(),
        Field::Int(v) => v.to_string(),
        Field::Text(s) => format!("{s:?}"),
        Field::Date(d) => d.clone(),
        Field::Unscaled => "<unscaled>".into(),
        Field::Skip => "<long>".into(),
    }
}

fn insert_sites(dump: &[u8]) -> Vec<(String, usize)> {
    let marker = b"INSERT INTO \"";
    let mut sites = Vec::new();
    let mut seen: HashMap<String, usize> = HashMap::new();
    let mut from = 0usize;
    while let Some(rel) = find(&dump[from..], marker) {
        let at = from + rel;
        from = at + marker.len();
        if at > 0 && dump[at - 1] != b'\n' {
            continue;
        }
        let name_at = at + marker.len();
        let Some(end) = dump[name_at..].iter().position(|byte| *byte == b'"') else {
            continue;
        };
        let Ok(raw) = std::str::from_utf8(&dump[name_at..name_at + end]) else {
            continue;
        };
        let n = seen.entry(raw.to_string()).or_insert(0);
        *n += 1;
        let label = if *n == 1 {
            raw.to_string()
        } else {
            format!("{raw}#{}", *n)
        };
        sites.push((label, at));
    }
    sites
}

fn text_at(row: &[Field], columns: &[Column], name: &str) -> String {
    match field_at(row, columns, name) {
        Field::Text(text) => text.clone(),
        _ => String::new(),
    }
}

fn int_at(row: &[Field], columns: &[Column], name: &str) -> Option<i64> {
    match field_at(row, columns, name) {
        Field::Int(value) => Some(*value),
        _ => None,
    }
}

fn field_at<'a>(row: &'a [Field], columns: &[Column], name: &str) -> &'a Field {
    let index = columns
        .iter()
        .position(|column| column.name == name)
        .unwrap_or_else(|| panic!("column {name}"));
    &row[index]
}

fn read_table_at(dump: &[u8], name: &str, at: usize) -> Result<Table, String> {
    let line_end = dump[at..]
        .iter()
        .position(|byte| *byte == b'\n')
        .map(|offset| at + offset)
        .ok_or_else(|| format!("{name}: INSERT line has no newline"))?;
    let line = std::str::from_utf8(&dump[at..line_end])
        .map_err(|_| format!("{name}: INSERT line is not ASCII"))?;
    let names = column_names(line).map_err(|err| format!("{name}: {err}"))?;
    let (columns, mut pos) =
        parse_header(dump, line_end + 1, &names).map_err(|err| format!("{name}: {err}"))?;
    let has_long = columns.iter().any(|column| column.kind == Kind::Long);
    if has_long {
        pos = skip_long_preamble(dump, pos, &columns).map_err(|err| format!("{name}: {err}"))?;
    } else if dump.get(pos..pos + 4) == Some(&[0, 0, 0, 0]) {
        pos += 4;
    } else {
        return Err(format!("{name}: column header terminator at {pos:#x}"));
    }
    let mut rows = Vec::new();
    loop {
        if pos + 2 > dump.len() {
            return Err(format!("{name}: truncated before end marker"));
        }
        let peek = u16(dump, pos);
        if peek == 0xFFFF {
            return Ok(Table { columns, rows });
        }
        let mut row = Vec::with_capacity(columns.len());
        let mut long_present = false;
        for column in &columns {
            let (field, next) = read_field(dump, pos, column)
                .map_err(|err| format!("{name}: row {} {err}", rows.len()))?;
            if column.kind == Kind::Long && matches!(field, Field::Skip) {
                long_present = true;
            }
            pos = next;
            row.push(field);
        }
        pos = finish_row(dump, pos, long_present)
            .map_err(|err| format!("{name}: row {} {err}", rows.len()))?;
        rows.push(row);
    }
}

fn skip_long_preamble(dump: &[u8], pos: usize, columns: &[Column]) -> Result<usize, String> {
    if dump.get(pos..pos + 4) == Some(&[0, 0, 0, 0]) {
        return skip_long_descriptor(dump, pos + 4);
    }
    if pos + 2 <= dump.len() && u16(dump, pos) == columns.len() as u16 {
        let name = columns
            .iter()
            .find(|column| column.kind == Kind::Long)
            .map(|column| column.name.as_str())
            .ok_or("long column missing")?;
        return skip_named_padding(dump, pos + 2, name);
    }
    Err(format!("column header terminator at {pos:#x}"))
}

fn skip_named_padding(dump: &[u8], pos: usize, name: &str) -> Result<usize, String> {
    let window = dump.get(pos..dump.len().min(pos + 80)).unwrap_or_default();
    let rel =
        find(window, name.as_bytes()).ok_or_else(|| format!("long name {name} not in preamble"))?;
    let mut at = pos + rel + name.len();
    while dump.get(at) == Some(&0) {
        at += 1;
    }
    Ok(at)
}

fn skip_long_descriptor(dump: &[u8], mut pos: usize) -> Result<usize, String> {
    if pos + 4 > dump.len() {
        return Err("truncated long descriptor".into());
    }
    let pre = u32_at(dump, pos);
    pos += 4;
    if pre != 0 {
        return Err(format!("long descriptor prefix {pre:#x} at {pos:#x}"));
    }
    if pos >= dump.len() {
        return Err("truncated long name".into());
    }
    let name_len = dump[pos] as usize;
    pos += 1 + name_len;
    if pos + 2 > dump.len() {
        return Err("truncated long name tail".into());
    }
    pos += 2;
    Ok(pos)
}

fn column_names(line: &str) -> Result<Vec<String>, String> {
    let open = line.find('(').ok_or("INSERT has no column list")?;
    let close = line.find(") VALUES").ok_or("INSERT has no VALUES")?;
    let mut names = Vec::new();
    for piece in line[open + 1..close].split(',') {
        let name = piece.trim().trim_matches('"');
        if name.is_empty() {
            return Err("empty column name".into());
        }
        names.push(name.to_string());
    }
    Ok(names)
}

fn parse_header(
    dump: &[u8],
    mut pos: usize,
    names: &[String],
) -> Result<(Vec<Column>, usize), String> {
    let count = u16(dump, pos) as usize;
    pos += 2;
    if count != names.len() {
        return Err(format!(
            "header columns {count} != INSERT names {}",
            names.len()
        ));
    }
    let mut columns = Vec::with_capacity(count);
    for name in names {
        let kind_code = u16(dump, pos);
        pos += 2;
        let column = match kind_code {
            1 => {
                let maxlen = u16(dump, pos);
                pos += 2;
                let _charset = u16(dump, pos);
                pos += 2;
                let form = u16(dump, pos);
                pos += 2;
                Column {
                    name: name.clone(),
                    kind: Kind::Char,
                    maxlen,
                    form: Some(form),
                }
            }
            2 => {
                let maxlen = u16(dump, pos);
                pos += 2;
                Column {
                    name: name.clone(),
                    kind: Kind::Num,
                    maxlen,
                    form: None,
                }
            }
            12 => {
                let maxlen = u16(dump, pos);
                pos += 2;
                Column {
                    name: name.clone(),
                    kind: Kind::Date,
                    maxlen,
                    form: None,
                }
            }
            113 => {
                let maxlen = u16(dump, pos);
                pos += 2;
                let _flag = u16(dump, pos);
                pos += 2;
                Column {
                    name: name.clone(),
                    kind: Kind::Long,
                    maxlen,
                    form: None,
                }
            }
            0xA0 => {
                let _charset = u16(dump, pos);
                pos += 2;
                let form = u16(dump, pos);
                pos += 2;
                Column {
                    name: name.clone(),
                    kind: Kind::Char,
                    maxlen: 4000,
                    form: Some(form),
                }
            }
            other => return Err(format!("column {name} type {other:#x} at {pos:#x}")),
        };
        columns.push(column);
    }
    Ok((columns, pos))
}

fn read_field(dump: &[u8], pos: usize, column: &Column) -> Result<(Field, usize), String> {
    if column.kind == Kind::Long {
        return read_long(dump, pos);
    }
    if pos + 2 > dump.len() {
        return Err(format!("truncated length at {pos:#x}"));
    }
    let len = u16(dump, pos) as usize;
    let pos = pos + 2;
    if len == 0xFFFE {
        return Ok((Field::Null, pos));
    }
    if len > column.maxlen as usize + 8 || pos + len > dump.len() {
        return Err(format!("length {len} at {pos:#x}"));
    }
    let raw = &dump[pos..pos + len];
    let next = pos + len;
    let field = match column.kind {
        Kind::Char => {
            if column.form == Some(2) {
                if !len.is_multiple_of(2) {
                    return Err(format!("odd UTF-16 length {len}"));
                }
                Field::Text(decode_utf16_be(raw))
            } else {
                Field::Text(raw.iter().map(|byte| char::from(*byte)).collect())
            }
        }
        Kind::Num => match decode_i64(raw) {
            Some(value) => Field::Int(value),
            None => Field::Unscaled,
        },
        Kind::Date => Field::Date(decode_oracle_date(raw)),
        Kind::Long => Field::Skip,
    };
    Ok((field, next))
}

fn decode_oracle_date(raw: &[u8]) -> String {
    if raw.len() == 7 && raw[0] >= 100 && raw[1] >= 100 {
        let year = (i32::from(raw[0]) - 100) * 100 + (i32::from(raw[1]) - 100);
        let month = raw[2];
        let day = raw[3];
        let hour = raw[4].saturating_sub(1);
        let min = raw[5].saturating_sub(1);
        let sec = raw[6].saturating_sub(1);
        format!("{year:04}/{month:02}/{day:02} {hour:02}:{min:02}:{sec:02}")
    } else {
        format!("raw({raw:02x?})")
    }
}

fn read_long(dump: &[u8], mut pos: usize) -> Result<(Field, usize), String> {
    if pos + 2 > dump.len() {
        return Err(format!("truncated long at {pos:#x}"));
    }
    let first = u16(dump, pos) as usize;
    pos += 2;
    if first == 0xFFFE {
        return Ok((Field::Null, pos));
    }
    if pos + first > dump.len() {
        return Err(format!("long piece {first} at {pos:#x}"));
    }
    pos += first;
    Ok((Field::Skip, pos))
}

fn finish_row(dump: &[u8], pos: usize, long_present: bool) -> Result<usize, String> {
    if long_present && dump.get(pos..pos + 4) == Some(&[0x01, 0x00, 0xfc, 0xff]) {
        let zip_at = pos + 16;
        let mut end = zip_end(dump, zip_at)?;
        if dump.get(end) == Some(&0) {
            end += 1;
        }
        return Ok(end);
    }
    if long_present && dump.get(pos..pos + 2) == Some(&[0x01, 0x00]) {
        if pos + 4 > dump.len() {
            return Err(format!("truncated long tail at {pos:#x}"));
        }
        let second = u16(dump, pos + 2) as usize;
        let next = pos + 4 + second;
        if next > dump.len() {
            return Err(format!("long payload {second} at {pos:#x}"));
        }
        return Ok(next);
    }
    if dump.get(pos..pos + 2) == Some(&[0, 0]) {
        return Ok(pos + 2);
    }
    Err(format!(
        "separator is {:02x?} at {pos:#x}",
        dump.get(pos..pos + 2)
    ))
}

fn zip_end(dump: &[u8], zip_at: usize) -> Result<usize, String> {
    let sig = b"PK\x05\x06";
    let rel = find(dump.get(zip_at..).unwrap_or_default(), sig)
        .ok_or_else(|| format!("long payload has no zip end at {zip_at:#x}"))?;
    let end_rec = zip_at + rel;
    if end_rec + 22 > dump.len() {
        return Err(format!("truncated zip end at {end_rec:#x}"));
    }
    let comment = u16(dump, end_rec + 20) as usize;
    let end = end_rec + 22 + comment;
    if end > dump.len() {
        return Err(format!("zip comment runs past the dump at {end_rec:#x}"));
    }
    Ok(end)
}

fn decode_utf16_be(raw: &[u8]) -> String {
    let units = raw
        .as_chunks::<2>()
        .0
        .iter()
        .map(|pair| u16::from_be_bytes(*pair))
        .collect::<Vec<_>>();
    String::from_utf16_lossy(&units)
}

fn decode_i64(raw: &[u8]) -> Option<i64> {
    if raw.is_empty() {
        return None;
    }
    if raw == [0x80] {
        return Some(0);
    }
    let negative = raw[0] < 0x80;
    let body = if negative && raw.last() == Some(&0x66) && raw.len() < 21 {
        &raw[..raw.len() - 1]
    } else {
        raw
    };
    if body.is_empty() {
        return None;
    }
    let exponent = if negative {
        62 - i32::from(body[0])
    } else {
        i32::from(body[0]) - 193
    };
    let mut total: i128 = 0;
    for (index, byte) in body[1..].iter().enumerate() {
        let digit = if negative {
            101 - i32::from(*byte)
        } else {
            i32::from(*byte) - 1
        };
        if !(0..100).contains(&digit) {
            return None;
        }
        let power = exponent - index as i32;
        if power < 0 {
            if digit != 0 {
                return None;
            }
            continue;
        }
        let mut place: i128 = i128::from(digit);
        for _ in 0..power {
            place = place.checked_mul(100)?;
        }
        total = total.checked_add(place)?;
    }
    let signed = if negative { -total } else { total };
    i64::try_from(signed).ok()
}

fn u16(dump: &[u8], pos: usize) -> u16 {
    u16::from_le_bytes([dump[pos], dump[pos + 1]])
}

fn u32_at(dump: &[u8], pos: usize) -> u32 {
    u32::from_le_bytes([dump[pos], dump[pos + 1], dump[pos + 2], dump[pos + 3]])
}

fn find(hay: &[u8], needle: &[u8]) -> Option<usize> {
    hay.windows(needle.len())
        .position(|window| window == needle)
}

fn utf16_be(text: &str) -> Vec<u8> {
    let mut out = Vec::with_capacity(text.len() * 2);
    for byte in text.bytes() {
        out.push(0);
        out.push(byte);
    }
    out
}

fn utf16_le(text: &str) -> Vec<u8> {
    let mut out = Vec::with_capacity(text.len() * 2);
    for byte in text.bytes() {
        out.push(byte);
        out.push(0);
    }
    out
}

fn hex_bytes(text: &str) -> Option<Vec<u8>> {
    if !text.len().is_multiple_of(2) {
        return None;
    }
    let mut out = Vec::with_capacity(text.len() / 2);
    let bytes = text.as_bytes();
    for pair in bytes.chunks(2) {
        let hi = hex_value(pair[0])?;
        let lo = hex_value(pair[1])?;
        out.push((hi << 4) | lo);
    }
    Some(out)
}

fn hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        _ => None,
    }
}

fn contains_bytes(hay: &[u8], needle: &[u8]) -> bool {
    if needle.is_empty() || hay.len() < needle.len() {
        return needle.is_empty();
    }
    let last = needle.len() - 1;
    let mut skip = [needle.len(); 256];
    for (index, &byte) in needle[..last].iter().enumerate() {
        skip[byte as usize] = last - index;
    }
    let mut index = 0usize;
    while index + last < hay.len() {
        let mut matched = last;
        while hay[index + matched] == needle[matched] {
            if matched == 0 {
                return true;
            }
            matched -= 1;
        }
        index += skip[hay[index + last] as usize];
    }
    false
}

fn take(start: &BytesStart<'_>, values: &mut BTreeMap<String, String>) {
    let interface = local_name(start);
    for (name, value) in attributes_of(start) {
        values.insert(format!("{interface}.{name}"), value);
    }
}

fn attributes_of(start: &BytesStart<'_>) -> Vec<(String, String)> {
    start
        .attributes()
        .flatten()
        .map(|attribute| {
            (
                String::from_utf8_lossy(attribute.key.as_ref()).into_owned(),
                attribute
                    .unescape_value()
                    .map(std::borrow::Cow::into_owned)
                    .unwrap_or_default(),
            )
        })
        .collect()
}

fn local_name(start: &BytesStart<'_>) -> String {
    String::from_utf8_lossy(start.name().as_ref()).into_owned()
}
