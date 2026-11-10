//! Where the plant's data map says a published value comes from.
//!
//! A `SmartPlant` `_Data.xml` states its values under `(class, interface,
//! attribute)`. `SPPIDDataMap.xml` names, for a publish class and a schema
//! property, the P&ID item type and attribute the property is published
//! from, the conversion the vendor's adapter runs on it and the enum or unit
//! list its values come from. This probe joins the two for every non-empty
//! value of one or more reference documents.
//!
//! The join is on names alone: a value counts as named when the map
//! publishes some attribute as a property of that name on that class. How
//! the value is spelled (an enum's `@{GUID}`, a length's unit, a level of an
//! enum hierarchy) is not examined.
//!
//! ```text
//! probe_where_the_data_map_says_a_published_value_comes_from
//!     <SPPIDDataMap.xml | zip that holds it>
//!     [--ours DIR] [--dump Export.dmp] [--top N]
//!     <reference_Data.xml>...
//! ```
//!
//! * `--ours DIR` — the generated `_Data.xml` of the same file name in `DIR`
//!   for each reference. The counts are then split into the values that
//!   document states and the ones it does not, by the rule of
//!   `publish::completeness::measure`.
//! * `--dump` — an Oracle `exp` dump. Its own data dictionary (`ITEM`,
//!   `ITEMATTRIBUTIONS`, `UNIQUEATTS`, `ENTITIES`, `ATTRIBUTES`) places an
//!   item type's attribute in a table and a column, and each row of the
//!   breakdown gains that place.
//! * `--top N` — rows of the breakdown to print (40; 0 prints all).
//!
//! Two counts of "named" are printed. One takes the publish class from the
//! item type (`MapClassToClass`), which is how
//! `docs/analysis/2026-11-10-publish-clean-is-nothing-wrong-not-everything-stated.md`
//! counted; the other from the property rel itself (`IMapRel End2MapValue`).
//!
//! The figures this reproduces on 2026-11-10: over the eleven acceptance
//! references, 45 963 values of which the map names 40 208; over the seven
//! that have a dump of their revision, against the `--oracle-exp` outputs,
//! 19 437 values not stated of which the map names 14 949.
//!
//! Exit code 2 on a usage error, 1 when a file cannot be read, 0 otherwise.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use pid_parse::backup::{read_datamap, DataMap, MapSource};
use pid_parse::publish::completeness::measure;

/// What the references hold under one `(class, interface, attribute)`.
#[derive(Default, Clone, Copy)]
struct Tally {
    /// Non-empty values in the references.
    reference: usize,
    /// Of those, the values the generated documents state.
    stated: usize,
}

/// Values counted four ways: all of them, the ones the map names by each
/// rule, and the ones it names by neither.
#[derive(Default, Clone, Copy)]
struct Named {
    values: usize,
    by_item_type: usize,
    by_rel: usize,
    by_neither: usize,
}

impl Named {
    fn add(&mut self, values: usize, by_item_type: bool, by_rel: bool) {
        self.values += values;
        self.by_item_type += if by_item_type { values } else { 0 };
        self.by_rel += if by_rel { values } else { 0 };
        self.by_neither += if by_item_type || by_rel { 0 } else { values };
    }
}

fn percent(part: usize, whole: usize) -> String {
    if whole == 0 {
        return "-".to_string();
    }
    format!("{:.1} %", 100.0 * part as f64 / whole as f64)
}

/// The publish classes of every item type, each with the schema properties
/// some attribute of that item type is published as: the map's
/// `MapClassToClass` joined to its property rels through the owner.
fn named_by_item_type(map: &DataMap) -> HashMap<&str, HashSet<&str>> {
    let mut named: HashMap<&str, HashSet<&str>> = HashMap::new();
    for property in &map.properties {
        let Some(class) = map.class(&property.class_uid) else {
            continue;
        };
        for publish_class in &class.publish_classes {
            let properties = named.entry(publish_class.as_str()).or_default();
            for target in &property.targets {
                properties.insert(target.property.as_str());
            }
        }
    }
    named
}

/// The attributes of the item types published as `publish_class` that the
/// map publishes as `property`, whatever class the rel itself names.
fn sources_by_item_type<'a>(
    map: &'a DataMap,
    publish_class: &str,
    property: &str,
) -> Vec<MapSource<'a>> {
    let mut sources = Vec::new();
    for candidate in &map.properties {
        let Some(class) = map.class(&candidate.class_uid) else {
            continue;
        };
        if !class
            .publish_classes
            .iter()
            .any(|name| name == publish_class)
        {
            continue;
        }
        for target in &candidate.targets {
            if target.property == property {
                sources.push(MapSource {
                    class: Some(class),
                    property: candidate,
                    target,
                });
            }
        }
    }
    sources
}

/// `Item.Attribute`, with the conversion and the value list the map states.
fn describe(map: &DataMap, source: &MapSource<'_>, dictionary: Option<&Dictionary>) -> String {
    let item_type = source.class.map_or("?", |class| class.name.as_str());
    let mut text = format!("{item_type}.{}", source.property.name);
    let mut notes = Vec::new();
    match source.property.process_criteria.as_deref() {
        None | Some("") => {}
        Some(criteria) => notes.push(criteria.to_string()),
    }
    if let Some(list) = source.property.value_list.as_deref() {
        if let Some(name) = map.unit_list_names.get(list) {
            notes.push(format!("unit list {name}"));
        } else if let Some(name) = map.enum_list_names.get(list) {
            notes.push(format!("enum list {name}"));
        } else {
            notes.push(format!("list {list}"));
        }
    }
    if !source.target.edge.is_empty() {
        notes.push(format!("over {}", source.target.edge));
    }
    if !notes.is_empty() {
        text.push_str(&format!(" [{}]", notes.join("; ")));
    }
    if let Some(dictionary) = dictionary {
        text.push_str(&format!(
            " => {}",
            dictionary.place(item_type, &source.property.name)
        ));
    }
    text
}

/// The data dictionary an Oracle `exp` dump carries: which table and column
/// hold an item type's attribute.
struct Dictionary {
    /// `ITEM.NAME` → `ITEM.ID`.
    items: HashMap<String, String>,
    /// (`ITEMATTRIBUTIONS.ITEMID`, `NAME`) → `ATTRIBUTIONID`.
    attributions: HashMap<(String, String), String>,
    /// `UNIQUEATTS.ID` → (`ENTITY_NUMBER`, `ATTRIBUTE_NUMBER`).
    unique: HashMap<String, (String, String)>,
    /// `ENTITIES.ENTITY_NUMBER` → `ENTITY_NAME`.
    entities: HashMap<String, String>,
    /// `ATTRIBUTES.ATTRIBUTE_NUMBER` → `ATTRIBUTE_NAME`.
    attributes: HashMap<String, String>,
}

impl Dictionary {
    fn read(dump: &[u8]) -> Result<Self, String> {
        let column = |table: &ExpTable, name: &str| {
            table
                .columns
                .iter()
                .position(|column| column == name)
                .ok_or_else(|| format!("{}: no column {name}", table.name))
        };
        let pairs = |name: &str, key: &str, value: &str| -> Result<Vec<(String, String)>, String> {
            let table = ExpTable::read(dump, name)?;
            let (key, value) = (column(&table, key)?, column(&table, value)?);
            Ok(table
                .rows
                .iter()
                .filter_map(|row| Some((row[key].clone()?, row[value].clone()?)))
                .collect())
        };
        let items = pairs("ITEM", "NAME", "ID")?.into_iter().collect();
        let entities = pairs("ENTITIES", "ENTITY_NUMBER", "ENTITY_NAME")?
            .into_iter()
            .collect();
        let attributes = pairs("ATTRIBUTES", "ATTRIBUTE_NUMBER", "ATTRIBUTE_NAME")?
            .into_iter()
            .collect();
        let table = ExpTable::read(dump, "UNIQUEATTS")?;
        let (id, entity, attribute) = (
            column(&table, "ID")?,
            column(&table, "ENTITY_NUMBER")?,
            column(&table, "ATTRIBUTE_NUMBER")?,
        );
        let unique = table
            .rows
            .iter()
            .filter_map(|row| {
                Some((
                    row[id].clone()?,
                    (row[entity].clone()?, row[attribute].clone()?),
                ))
            })
            .collect();
        let table = ExpTable::read(dump, "ITEMATTRIBUTIONS")?;
        let (item, attribution, name) = (
            column(&table, "ITEMID")?,
            column(&table, "ATTRIBUTIONID")?,
            column(&table, "NAME")?,
        );
        let mut attributions = HashMap::new();
        for row in &table.rows {
            if let (Some(item), Some(attribution), Some(name)) = (
                row[item].clone(),
                row[attribution].clone(),
                row[name].clone(),
            ) {
                attributions.entry((item, name)).or_insert(attribution);
            }
        }
        Ok(Self {
            items,
            attributions,
            unique,
            entities,
            attributes,
        })
    }

    /// `T_Table.Column` of an item type's attribute, or why the dictionary
    /// does not place it.
    fn place(&self, item_type: &str, attribute: &str) -> String {
        let Some(item) = self.items.get(item_type) else {
            return format!("(the dictionary has no item type {item_type})");
        };
        let Some(attribution) = self
            .attributions
            .get(&(item.clone(), attribute.to_string()))
        else {
            return format!("(item type {item_type} has no attribution {attribute})");
        };
        let Some((entity, column)) = self.unique.get(attribution) else {
            return format!("(attribution {attribution} is not in UNIQUEATTS)");
        };
        format!(
            "{}.{}",
            self.entities.get(entity).map_or("?", String::as_str),
            self.attributes.get(column).map_or("?", String::as_str)
        )
    }

    fn places(&self, item_type: &str, attribute: &str) -> bool {
        self.items
            .get(item_type)
            .and_then(|item| {
                self.attributions
                    .get(&(item.clone(), attribute.to_string()))
            })
            .is_some_and(|attribution| self.unique.contains_key(attribution))
    }
}

/// The first `INSERT INTO "name"` of an Oracle `exp` dump with each cell as
/// text: national text as written, an integer `NUMBER` in decimal, and
/// `None` for NULL, a date, a fraction or database-charset text.
struct ExpTable {
    name: String,
    columns: Vec<String>,
    rows: Vec<Vec<Option<String>>>,
}

impl ExpTable {
    fn read(dump: &[u8], name: &str) -> Result<Self, String> {
        let fail = |detail: &str| format!("{name}: {detail}");
        let marker = format!("INSERT INTO \"{name}\"");
        let at = dump
            .windows(marker.len())
            .position(|window| window == marker.as_bytes())
            .ok_or_else(|| fail("no INSERT in the dump"))?;
        let line_end = dump[at..]
            .iter()
            .position(|byte| *byte == b'\n')
            .map(|offset| at + offset)
            .ok_or_else(|| fail("the INSERT line does not end"))?;
        let line = std::str::from_utf8(&dump[at..line_end])
            .map_err(|_| fail("the INSERT line is not ASCII"))?;
        let list = line
            .find('(')
            .zip(line.find(") VALUES"))
            .and_then(|(open, close)| line.get(open + 1..close))
            .ok_or_else(|| fail("the INSERT line has no column list"))?;
        let columns: Vec<String> = list
            .split(',')
            .map(|piece| piece.trim().trim_matches('"').to_string())
            .collect();
        let word = |pos: usize| {
            dump.get(pos..pos + 2)
                .map(|bytes| usize::from(u16::from_le_bytes([bytes[0], bytes[1]])))
                .ok_or_else(|| fail("the table is cut short"))
        };
        let mut pos = line_end + 1;
        if word(pos)? != columns.len() {
            return Err(fail(
                "the header and the INSERT line disagree on the columns",
            ));
        }
        pos += 2;
        // Per column: is it national text (charset 2000, form 2), is it a number.
        let mut kinds = Vec::with_capacity(columns.len());
        for _ in &columns {
            let kind = word(pos)?;
            pos += 2;
            match kind {
                1 => {
                    let (charset, form) = (word(pos + 2)?, word(pos + 4)?);
                    pos += 6;
                    kinds.push((charset == 2000 && form == 2, false));
                }
                2 | 12 => {
                    pos += 2;
                    kinds.push((false, kind == 2));
                }
                _ => return Err(fail("a column type this probe does not read")),
            }
        }
        if dump.get(pos..pos + 4) != Some(&[0, 0, 0, 0]) {
            return Err(fail("the column header does not end"));
        }
        pos += 4;
        let mut rows = Vec::new();
        while word(pos)? != 0xFFFF {
            let mut row = Vec::with_capacity(columns.len());
            for (text, number) in &kinds {
                let len = word(pos)?;
                pos += 2;
                if len == 0xFFFE {
                    row.push(None);
                    continue;
                }
                let raw = dump
                    .get(pos..pos + len)
                    .ok_or_else(|| fail("a cell runs past the dump"))?;
                pos += len;
                row.push(if *text {
                    let (pairs, _) = raw.as_chunks::<2>();
                    let units: Vec<u16> =
                        pairs.iter().map(|pair| u16::from_be_bytes(*pair)).collect();
                    Some(String::from_utf16_lossy(&units))
                } else if *number {
                    oracle_integer(raw).map(|value| value.to_string())
                } else {
                    None
                });
            }
            if dump.get(pos..pos + 2) != Some(&[0, 0]) {
                return Err(fail("a row does not end"));
            }
            pos += 2;
            rows.push(row);
        }
        Ok(Self {
            name: name.to_string(),
            columns,
            rows,
        })
    }
}

/// An Oracle `NUMBER` that is a non-negative integer: `80` is 0, `c1 02` is
/// 1, `c2 1a 34` is 2551. Anything else is `None`.
fn oracle_integer(raw: &[u8]) -> Option<u64> {
    let (&first, digits) = raw.split_first()?;
    if first == 0x80 && digits.is_empty() {
        return Some(0);
    }
    let exponent = usize::from(first.checked_sub(193)?);
    if digits.len() > exponent + 1 {
        return None;
    }
    let mut total: u64 = 0;
    for index in 0..=exponent {
        let digit = match digits.get(index) {
            Some(byte) => u64::from(byte.checked_sub(1).filter(|digit| *digit < 100)?),
            None => 0,
        };
        total = total.checked_mul(100)?.checked_add(digit)?;
    }
    Some(total)
}

struct Arguments {
    map: PathBuf,
    references: Vec<PathBuf>,
    ours: Option<PathBuf>,
    dump: Option<PathBuf>,
    top: usize,
}

fn arguments() -> Result<Arguments, String> {
    let mut positional = Vec::new();
    let mut ours = None;
    let mut dump = None;
    let mut top = 40;
    let mut args = std::env::args_os().skip(1);
    while let Some(arg) = args.next() {
        match arg.to_str() {
            Some("--ours") => {
                ours = Some(PathBuf::from(
                    args.next().ok_or("--ours takes a directory")?,
                ))
            }
            Some("--dump") => dump = Some(PathBuf::from(args.next().ok_or("--dump takes a file")?)),
            Some("--top") => {
                top = args
                    .next()
                    .and_then(|value| value.to_str().and_then(|text| text.parse().ok()))
                    .ok_or("--top takes a number")?;
            }
            Some(flag) if flag.starts_with("--") => return Err(format!("unknown option {flag}")),
            _ => positional.push(PathBuf::from(arg)),
        }
    }
    let mut positional = positional.into_iter();
    let map = positional.next().ok_or("no data map given")?;
    let references: Vec<PathBuf> = positional.collect();
    if references.is_empty() {
        return Err("no reference _Data.xml given".to_string());
    }
    Ok(Arguments {
        map,
        references,
        ours,
        dump,
        top,
    })
}

fn read_text(path: &Path) -> Result<String, String> {
    std::fs::read_to_string(path).map_err(|error| format!("{}: {error}", path.display()))
}

fn run(arguments: &Arguments) -> Result<(), String> {
    let map = read_datamap(&arguments.map)
        .map_err(|error| format!("{}: {error}", arguments.map.display()))?;
    let dictionary = match &arguments.dump {
        None => None,
        Some(path) => {
            let dump =
                std::fs::read(path).map_err(|error| format!("{}: {error}", path.display()))?;
            Some(Dictionary::read(&dump).map_err(|error| format!("{}: {error}", path.display()))?)
        }
    };
    println!(
        "data map {}: {} item types, {} attributes, {} of them published by {} property rels",
        arguments.map.display(),
        map.classes.len(),
        map.properties.len(),
        map.properties
            .iter()
            .filter(|property| !property.targets.is_empty())
            .count(),
        map.properties
            .iter()
            .map(|property| property.targets.len())
            .sum::<usize>()
    );
    if let Some(dictionary) = &dictionary {
        let placed = map
            .properties
            .iter()
            .map(|property| {
                let item_type = map
                    .class(&property.class_uid)
                    .map_or("?", |class| class.name.as_str());
                if dictionary.places(item_type, &property.name) {
                    property.targets.len()
                } else {
                    0
                }
            })
            .sum::<usize>();
        println!(
            "dump dictionary: {} item types, {} attributions; it places {placed} of the property rels in a table and column",
            dictionary.items.len(),
            dictionary.attributions.len()
        );
    }

    let by_item_type = named_by_item_type(&map);
    let mut rows: BTreeMap<(String, String, String), Tally> = BTreeMap::new();
    let mut all = Named::default();
    let mut stated = Named::default();
    let mut not_stated = Named::default();
    println!();
    for reference in &arguments.references {
        let reference_xml = read_text(reference)?;
        let file_name = reference
            .file_name()
            .ok_or_else(|| format!("{}: not a file", reference.display()))?;
        let ours_xml = match &arguments.ours {
            None => None,
            Some(directory) => Some(read_text(&directory.join(file_name))?),
        };
        let measured = measure(
            ours_xml.as_deref().unwrap_or(&reference_xml),
            &reference_xml,
        )
        .map_err(|error| format!("{}: {error}", reference.display()))?;
        let mut file_all = Named::default();
        let mut file_not_stated = Named::default();
        for ((class, interface, attribute), share) in &measured.values_by_attribute {
            let item_type = by_item_type
                .get(class.as_str())
                .is_some_and(|properties| properties.contains(attribute.as_str()));
            let rel = !map.sources(class, attribute).is_empty();
            file_all.add(share.reference, item_type, rel);
            all.add(share.reference, item_type, rel);
            stated.add(share.stated, item_type, rel);
            file_not_stated.add(share.missing(), item_type, rel);
            not_stated.add(share.missing(), item_type, rel);
            let row = rows
                .entry((class.clone(), interface.clone(), attribute.clone()))
                .or_default();
            row.reference += share.reference;
            row.stated += share.stated;
        }
        let name = file_name.to_string_lossy();
        print!(
            "{name}: {} values, the map names {} ({})",
            file_all.values,
            file_all.by_item_type,
            percent(file_all.by_item_type, file_all.values)
        );
        if ours_xml.is_some() {
            print!(
                "; not stated {}, the map names {} ({})",
                file_not_stated.values,
                file_not_stated.by_item_type,
                percent(file_not_stated.by_item_type, file_not_stated.values)
            );
        }
        println!();
    }

    println!();
    let count = arguments.references.len();
    println!(
        "{count} {}: {} non-empty values; the map names {} ({}) by the item type's publish class, {} ({}) by the class the rel names; it names {} of them neither way",
        if count == 1 { "reference" } else { "references" },
        all.values,
        all.by_item_type,
        percent(all.by_item_type, all.values),
        all.by_rel,
        percent(all.by_rel, all.values),
        all.by_neither
    );
    if arguments.ours.is_some() {
        println!(
            "stated by the generated documents: {} ({} of all), of which the map names {} ({})",
            stated.values,
            percent(stated.values, all.values),
            stated.by_item_type,
            percent(stated.by_item_type, stated.values)
        );
        println!(
            "not stated: {}, of which the map names {} ({}) by the item type's publish class, {} ({}) by the class the rel names; named by neither {}",
            not_stated.values,
            not_stated.by_item_type,
            percent(not_stated.by_item_type, not_stated.values),
            not_stated.by_rel,
            percent(not_stated.by_rel, not_stated.values),
            not_stated.by_neither
        );
    }

    // The breakdown: what is not stated first when there is a generated side,
    // the most frequent first otherwise.
    let weight = |tally: &Tally| {
        if arguments.ours.is_some() {
            tally.reference - tally.stated
        } else {
            tally.reference
        }
    };
    let mut ordered: Vec<(&(String, String, String), &Tally)> = rows.iter().collect();
    ordered.sort_by(|left, right| {
        weight(right.1)
            .cmp(&weight(left.1))
            .then(left.0.cmp(right.0))
    });
    let shown = if arguments.top == 0 {
        ordered.len()
    } else {
        arguments.top.min(ordered.len())
    };
    println!();
    println!(
        "{} of {} (class, interface, attribute) rows{}:",
        shown,
        ordered.len(),
        if arguments.ours.is_some() {
            ", by values not stated"
        } else {
            ", by values"
        }
    );
    for ((class, interface, attribute), tally) in ordered.into_iter().take(shown) {
        let mut sources = map.sources(class, attribute);
        let mut rule = "";
        if sources.is_empty() {
            sources = sources_by_item_type(&map, class, attribute);
            if !sources.is_empty() {
                rule = " (through the item type; the rel names another class)";
            }
        }
        let from = if sources.is_empty() {
            "nothing in the map".to_string()
        } else {
            sources
                .iter()
                .map(|source| describe(&map, source, dictionary.as_ref()))
                .collect::<Vec<_>>()
                .join(" | ")
        };
        let counts = if arguments.ours.is_some() {
            format!(
                "{:>6} not stated of {:>6}",
                tally.reference - tally.stated,
                tally.reference
            )
        } else {
            format!("{:>6}", tally.reference)
        };
        println!("  {counts}  {class} {interface}.{attribute}  <- {from}{rule}");
    }
    Ok(())
}

fn main() -> ExitCode {
    let arguments = match arguments() {
        Ok(arguments) => arguments,
        Err(error) => {
            eprintln!("{error}");
            eprintln!(
                "usage: probe_where_the_data_map_says_a_published_value_comes_from \
                 <SPPIDDataMap.xml | zip> [--ours DIR] [--dump Export.dmp] [--top N] \
                 <reference_Data.xml>..."
            );
            return ExitCode::from(2);
        }
    };
    match run(&arguments) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::from(1)
        }
    }
}
