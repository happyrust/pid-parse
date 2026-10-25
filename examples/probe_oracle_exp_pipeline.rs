//! Read T_PIPELINE, T_PIPERUN and T_PLANTITEMGROUP out of an Oracle
//! conventional exp dump and compare them with I井场 reference
//! PIDPipeline / PipingConnectors.
//!
//! Row format is the one pinned by examples/probe_oracle_exp_rows.rs:
//! INSERT line, then a binary column header, then length-prefixed
//! cells. NVARCHAR2 is UTF-16BE. NULL is 0xFFFE. The table ends at
//! 0xFFFF. This probe does not run imp, does not connect to Oracle,
//! and does not write src/ or publish/.
//!
//! ```powershell
//! cargo run --example probe_oracle_exp_pipeline
//! ```

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::path::{Path, PathBuf};

use quick_xml::events::BytesStart;
use quick_xml::Reader;

const DEFAULT_DUMP: &str = r"D:\work\cad\pid-test-data\Export.dmp";
const DEFAULT_REFERENCE: &str =
    r"D:\work\cad\PID自研软件验证\I井场 注采阀组工艺及自控流程图_Data.xml";
const IJINGCHANG_DOC: &str = "5633AF4121EE4F4D9FE78E90722CC050";

const TABLES: &[&str] = &["T_PIPELINE", "T_PIPERUN", "T_PLANTITEMGROUP"];

fn main() {
    if let Err(err) = run() {
        eprintln!("probe_oracle_exp_pipeline: {err}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    self_check_numbers();
    let mut args = std::env::args().skip(1);
    let dump_path = args
        .next()
        .map_or_else(|| PathBuf::from(DEFAULT_DUMP), PathBuf::from);
    let reference_path = args
        .next()
        .map_or_else(|| PathBuf::from(DEFAULT_REFERENCE), PathBuf::from);

    let dump =
        std::fs::read(&dump_path).map_err(|err| format!("read {}: {err}", dump_path.display()))?;
    println!(
        "dump {} bytes ({:.2} MB)",
        dump.len(),
        dump.len() as f64 / 1_048_576.0
    );

    let apps = read_table(&dump, "APPLICATIONS")?;
    pin_applications(&apps)?;
    let mut tables = Vec::new();
    for name in TABLES {
        tables.push(read_table(&dump, name)?);
    }
    let reference = read_xml(&reference_path)?;
    println!(
        "reference DocUID={} pipelines={} piping_connectors={} rels={}",
        reference.doc_uid,
        reference.pipelines.len(),
        reference.connectors.len(),
        reference.rels.len()
    );
    if reference.doc_uid != IJINGCHANG_DOC {
        return Err(format!(
            "reference DocUID {} != {IJINGCHANG_DOC}",
            reference.doc_uid
        ));
    }

    for table in &tables {
        report_table(table);
    }
    report_drawing(&tables, &reference);
    report_uid_hits(&tables, &reference);
    Ok(())
}

struct Table {
    name: String,
    columns: Vec<Column>,
    rows: Vec<Vec<Field>>,
    tail: String,
}

struct Column {
    name: String,
    kind: Kind,
    maxlen: u16,
    charset: Option<u16>,
    form: Option<u16>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Char,
    Num,
}

#[derive(Clone)]
enum Field {
    Null,
    Int(i64),
    Text(String),
    Unscaled,
}

struct XmlDoc {
    doc_uid: String,
    objects: HashMap<String, String>,
    pipelines: HashMap<String, String>,
    connectors: HashSet<String>,
    rels: Vec<Rel>,
}

struct Rel {
    uid1: String,
    uid2: String,
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
            "APPLICATIONS row 0 is not (1, SmartPlant PID, SPPID); refusing to read the big tables"
                .into(),
        );
    }
    println!("format_pin ok tail={}", table.tail);
    Ok(())
}

fn report_table(table: &Table) {
    let null_cols = table
        .columns
        .iter()
        .filter(|column| non_null(table, &column.name) == 0)
        .count();
    println!(
        "{} rows={} cols={} null_cols={null_cols} tail={}",
        table.name,
        table.rows.len(),
        table.columns.len(),
        table.tail
    );
    for column in &table.columns {
        let count = non_null(table, &column.name);
        if count == 0 {
            continue;
        }
        match column.kind {
            Kind::Char => println!(
                "  col {} char non_null={count} maxlen={} charset={:?} form={:?}",
                column.name, column.maxlen, column.charset, column.form
            ),
            Kind::Num => println!(
                "  col {} number non_null={count} maxlen={}",
                column.name, column.maxlen
            ),
        }
    }
    if table.rows.is_empty() {
        let names = table
            .columns
            .iter()
            .map(|column| column.name.as_str())
            .collect::<Vec<_>>()
            .join(",");
        println!("  header {names}");
    }
}

fn report_drawing(tables: &[Table], reference: &XmlDoc) {
    let mut any = false;
    for table in tables {
        let names: Vec<&str> = table
            .columns
            .iter()
            .filter(|column| is_drawing_column(&column.name))
            .map(|column| column.name.as_str())
            .collect();
        println!(
            "drawing {} columns={}",
            table.name,
            if names.is_empty() {
                "0".to_string()
            } else {
                names.join(",")
            }
        );
        if !names.is_empty() {
            any = true;
            let rows = table
                .rows
                .iter()
                .filter(|row| {
                    names.iter().any(|name| {
                        norm_uid(&text_at(row, &table.columns, name)).as_deref()
                            == Some(reference.doc_uid.as_str())
                    })
                })
                .count();
            println!("drawing {} doc_rows={rows}", table.name);
        }
        let doc_cells = count_uid(table, &reference.doc_uid);
        println!("drawing {} doc_uid_cells={doc_cells}", table.name);
    }
    if !any {
        println!(
            "drawing none: T_PIPELINE / T_PIPERUN / T_PLANTITEMGROUP have no drawing column. I井场 rows are the ones whose cell normalizes to a reference PIDPipeline or PIDPipingConnector UID from DocUID {}. The other four drawings are not in this reference file.",
            reference.doc_uid
        );
    }
}

struct UidIndex {
    columns_of: HashMap<String, BTreeSet<String>>,
    rows: Vec<Vec<(String, String)>>,
}

fn index_uids(table: &Table) -> UidIndex {
    let mut columns_of: HashMap<String, BTreeSet<String>> = HashMap::new();
    let mut rows = Vec::with_capacity(table.rows.len());
    for row in &table.rows {
        let cells = uid_cells(row, &table.columns);
        for (column, uid) in &cells {
            columns_of
                .entry(uid.clone())
                .or_default()
                .insert(column.clone());
        }
        rows.push(cells);
    }
    UidIndex { columns_of, rows }
}

fn report_uid_hits(tables: &[Table], reference: &XmlDoc) {
    println!(
        "pipelines reference={} connector_objects={} connectors_in_rels={}",
        reference.pipelines.len(),
        reference.connectors.len(),
        reference.rels.len()
    );
    let mut class_pairs: BTreeMap<(String, String), usize> = BTreeMap::new();
    for rel in &reference.rels {
        let left = reference
            .objects
            .get(&rel.uid1)
            .cloned()
            .unwrap_or_else(|| "?".into());
        let right = reference
            .objects
            .get(&rel.uid2)
            .cloned()
            .unwrap_or_else(|| "?".into());
        *class_pairs.entry((left, right)).or_default() += 1;
    }
    for ((left, right), count) in &class_pairs {
        println!("  rel_ends {left} -> {right} {count}");
    }

    let indexes: Vec<UidIndex> = tables.iter().map(index_uids).collect();
    let pipeline_ids: Vec<&str> = reference.pipelines.keys().map(String::as_str).collect();
    let connector_ids: Vec<&str> = reference
        .rels
        .iter()
        .map(|rel| rel.uid1.as_str())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    let pipeline_end_ids: Vec<&str> = reference
        .rels
        .iter()
        .map(|rel| rel.uid2.as_str())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();

    let connector_object_ids: Vec<&str> = reference.connectors.iter().map(String::as_str).collect();
    score_ids(tables, &indexes, "pipeline_uid", &pipeline_ids);
    score_ids(tables, &indexes, "connector_uid", &connector_ids);
    score_ids(tables, &indexes, "rel_pipeline_uid", &pipeline_end_ids);
    score_ids(tables, &indexes, "connector_object", &connector_object_ids);
    let pipeline_set: HashSet<&str> = pipeline_ids.iter().copied().collect();
    let rel_end_set: HashSet<&str> = pipeline_end_ids.iter().copied().collect();
    println!(
        "pipeline_uid_set_eq_rel_uid2={}",
        pipeline_set == rel_end_set
    );
    if let Some((index, table)) = indexes
        .iter()
        .zip(tables.iter())
        .find(|(_, table)| table.name == "T_PIPERUN")
    {
        let rows = index
            .rows
            .iter()
            .filter(|cells| {
                cells.iter().any(|(column, uid)| {
                    column == "SP_ID" && connector_ids.iter().any(|id| id == uid)
                })
            })
            .count();
        println!(
            "ijingchang T_PIPERUN rows_sp_id_in_rel_uid1={rows} of_table={}",
            table.rows.len()
        );
    }
    report_pairs(tables, &indexes, reference);
}

fn score_ids(tables: &[Table], indexes: &[UidIndex], label: &str, ids: &[&str]) {
    let mut any: HashSet<&str> = HashSet::new();
    for (table, index) in tables.iter().zip(indexes.iter()) {
        let mut hits: HashSet<&str> = HashSet::new();
        let mut by_col: BTreeMap<&str, usize> = BTreeMap::new();
        for id in ids {
            if let Some(columns) = index.columns_of.get(*id) {
                hits.insert(id);
                for column in columns {
                    *by_col.entry(column.as_str()).or_default() += 1;
                }
            }
        }
        any.extend(hits.iter().copied());
        println!(
            "{label} {} hit={} miss={} of={}",
            table.name,
            hits.len(),
            ids.len() - hits.len(),
            ids.len()
        );
        for (name, count) in &by_col {
            println!("  {label} {} col {name}={count}", table.name);
        }
    }
    println!(
        "{label} any hit={} miss={} of={}",
        any.len(),
        ids.len() - any.len(),
        ids.len()
    );
    if any.len() != ids.len() {
        let mut missing: Vec<&str> = ids.iter().copied().filter(|id| !any.contains(id)).collect();
        missing.sort_unstable();
        for id in missing.iter().take(12) {
            println!("  {label} missing {id}");
        }
        if missing.len() > 12 {
            println!("  {label} missing_more={}", missing.len() - 12);
        }
    }
}

fn report_pairs(tables: &[Table], indexes: &[UidIndex], reference: &XmlDoc) {
    let mut seen_ordered = HashSet::new();
    let mut seen_swapped = HashSet::new();
    let mut seen_both = HashSet::new();
    for (table, index) in tables.iter().zip(indexes.iter()) {
        let mut ordered = 0usize;
        let mut swapped = 0usize;
        let mut both = 0usize;
        let mut pair_cols: BTreeMap<(String, String), usize> = BTreeMap::new();
        for rel in &reference.rels {
            let mut row_ordered = false;
            let mut row_swapped = false;
            for cells in &index.rows {
                if cells.len() < 2 {
                    continue;
                }
                for left in 0..cells.len() {
                    for right in 0..cells.len() {
                        if left == right {
                            continue;
                        }
                        if cells[left].1 == rel.uid1 && cells[right].1 == rel.uid2 {
                            row_ordered = true;
                            *pair_cols
                                .entry((cells[left].0.clone(), cells[right].0.clone()))
                                .or_default() += 1;
                        } else if cells[left].1 == rel.uid2 && cells[right].1 == rel.uid1 {
                            row_swapped = true;
                            *pair_cols
                                .entry((cells[left].0.clone(), cells[right].0.clone()))
                                .or_default() += 1;
                        }
                    }
                }
            }
            if row_ordered {
                ordered += 1;
                seen_ordered.insert((rel.uid1.clone(), rel.uid2.clone()));
            }
            if row_swapped {
                swapped += 1;
                seen_swapped.insert((rel.uid1.clone(), rel.uid2.clone()));
            }
            if row_ordered || row_swapped {
                both += 1;
                seen_both.insert((rel.uid1.clone(), rel.uid2.clone()));
            }
        }
        println!(
            "pairs {} reference={} same_row_ordered={ordered} same_row_swapped={swapped} same_row_either={both}",
            table.name,
            reference.rels.len()
        );
        if pair_cols.is_empty() {
            println!("  pairs {} columns=0", table.name);
        }
        for ((left, right), count) in &pair_cols {
            println!("  pairs {} {left} + {right} rows={count}", table.name);
        }
    }
    println!(
        "pairs any same_row_ordered={} same_row_swapped={} same_row_either={} of={}",
        seen_ordered.len(),
        seen_swapped.len(),
        seen_both.len(),
        reference.rels.len()
    );
}

fn uid_cells(row: &[Field], columns: &[Column]) -> Vec<(String, String)> {
    let mut cells = Vec::new();
    for (column, field) in columns.iter().zip(row.iter()) {
        let Field::Text(text) = field else {
            continue;
        };
        if let Some(uid) = norm_uid(text) {
            cells.push((column.name.clone(), uid));
        }
    }
    cells
}

fn is_drawing_column(name: &str) -> bool {
    let upper = name.to_ascii_uppercase();
    upper.contains("DRAWING") || upper.contains("DOCU")
}

fn non_null(table: &Table, name: &str) -> usize {
    table
        .rows
        .iter()
        .filter(|row| !matches!(field_at(row, &table.columns, name), Field::Null))
        .count()
}

fn count_uid(table: &Table, uid: &str) -> usize {
    let mut count = 0usize;
    for row in &table.rows {
        for column in &table.columns {
            if norm_uid(&text_at(row, &table.columns, &column.name)).as_deref() == Some(uid) {
                count += 1;
            }
        }
    }
    count
}

fn norm_uid(text: &str) -> Option<String> {
    let mut out = String::new();
    for ch in text.chars() {
        if ch == '{' || ch == '}' || ch == '-' || ch.is_whitespace() {
            continue;
        }
        if ch.is_ascii_hexdigit() {
            out.push(ch.to_ascii_uppercase());
        } else {
            return None;
        }
    }
    if out.len() == 32 {
        Some(out)
    } else {
        None
    }
}

fn field_at<'a>(row: &'a [Field], columns: &[Column], name: &str) -> &'a Field {
    let index = columns
        .iter()
        .position(|column| column.name == name)
        .unwrap_or_else(|| panic!("column {name}"));
    &row[index]
}

fn text_at(row: &[Field], columns: &[Column], name: &str) -> String {
    match field_at(row, columns, name) {
        Field::Text(text) => text.clone(),
        Field::Null => String::new(),
        Field::Int(value) => value.to_string(),
        Field::Unscaled => String::new(),
    }
}

fn int_at(row: &[Field], columns: &[Column], name: &str) -> Option<i64> {
    match field_at(row, columns, name) {
        Field::Int(value) => Some(*value),
        _ => None,
    }
}

fn read_table(dump: &[u8], name: &str) -> Result<Table, String> {
    let marker = format!("INSERT INTO \"{name}\"");
    let at = find(dump, marker.as_bytes()).ok_or_else(|| format!("{name}: INSERT not found"))?;
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
    let mut rows = Vec::new();
    loop {
        if pos + 2 > dump.len() {
            return Err(format!("{name}: truncated before end marker"));
        }
        let peek = u16(dump, pos);
        if peek == 0xFFFF {
            let tail = &dump[pos + 2..dump.len().min(pos + 10)];
            let tail = String::from_utf8_lossy(tail).replace('\n', "\\n");
            return Ok(Table {
                name: name.to_string(),
                columns,
                rows,
                tail,
            });
        }
        let mut row = Vec::with_capacity(columns.len());
        for column in &columns {
            let (field, next) = read_field(dump, pos, column)
                .map_err(|err| format!("{name}: row {} {err}", rows.len()))?;
            pos = next;
            row.push(field);
        }
        if dump.get(pos..pos + 2) != Some(&[0, 0]) {
            return Err(format!(
                "{name}: row {} separator is {:02x?} at {pos:#x}",
                rows.len(),
                dump.get(pos..pos + 2)
            ));
        }
        pos += 2;
        rows.push(row);
    }
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
                let charset = u16(dump, pos);
                pos += 2;
                let form = u16(dump, pos);
                pos += 2;
                Column {
                    name: name.clone(),
                    kind: Kind::Char,
                    maxlen,
                    charset: Some(charset),
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
                    charset: None,
                    form: None,
                }
            }
            other => return Err(format!("column {name} type {other:#x} at {pos:#x}")),
        };
        columns.push(column);
    }
    if dump.get(pos..pos + 4) != Some(&[0, 0, 0, 0]) {
        return Err(format!("column header terminator at {pos:#x}"));
    }
    Ok((columns, pos + 4))
}

fn read_field(dump: &[u8], pos: usize, column: &Column) -> Result<(Field, usize), String> {
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
            if !len.is_multiple_of(2) {
                return Err(format!("odd UTF-16 length {len}"));
            }
            Field::Text(decode_utf16_be(raw))
        }
        Kind::Num => match decode_i64(raw) {
            Some(value) => Field::Int(value),
            None => Field::Unscaled,
        },
    };
    Ok((field, next))
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

fn self_check_numbers() {
    assert_eq!(decode_i64(&[0x80]), Some(0));
    assert_eq!(decode_i64(&[0xc1, 0x02]), Some(1));
    assert_eq!(decode_i64(&[0xc1, 0x03]), Some(2));
    assert_eq!(decode_i64(&[0xc1, 0x06]), Some(5));
    assert_eq!(decode_i64(&[0xc2, 0x02]), Some(100));
    assert_eq!(decode_i64(&[0xc2, 0x1a, 0x34]), Some(2551));
    assert_eq!(decode_i64(&[0xc2, 0x10, 0x1f]), Some(1530));
    assert_eq!(decode_i64(&[0x3e, 0x64, 0x66]), Some(-1));
    assert_eq!(decode_i64(&[0x3e, 0x62, 0x66]), Some(-3));
}

fn u16(dump: &[u8], pos: usize) -> u16 {
    u16::from_le_bytes([dump[pos], dump[pos + 1]])
}

fn find(hay: &[u8], needle: &[u8]) -> Option<usize> {
    hay.windows(needle.len())
        .position(|window| window == needle)
}

fn read_xml(path: &Path) -> Result<XmlDoc, String> {
    let xml =
        std::fs::read_to_string(path).map_err(|err| format!("read {}: {err}", path.display()))?;
    let mut reader = Reader::from_str(&xml);
    reader.config_mut().trim_text(true);
    let mut doc = XmlDoc {
        doc_uid: String::new(),
        objects: HashMap::new(),
        pipelines: HashMap::new(),
        connectors: HashSet::new(),
        rels: Vec::new(),
    };
    let mut depth = 0usize;
    let mut tag = String::new();
    let mut values: BTreeMap<String, String> = BTreeMap::new();
    loop {
        match reader.read_event() {
            Ok(quick_xml::events::Event::Start(start)) => {
                if depth == 0 {
                    take_doc_uid(&start, &mut doc.doc_uid);
                } else if depth == 1 {
                    tag = local_name(&start);
                    values.clear();
                } else if depth == 2 {
                    take(&start, &mut values);
                }
                depth += 1;
            }
            Ok(quick_xml::events::Event::Empty(start)) => {
                if depth == 0 {
                    take_doc_uid(&start, &mut doc.doc_uid);
                } else if depth == 2 {
                    take(&start, &mut values);
                }
            }
            Ok(quick_xml::events::Event::End(_)) => {
                depth = depth.saturating_sub(1);
                if depth == 1 {
                    finish(&tag, &values, &mut doc);
                }
            }
            Ok(quick_xml::events::Event::Eof) => break,
            Err(err) => return Err(format!("xml {}: {err}", path.display())),
            _ => {}
        }
    }
    Ok(doc)
}

fn take_doc_uid(start: &BytesStart<'_>, doc_uid: &mut String) {
    if local_name(start) != "Container" {
        return;
    }
    for (name, value) in attributes_of(start) {
        if name == "DocUID" {
            *doc_uid = value;
        }
    }
}

fn take(start: &BytesStart<'_>, values: &mut BTreeMap<String, String>) {
    let interface = local_name(start);
    for (name, value) in attributes_of(start) {
        values.insert(format!("{interface}.{name}"), value);
    }
}

fn finish(tag: &str, values: &BTreeMap<String, String>, doc: &mut XmlDoc) {
    if tag == "Rel" {
        if values.get("IRel.DefUID").map(String::as_str) == Some("PipingConnectors") {
            doc.rels.push(Rel {
                uid1: norm_uid(values.get("IRel.UID1").map(String::as_str).unwrap_or(""))
                    .unwrap_or_default(),
                uid2: norm_uid(values.get("IRel.UID2").map(String::as_str).unwrap_or(""))
                    .unwrap_or_default(),
            });
        }
        return;
    }
    let Some(uid) = values.get("IObject.UID").and_then(|value| norm_uid(value)) else {
        return;
    };
    doc.objects.insert(uid.clone(), tag.to_string());
    if tag == "PIDPipeline" {
        doc.pipelines
            .insert(uid, values.get("IObject.Name").cloned().unwrap_or_default());
    } else if tag == "PIDPipingConnector" {
        doc.connectors.insert(uid);
    }
}

fn attributes_of(start: &BytesStart<'_>) -> Vec<(String, String)> {
    start
        .attributes()
        .flatten()
        .map(|attribute| {
            (
                attribute.key.as_ref().to_string(),
                attribute
                    .normalized_value(quick_xml::XmlVersion::Implicit1_0)
                    .map(std::borrow::Cow::into_owned)
                    .unwrap_or_default(),
            )
        })
        .collect()
}

fn local_name(start: &BytesStart<'_>) -> String {
    start.name().as_ref().to_string()
}
