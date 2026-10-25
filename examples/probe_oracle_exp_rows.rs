//! Read I井场 rows out of an Oracle conventional `exp` dump by column,
//! then compare them with the reference publish XML.
//!
//! The dump stores each table as an ASCII `INSERT INTO ... VALUES`
//! line followed by a binary column header and one record per row.
//! Character columns are UTF-16BE (charset id 2000, AL16UTF16). A view
//! shifted by one byte looks like UTF-16LE (`S\0m\0a\0r\0t`); the
//! length-prefixed payload itself is big-endian. NULL is the unsigned
//! length `0xFFFE`. Each row ends with two zero bytes. The table ends
//! with length `0xFFFF` and then the next `CREATE` statement.
//!
//! This probe does not run `imp`, does not connect to Oracle, and does
//! not write `src/publish` or the `publish\` directory.
//!
//! ```powershell
//! cargo run --example probe_oracle_exp_rows
//! ```

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};

use quick_xml::events::BytesStart;
use quick_xml::Reader;

const DEFAULT_DUMP: &str = r"D:\work\cad\pid-test-data\Export.dmp";
const DEFAULT_REFERENCE: &str =
    r"D:\work\cad\PID自研软件验证\I井场 注采阀组工艺及自控流程图_Data.xml";
const DEFAULT_PUBLISH: &str =
    r"D:\work\cad\PID自研软件验证\publish\I井场 注采阀组工艺及自控流程图_Data.xml";

const CATEGORIES: &[(&str, &str)] = &[
    ("EquipmentComponentComposition", "设备↔管口"),
    ("PBSItemNotes", "注释"),
    ("PipingConnectors", "管线↔管段"),
];

fn main() {
    if let Err(err) = run() {
        eprintln!("probe_oracle_exp_rows: {err}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    self_check_numbers();
    assert_eq!(nominal_label(25).as_deref(), Some("25 mm"));
    assert_eq!(nominal_label(5024).as_deref(), Some("0.75 in"));
    assert_eq!(nominal_label(5032).as_deref(), Some("1 in"));
    assert_eq!(nominal_label(5384).as_deref(), Some("12 in"));
    let mut args = std::env::args().skip(1);
    let dump_path = args
        .next()
        .map_or_else(|| PathBuf::from(DEFAULT_DUMP), PathBuf::from);
    let reference_path = args
        .next()
        .map_or_else(|| PathBuf::from(DEFAULT_REFERENCE), PathBuf::from);
    let publish_path = args
        .next()
        .map_or_else(|| PathBuf::from(DEFAULT_PUBLISH), PathBuf::from);

    let dump =
        std::fs::read(&dump_path).map_err(|err| format!("read {}: {err}", dump_path.display()))?;
    println!(
        "dump {} bytes ({:.2} MB)",
        dump.len(),
        dump.len() as f64 / 1_048_576.0
    );

    let apps = read_table(&dump, "APPLICATIONS")?;
    pin_applications(&apps)?;
    let piping = read_table(&dump, "T_PIPINGPOINT")?;
    let signal = read_table(&dump, "T_SIGNALPOINT")?;
    let relationship = read_table(&dump, "T_RELATIONSHIP")?;
    let reference = read_xml(&reference_path)?;
    let publish = read_xml(&publish_path)?;

    println!(
        "reference DocUID={} objects={} piping_ports={} signal_ports={}",
        reference.doc_uid,
        reference.objects.len(),
        reference.piping.len(),
        reference.signal.len()
    );
    println!(
        "publish_output objects={} piping_ports={} signal_ports={}",
        publish.objects.len(),
        publish.piping.len(),
        publish.signal.len()
    );

    report_table(&piping);
    report_table(&signal);
    report_table(&relationship);
    report_piping(&piping, &reference);
    report_signal(&signal, &reference);
    report_relationships(&relationship, &reference, &publish);
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
    /// Non-null number that is fractional or does not fit in `i64`.
    Unscaled,
}

struct XmlDoc {
    doc_uid: String,
    objects: HashMap<String, String>,
    piping: HashMap<String, Port>,
    signal: HashMap<String, Port>,
    rels: Vec<Rel>,
}

struct Port {
    name: String,
    flow: String,
    nominal: String,
    parent: String,
}

struct Rel {
    def: String,
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
    println!(
        "{} rows={} cols={} tail={}",
        table.name,
        table.rows.len(),
        table.columns.len(),
        table.tail
    );
    for column in &table.columns {
        match column.kind {
            Kind::Char => println!(
                "  col {} char maxlen={} charset={:?} form={:?}",
                column.name, column.maxlen, column.charset, column.form
            ),
            Kind::Num => println!("  col {} number maxlen={}", column.name, column.maxlen),
        }
    }
}

fn report_piping(table: &Table, reference: &XmlDoc) {
    let wanted = [
        "SP_ID",
        "SP_PLANTITEMID",
        "PIPINGPOINTNUMBER",
        "FLOWDIRECTION",
        "POINTINDEX",
        "NOMINALDIAMETER",
    ];
    print_non_null(table, "table", &wanted);
    let mut dotted = 0usize;
    let mut by_uid: HashMap<String, usize> = HashMap::new();
    let mut plants = HashSet::new();
    let mut subset = Vec::new();
    for row in &table.rows {
        let id = text_at(row, &table.columns, "SP_ID");
        if id.contains('.') {
            dotted += 1;
        }
        let plant = text_at(row, &table.columns, "SP_PLANTITEMID");
        if !plant.is_empty() {
            plants.insert(plant.clone());
        }
        let Some(number) = int_at(row, &table.columns, "PIPINGPOINTNUMBER") else {
            continue;
        };
        if plant.is_empty() {
            continue;
        }
        let uid = format!("{plant}.{number}");
        *by_uid.entry(uid.clone()).or_default() += 1;
        if reference.piping.contains_key(&uid) {
            subset.push(row);
        }
    }
    let parent_hit = reference
        .piping
        .values()
        .filter(|port| plants.contains(&port.parent))
        .map(|port| port.parent.clone())
        .collect::<HashSet<_>>()
        .len();
    let unique_parents = reference
        .piping
        .values()
        .map(|port| port.parent.as_str())
        .collect::<HashSet<_>>()
        .len();
    println!(
        "piping dotted_sp_id={dotted} reconstructed={} reference={} missing={} duplicate_uids={}",
        by_uid.len(),
        reference.piping.len(),
        reference.piping.len().saturating_sub(subset.len()),
        by_uid.values().filter(|count| **count > 1).count()
    );
    println!("piping parents_in_SP_PLANTITEMID={parent_hit}/{unique_parents}");
    print_non_null_rows(table, "ijingchang", &subset, &wanted);
    score_ports(table, &subset, &reference.piping, "piping");
}

fn report_signal(table: &Table, reference: &XmlDoc) {
    let wanted = ["SP_ID", "SP_PLANTITEMID", "SIGNALPOINTNUMBER", "POINTINDEX"];
    print_non_null(table, "table", &wanted);
    let mut dotted = 0usize;
    let mut plants = HashSet::new();
    let mut subset = Vec::new();
    let mut reconstructed = 0usize;
    for row in &table.rows {
        let id = text_at(row, &table.columns, "SP_ID");
        if id.contains('.') {
            dotted += 1;
        }
        let plant = text_at(row, &table.columns, "SP_PLANTITEMID");
        if !plant.is_empty() {
            plants.insert(plant.clone());
        }
        let Some(number) = int_at(row, &table.columns, "SIGNALPOINTNUMBER") else {
            continue;
        };
        if plant.is_empty() {
            continue;
        }
        reconstructed += 1;
        let uid = format!("{plant}.{number}");
        if reference.signal.contains_key(&uid) {
            subset.push(row);
        }
    }
    let parent_hit = reference
        .signal
        .values()
        .filter(|port| plants.contains(&port.parent))
        .map(|port| port.parent.clone())
        .collect::<HashSet<_>>()
        .len();
    let unique_parents = reference
        .signal
        .values()
        .map(|port| port.parent.as_str())
        .collect::<HashSet<_>>()
        .len();
    println!(
        "signal dotted_sp_id={dotted} reconstructed={reconstructed} reference={} matched={} missing={}",
        reference.signal.len(),
        subset.len(),
        reference.signal.len().saturating_sub(subset.len())
    );
    println!("signal parents_in_SP_PLANTITEMID={parent_hit}/{unique_parents}");
    print_non_null_rows(table, "ijingchang", &subset, &wanted);
    score_ports(table, &subset, &reference.signal, "signal");
}

fn score_ports(table: &Table, rows: &[&Vec<Field>], ports: &HashMap<String, Port>, label: &str) {
    let number_col = if label == "piping" {
        "PIPINGPOINTNUMBER"
    } else {
        "SIGNALPOINTNUMBER"
    };
    let mut name_bad = 0usize;
    let mut parent_bad = 0usize;
    let mut index_diff = 0usize;
    let mut flow_match = 0usize;
    let mut flow_mismatch = 0usize;
    let mut flow_both_empty = 0usize;
    let mut flow_dump_only = 0usize;
    let mut flow_ref_only = 0usize;
    let mut nominal_pairs: BTreeMap<(i64, String), usize> = BTreeMap::new();
    let mut nominal_dump_null = 0usize;
    for row in rows {
        let plant = text_at(row, &table.columns, "SP_PLANTITEMID");
        let number = int_at(row, &table.columns, number_col).unwrap_or_default();
        let uid = format!("{plant}.{number}");
        let Some(port) = ports.get(&uid) else {
            continue;
        };
        if port.name != number.to_string() {
            name_bad += 1;
        }
        if port.parent != plant {
            parent_bad += 1;
        }
        if int_at(row, &table.columns, "POINTINDEX") != Some(number) {
            index_diff += 1;
        }
        if label == "piping" {
            let dump_flow = int_at(row, &table.columns, "FLOWDIRECTION");
            match (dump_flow, port.flow.as_str()) {
                (None, "") => flow_both_empty += 1,
                (Some(_), "") => flow_dump_only += 1,
                (None, _) => flow_ref_only += 1,
                (Some(value), expected) => {
                    let mapped = flow_guid(value).unwrap_or("");
                    if mapped.eq_ignore_ascii_case(expected) {
                        flow_match += 1;
                    } else {
                        flow_mismatch += 1;
                    }
                }
            }
            match int_at(row, &table.columns, "NOMINALDIAMETER") {
                Some(value) if !port.nominal.is_empty() => {
                    *nominal_pairs
                        .entry((value, port.nominal.clone()))
                        .or_default() += 1;
                }
                None if !port.nominal.is_empty() => nominal_dump_null += 1,
                _ => {}
            }
        }
    }
    println!("{label} matched_name_bad={name_bad} parent_bad={parent_bad} pointindex_ne_number={index_diff}");
    if label == "piping" {
        println!(
            "piping flow_match={flow_match} flow_mismatch={flow_mismatch} both_empty={flow_both_empty} dump_only={flow_dump_only} ref_only={flow_ref_only}"
        );
        let mut by_code: BTreeMap<i64, BTreeMap<&str, usize>> = BTreeMap::new();
        for ((code, text), count) in &nominal_pairs {
            *by_code
                .entry(*code)
                .or_default()
                .entry(text.as_str())
                .or_default() += *count;
        }
        let ambiguous = by_code.values().filter(|texts| texts.len() > 1).count();
        let mut rule_hit = 0usize;
        let mut rule_miss = 0usize;
        for ((code, text), count) in &nominal_pairs {
            match nominal_label(*code) {
                Some(label) if label == *text => rule_hit += *count,
                _ => rule_miss += *count,
            }
        }
        println!(
            "piping nominal_pairs={} ambiguous_codes={ambiguous} ref_nominal_but_dump_null={nominal_dump_null} rule_hit={rule_hit} rule_miss={rule_miss}",
            nominal_pairs.len()
        );
        for (code, texts) in &by_code {
            for (text, count) in texts {
                println!("  nominal_code {code} -> {text} {count}");
            }
        }
    }
}

fn report_relationships(table: &Table, reference: &XmlDoc, publish: &XmlDoc) {
    let wanted = ["SP_ID", "SP_DRAWINGID", "SP_ITEM1ID", "SP_ITEM2ID"];
    print_non_null(table, "table", &wanted);
    let mut drawings: BTreeMap<String, usize> = BTreeMap::new();
    let mut pairs: HashSet<(String, String)> = HashSet::new();
    let mut on_drawing = Vec::new();
    for row in &table.rows {
        let drawing = text_at(row, &table.columns, "SP_DRAWINGID");
        *drawings.entry(drawing.clone()).or_default() += 1;
        let left = text_at(row, &table.columns, "SP_ITEM1ID");
        let right = text_at(row, &table.columns, "SP_ITEM2ID");
        if !left.is_empty() && !right.is_empty() {
            pairs.insert((left, right));
        }
        if drawing == reference.doc_uid {
            on_drawing.push(row);
        }
    }
    println!(
        "relationship distinct_SP_DRAWINGID={} doc_rows={} doc_uid_present={}",
        drawings.len(),
        on_drawing.len(),
        drawings.contains_key(&reference.doc_uid)
    );
    print_non_null_rows(table, "ijingchang", &on_drawing, &wanted);

    let mut both_publish: BTreeMap<(String, String), usize> = BTreeMap::new();
    let mut one_publish = 0usize;
    let mut item2_null = 0usize;
    for row in &on_drawing {
        let left = text_at(row, &table.columns, "SP_ITEM1ID");
        let right = text_at(row, &table.columns, "SP_ITEM2ID");
        if right.is_empty() {
            item2_null += 1;
        }
        let left_class = publish.objects.get(&left);
        let right_class = publish.objects.get(&right);
        match (left_class, right_class) {
            (Some(left_class), Some(right_class)) => {
                *both_publish
                    .entry((left_class.clone(), right_class.clone()))
                    .or_default() += 1;
            }
            (Some(_), None) | (None, Some(_)) => one_publish += 1,
            (None, None) => {}
        }
    }
    let both_total: usize = both_publish.values().sum();
    println!(
        "relationship ijingchang both_ends_in_publish={both_total} one_end_in_publish={one_publish} item2_null={item2_null}"
    );
    for ((left, right), count) in &both_publish {
        println!("  publish_pair {left} -> {right} {count}");
    }

    for (def, label) in CATEGORIES {
        let rows: Vec<&Rel> = reference
            .rels
            .iter()
            .filter(|rel| rel.def == *def)
            .collect();
        let ordered = rows
            .iter()
            .filter(|rel| pairs.contains(&(rel.uid1.clone(), rel.uid2.clone())))
            .count();
        let swapped = rows
            .iter()
            .filter(|rel| pairs.contains(&(rel.uid2.clone(), rel.uid1.clone())))
            .count();
        let both_objects = rows
            .iter()
            .filter(|rel| {
                reference.objects.contains_key(&rel.uid1)
                    && reference.objects.contains_key(&rel.uid2)
            })
            .count();
        let both_published = rows
            .iter()
            .filter(|rel| {
                publish.objects.contains_key(&rel.uid1) && publish.objects.contains_key(&rel.uid2)
            })
            .count();
        let mut classes: BTreeMap<(String, String), usize> = BTreeMap::new();
        for rel in &rows {
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
            *classes.entry((left, right)).or_default() += 1;
        }
        println!(
            "category {label} def={def} reference={} dump_ordered={ordered} dump_swapped={swapped} both_in_reference_objects={both_objects} both_in_publish={both_published}",
            rows.len()
        );
        for ((left, right), count) in &classes {
            println!("  ref_ends {left} -> {right} {count}");
        }
    }
}

fn print_non_null(table: &Table, scope: &str, names: &[&str]) {
    let refs: Vec<&Vec<Field>> = table.rows.iter().collect();
    print_non_null_rows(table, scope, &refs, names);
}

fn print_non_null_rows(table: &Table, scope: &str, rows: &[&Vec<Field>], names: &[&str]) {
    let mut parts = vec![format!("{scope} n={}", rows.len())];
    for name in names {
        let count = rows
            .iter()
            .filter(|row| !matches!(field_at(row, &table.columns, name), Field::Null))
            .count();
        parts.push(format!("{name}={count}"));
    }
    println!("{} {}", table.name, parts.join(" "));
}

fn nominal_label(code: i64) -> Option<String> {
    if (1..5000).contains(&code) {
        return Some(format!("{code} mm"));
    }
    if code <= 5000 {
        return None;
    }
    let ticks = code - 5000;
    if (ticks * 100) % 32 != 0 {
        return None;
    }
    let hundredths = ticks * 100 / 32;
    let whole = hundredths / 100;
    let frac = hundredths % 100;
    let body = if frac == 0 {
        format!("{whole}")
    } else if frac % 10 == 0 {
        format!("{whole}.{}", frac / 10)
    } else {
        format!("{whole}.{frac:02}")
    };
    Some(format!("{body} in"))
}

fn flow_guid(value: i64) -> Option<&'static str> {
    match value {
        1 | 5 => Some("{23F94152-B87D-11D6-80D4-00A0C92487AF}"),
        2 => Some("{23F94146-B87D-11D6-80D4-00A0C92487AF}"),
        3 => Some("{23F9414C-B87D-11D6-80D4-00A0C92487AF}"),
        4 => Some("{BCB3F097-66CA-487B-9AF4-D8FCF18624A3}"),
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
        piping: HashMap::new(),
        signal: HashMap::new(),
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
    attach_parents(&mut doc);
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
        doc.rels.push(Rel {
            def: values.get("IRel.DefUID").cloned().unwrap_or_default(),
            uid1: values.get("IRel.UID1").cloned().unwrap_or_default(),
            uid2: values.get("IRel.UID2").cloned().unwrap_or_default(),
        });
        return;
    }
    let Some(uid) = values.get("IObject.UID") else {
        return;
    };
    doc.objects.insert(uid.clone(), tag.to_string());
    if tag == "PIDPipingPort" || tag == "PIDSignalPort" {
        let flow = values
            .get("IConnection.ConnectionFlowDirection")
            .map(|value| value.trim_start_matches('@').to_string())
            .unwrap_or_default();
        let port = Port {
            name: values.get("IObject.Name").cloned().unwrap_or_default(),
            flow,
            nominal: values
                .get("IPipeCrossSectionItem.NominalDiameter")
                .cloned()
                .unwrap_or_default(),
            parent: String::new(),
        };
        if tag == "PIDPipingPort" {
            doc.piping.insert(uid.clone(), port);
        } else {
            doc.signal.insert(uid.clone(), port);
        }
    }
}

fn attach_parents(doc: &mut XmlDoc) {
    for rel in &doc.rels {
        if rel.def == "PipingPortComposition" {
            if let Some(port) = doc.piping.get_mut(&rel.uid1) {
                port.parent = rel.uid2.clone();
            }
        } else if rel.def == "SignalPortComposition" {
            if let Some(port) = doc.signal.get_mut(&rel.uid1) {
                port.parent = rel.uid2.clone();
            }
        }
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
