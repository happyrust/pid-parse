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
//! * `--dump` — an Oracle `exp` dump. Its own data dictionary
//!   (`publish::placement::DumpDictionary`) places an item type's attribute
//!   in a table and a column, and each row of the breakdown gains that
//!   place.
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

use pid_parse::backup::{read_datamap, DataMap};
use pid_parse::publish::completeness::measure;
use pid_parse::publish::placement::{place_property, DumpDictionary, Placement};

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

/// `Item.Attribute`, with the conversion and the value list the map states,
/// and the place in the dump when one was given.
fn describe(map: &DataMap, placement: &Placement<'_>, dump_given: bool) -> String {
    let source = &placement.source;
    let item_type = placement.item_type().unwrap_or("?");
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
    if dump_given {
        match &placement.column {
            Ok(column) => text.push_str(&format!(" => {column}")),
            Err(unplaced) => text.push_str(&format!(" => ({unplaced})")),
        }
    }
    text
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
            Some(
                DumpDictionary::read(&dump)
                    .map_err(|error| format!("{}: {error}", path.display()))?,
            )
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
                if dictionary.place(item_type, &property.name).is_ok() {
                    property.targets.len()
                } else {
                    0
                }
            })
            .sum::<usize>();
        println!(
            "dump dictionary: {} item types, {} attributions; it places {placed} of the property rels in a table and column",
            dictionary.item_type_count(),
            dictionary.attribution_count()
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
    let no_dictionary = DumpDictionary::default();
    for ((class, interface, attribute), tally) in ordered.into_iter().take(shown) {
        let placements = place_property(
            &map,
            dictionary.as_ref().unwrap_or(&no_dictionary),
            class,
            attribute,
        );
        let rule = if placements
            .first()
            .is_some_and(|placement| placement.through_item_type)
        {
            " (through the item type; the rel names another class)"
        } else {
            ""
        };
        let from = if placements.is_empty() {
            "nothing in the map".to_string()
        } else {
            placements
                .iter()
                .map(|placement| describe(&map, placement, dictionary.is_some()))
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
