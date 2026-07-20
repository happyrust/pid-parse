//! Shared XML formatting, ordering, and field-normalization helpers.

use std::collections::HashMap;

use super::super::catalog;
use super::super::model::{
    PublishDrawing, PublishError, PublishObject, PublishRepresentation, PublishStyle,
};

pub(super) const CONTAINER_SCOPE: &str = "Data";
pub(super) const CONTAINER_SOFTWARE_VERSION: &str = "10.00.31.0023";
pub(super) const CONTAINER_SCHEMA_VERSION: &str = "04.02.17.01";
pub(super) const CONTAINER_TOOL_ID: &str = "SMARTPLANTPID";
pub(super) const CONTAINER_TOOL_SIGNATURE: &str = "AAAD";
pub(super) const CONTAINER_SDECIMAL: &str = ".";
pub(super) fn ordered_publishable_representations(
    drawing: &PublishDrawing,
) -> Vec<&PublishRepresentation> {
    let mut reps: Vec<&PublishRepresentation> = drawing
        .representations
        .iter()
        .filter(|rep| representation_is_publishable(rep))
        .collect();
    if matches!(drawing.style, PublishStyle::A01) {
        let rank_by_object: HashMap<&str, u8> = drawing
            .objects
            .iter()
            .map(|obj| {
                (
                    obj.uid.as_str(),
                    a01_object_rank(obj.item_type_name.as_str()),
                )
            })
            .collect();
        reps.sort_by(|a, b| {
            let a_rank = a
                .model_item_uid
                .as_deref()
                .and_then(|uid| rank_by_object.get(uid).copied())
                .unwrap_or(u8::MAX);
            let b_rank = b
                .model_item_uid
                .as_deref()
                .and_then(|uid| rank_by_object.get(uid).copied())
                .unwrap_or(u8::MAX);
            a_rank
                .cmp(&b_rank)
                .then_with(|| b.graphic_oid.cmp(&a.graphic_oid))
                .then_with(|| a.uid.cmp(&b.uid))
        });
    }
    reps
}

pub(super) fn a01_object_rank(item_type_name: &str) -> u8 {
    catalog::a01_rank(item_type_name)
}
pub(super) fn non_empty_field<'a>(obj: &'a PublishObject, key: &str) -> Option<&'a str> {
    obj.fields
        .get(key)
        .map(String::as_str)
        .filter(|s| !s.trim().is_empty())
}

pub(super) fn non_empty_field_any<'a>(obj: &'a PublishObject, keys: &[&str]) -> Option<&'a str> {
    keys.iter().find_map(|key| non_empty_field(obj, key))
}

pub(super) fn dwg_field_with_aliases<'a>(
    obj: &'a PublishObject,
    style: PublishStyle,
    canonical_key: &str,
    dwg_aliases: &[&str],
) -> Option<&'a str> {
    non_empty_field(obj, canonical_key).or_else(|| {
        if matches!(style, PublishStyle::Dwg) {
            non_empty_field_any(obj, dwg_aliases)
        } else {
            None
        }
    })
}

pub(super) fn canonical_construction_status(obj: &PublishObject, style: PublishStyle) -> String {
    match (
        style,
        obj.fields.get("ConstructionStatus").map(|s| s.trim()),
    ) {
        (PublishStyle::A01, None | Some("") | Some("2")) => "@NewConstruction".to_string(),
        (_, Some(value)) => value.to_string(),
        (_, None) => "@NewConstruction".to_string(),
    }
}

pub(super) fn canonical_construction_status2(obj: &PublishObject, style: PublishStyle) -> String {
    match (
        style,
        obj.fields.get("ConstructionStatus2").map(|s| s.trim()),
    ) {
        (PublishStyle::A01, None | Some("")) => {
            "@{78398AB4-9F3D-11D6-BDA7-00104BCC2B69}".to_string()
        }
        (_, Some(value)) => value.to_string(),
        (_, None) => "@{78398AB4-9F3D-11D6-BDA7-00104BCC2B69}".to_string(),
    }
}

pub(super) fn canonical_is_typical(obj: &PublishObject, style: PublishStyle) -> &'static str {
    match style {
        PublishStyle::A01 => "False",
        PublishStyle::Dwg => obj.is_typical.as_deref().map_or("False", map_bool),
    }
}
/// Append a `" mm"` suffix to a bare numeric diameter so the XML
/// matches `SmartPlant`'s canonical "250 mm" form. If the value
/// already carries a unit we leave it alone.
pub(super) fn format_diameter(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    if trimmed.chars().any(|c| c.is_ascii_alphabetic()) {
        return trimmed.to_string();
    }
    format!("{trimmed} mm")
}

pub(super) fn format_insulation_inches(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    if let Some(number) = trimmed.strip_suffix('"').map(str::trim) {
        if let Ok(value) = number.parse::<f64>() {
            return format!("{value:.3} in");
        }
    }
    trimmed.to_string()
}

/// Map a SPPID boolean string ("1" / "0" / "") to the XML form
/// `SmartPlant` uses ("True" / "False").
pub(super) fn map_bool(value: &str) -> &'static str {
    match value.trim() {
        "1" | "True" | "true" => "True",
        _ => "False",
    }
}

/// Convert a `std::fmt::Error` into a [`PublishError`] so the
/// writer's `?`-operator chain stays uniform with the `SQLite`
/// loader's.
pub(super) fn fmt_err(err: std::fmt::Error) -> PublishError {
    PublishError::Sqlite(format!("format: {err}"))
}
/// True when a representation row deserves a `<PIDRepresentation>`
/// element in the published XML. `SmartPlant`'s exporter skips the
/// pure annotation / label rows — those whose `T_Representation.SP_ModelItemID`
/// is NULL or empty (typically `\Equipment\Labels - ...` and
/// `\Piping\Labels - ...` symbols). Mirroring that filter is what
/// keeps the A01 diff in lockstep with the reference fixture.
///
/// The check is centralized so `write_representations` and the
/// derived `<Rel>` emitters share the exact same predicate; the
/// derived `DwgRepresentationComposition` rel was already
/// naturally filtered (its source IS `model_item_uid`), but the
/// `DrawingItems` rel was not — A14 brings them into alignment.
pub(super) fn representation_is_publishable(rep: &PublishRepresentation) -> bool {
    matches!(rep.model_item_uid.as_deref(), Some(uid) if !uid.is_empty())
}
/// XML attribute-value escape. `SmartPlant` uses double-quote
/// delimiters so we only need to escape the five canonical
/// entities plus CR/LF (which SPPID stores verbatim as
/// `&#13;&#10;` inside attribute values).
pub(super) fn escape_attr(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            '\r' => out.push_str("&#13;"),
            '\n' => out.push_str("&#10;"),
            _ => out.push(c),
        }
    }
    out
}
