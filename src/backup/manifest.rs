//! Parser for `Manifest.txt` inside a `SmartPlant` P&ID backup folder.
//!
//! Every line is `Key<<|>>Field1<<|>>Field2<<|>>…`. Keys are not
//! unique — `Table`, `View`, `File`, `FileSize`, `Role`, `Right`,
//! `SiteConnInfo`, `PlantConnInfo`, `DatabaseFiles` typically repeat
//! across many lines, while a small set of singleton keys
//! (`BackupType`, `Version`, `Name`, `Spid`, `Rootitem`, …) carry
//! backup-level metadata.
//!
//! This module is intentionally a *thin* parser: the in-memory
//! representation keeps every line as a generic
//! [`ManifestLine`] (key + fields), and exposes typed accessors
//! ([`Manifest::tables`], [`Manifest::views`]) for the structured
//! lookups callers will reach for first. Higher-level extractors
//! (roles, rights, files, conn info) can be added without
//! breaking the line-level model.
//!
//! # Tolerance
//!
//! `parse_manifest` is panic-safe on arbitrary input: empty file,
//! mixed line endings, lines without the `<<|>>` separator, blank
//! keys, and totally non-ASCII payload all map to a manifest with
//! zero or partial lines instead of unwinding.

/// Field separator used between every component on a manifest line.
///
/// Picked by `SmartPlant` so each field can contain spaces, commas,
/// and embedded quoting without escaping (the multi-character
/// sentinel makes accidental collisions implausible).
pub const FIELD_SEP: &str = "<<|>>";

/// One line of a `Manifest.txt` after splitting on [`FIELD_SEP`].
///
/// `key` is the first component; `fields` holds every subsequent
/// component in source order. A singleton key (`Name`, `Spid`,
/// `Version`, …) typically has exactly one field; multi-valued keys
/// (`Table`, `Role`, `Right`, `File`, …) carry several.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManifestLine {
    /// First field on the line — never empty in a successfully
    /// parsed line (lines whose key trims to empty are dropped).
    pub key: String,
    /// Every field after the key, in source order.
    pub fields: Vec<String>,
}

/// Whole `Manifest.txt` parsed into its line-level model.
///
/// Use [`Manifest::first`] / [`Manifest::all`] for raw lookups, or
/// the typed helpers ([`Manifest::tables`], [`Manifest::views`])
/// for the most common structured queries.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Manifest {
    /// Every successfully parsed line in source order.
    pub lines: Vec<ManifestLine>,
}

/// One database table referenced by the backup, recovered from a
/// `Table<<|>>db<<|>>name` line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManifestTable {
    /// Logical database name (e.g. `"TEST02pid"`).
    pub database: String,
    /// Bare table name (e.g. `"T_Drawing"`).
    pub name: String,
}

/// Parse a `Manifest.txt` body into [`Manifest`].
///
/// The function never panics: malformed lines are silently
/// dropped and an all-blank input returns an empty [`Manifest`].
pub fn parse_manifest(text: &str) -> Manifest {
    let lines = text.lines().filter_map(parse_line).collect();
    Manifest { lines }
}

fn parse_line(raw: &str) -> Option<ManifestLine> {
    let trimmed = raw.trim_end_matches('\r');
    if trimmed.is_empty() {
        return None;
    }
    let parts: Vec<&str> = trimmed.split(FIELD_SEP).collect();
    let (key_raw, fields_raw) = parts.split_first()?;
    let key = key_raw.trim().to_string();
    if key.is_empty() {
        return None;
    }
    Some(ManifestLine {
        key,
        fields: fields_raw.iter().map(|s| (*s).to_string()).collect(),
    })
}

impl Manifest {
    /// Return the first line whose key matches `key`.
    pub fn first(&self, key: &str) -> Option<&ManifestLine> {
        self.lines.iter().find(|line| line.key == key)
    }

    /// Iterate over every line whose key matches `key`, in source
    /// order. Cheap (just a filter) — chain `.collect()` if a `Vec`
    /// is needed.
    pub fn all<'a>(&'a self, key: &'a str) -> impl Iterator<Item = &'a ManifestLine> + 'a {
        self.lines.iter().filter(move |line| line.key == key)
    }

    /// Convenience: first field of the first line for `key`,
    /// borrowed as `&str`. Returns `None` when the key is missing
    /// or its line carries zero fields.
    pub fn first_field(&self, key: &str) -> Option<&str> {
        self.first(key)?.fields.first().map(String::as_str)
    }

    /// Collect every `Table<<|>>db<<|>>name` line into typed
    /// [`ManifestTable`] entries. Lines that do not carry both
    /// fields are skipped.
    pub fn tables(&self) -> Vec<ManifestTable> {
        self.all("Table")
            .filter_map(|line| {
                let database = line.fields.first()?.clone();
                let name = line.fields.get(1)?.clone();
                Some(ManifestTable { database, name })
            })
            .collect()
    }

    /// Same shape as [`Manifest::tables`] but for `View<<|>>db<<|>>name`.
    pub fn views(&self) -> Vec<ManifestTable> {
        self.all("View")
            .filter_map(|line| {
                let database = line.fields.first()?.clone();
                let name = line.fields.get(1)?.clone();
                Some(ManifestTable { database, name })
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_input_yields_empty_manifest() {
        let m = parse_manifest("");
        assert!(m.lines.is_empty());
        assert!(m.first("Name").is_none());
        assert!(m.tables().is_empty());
    }

    #[test]
    fn singleton_key_with_one_field() {
        let m = parse_manifest("Name<<|>>TEST02\n");
        assert_eq!(m.lines.len(), 1);
        assert_eq!(m.first_field("Name"), Some("TEST02"));
    }

    #[test]
    fn singleton_key_with_multiple_fields() {
        // Real `Rootitem` line carries a 10-field tail (UID, name,
        // empty slot, UNC path, kind, two flags, two empty slots,
        // creation timestamp).
        let line = "Rootitem<<|>>UID<<|>>TEST02<<|>><<|>>\\\\Mm-128\\spid\\TEST02<<|>>Plant<<|>>1<<|>>1<<|>><<|>><<|>>2026-01-01";
        let m = parse_manifest(line);
        let first = m.first("Rootitem").expect("present");
        assert_eq!(first.fields.len(), 10);
        assert_eq!(first.fields[0], "UID");
        assert_eq!(first.fields[1], "TEST02");
        assert_eq!(first.fields[2], "");
        assert_eq!(first.fields[3], "\\\\Mm-128\\spid\\TEST02");
        assert_eq!(first.fields[4], "Plant");
        assert_eq!(first.fields[7], "");
        assert_eq!(first.fields[9], "2026-01-01");
    }

    #[test]
    fn repeated_key_yields_multiple_lines() {
        let body = "Table<<|>>TEST02pid<<|>>T_Drawing\n\
                    Table<<|>>TEST02pid<<|>>T_Equipment\n\
                    Table<<|>>TEST02d<<|>>entities\n";
        let m = parse_manifest(body);
        let tables = m.tables();
        assert_eq!(tables.len(), 3);
        assert_eq!(tables[0].database, "TEST02pid");
        assert_eq!(tables[0].name, "T_Drawing");
        assert_eq!(tables[2].database, "TEST02d");
        assert_eq!(tables[2].name, "entities");
    }

    #[test]
    fn views_are_collected_separately_from_tables() {
        let body = "Table<<|>>TEST02pid<<|>>T_Drawing\n\
                    View<<|>>TEST02pid<<|>>V_GlobalTagList\n\
                    View<<|>>TEST02d<<|>>SPPIDentities\n";
        let m = parse_manifest(body);
        assert_eq!(m.tables().len(), 1);
        let views = m.views();
        assert_eq!(views.len(), 2);
        assert_eq!(views[0].name, "V_GlobalTagList");
        assert_eq!(views[1].database, "TEST02d");
    }

    #[test]
    fn windows_crlf_line_endings_are_handled() {
        let body = "BackupType<<|>>2\r\nVersion<<|>>7.02\r\n";
        let m = parse_manifest(body);
        assert_eq!(m.first_field("BackupType"), Some("2"));
        assert_eq!(m.first_field("Version"), Some("7.02"));
    }

    #[test]
    fn blank_lines_are_ignored() {
        let body = "\n\nName<<|>>TEST02\n\n\nVersion<<|>>7.02\n";
        let m = parse_manifest(body);
        assert_eq!(m.lines.len(), 2);
        assert_eq!(m.first_field("Name"), Some("TEST02"));
    }

    #[test]
    fn line_without_separator_keeps_key_and_zero_fields() {
        let m = parse_manifest("BackupCommand\n");
        let line = m.first("BackupCommand").expect("present");
        assert!(line.fields.is_empty());
    }

    #[test]
    fn empty_key_after_trim_is_dropped() {
        // A line that starts with the separator yields an empty key
        // (the leading `split` field is `""`); we drop it.
        let m = parse_manifest("<<|>>orphan-value\n   <<|>>still-orphan\n");
        assert!(m.lines.is_empty());
    }

    #[test]
    fn first_returns_only_the_first_match() {
        let body = "Foo<<|>>1\nFoo<<|>>2\n";
        let m = parse_manifest(body);
        assert_eq!(m.first_field("Foo"), Some("1"));
        let collected: Vec<_> = m.all("Foo").map(|l| l.fields[0].clone()).collect();
        assert_eq!(collected, vec!["1".to_string(), "2".to_string()]);
    }

    #[test]
    fn tables_skip_lines_missing_a_field() {
        // A `Table` line with only one field (no name) is skipped
        // by `tables()` rather than producing a struct with an
        // empty `name`.
        let body = "Table<<|>>TEST02pid<<|>>T_Drawing\n\
                    Table<<|>>TEST02pid\n\
                    Table<<|>>TEST02pid<<|>>T_Equipment\n";
        let m = parse_manifest(body);
        let tables = m.tables();
        assert_eq!(tables.len(), 2);
        assert_eq!(tables[0].name, "T_Drawing");
        assert_eq!(tables[1].name, "T_Equipment");
    }

    #[test]
    fn arbitrary_input_does_not_panic() {
        // Smoke test: a soup of separators, lone `<`, control bytes
        // (as far as `&str` allows — i.e. valid UTF-8), and very
        // long fields must all parse without unwinding.
        let body = "<<|>>\n\
                    \n\
                    K<<|>>\n\
                    K<<|>><<|>><<|>>\n\
                    A<<|>>B<<|>>C<<|>>D<<|>>E<<|>>F<<|>>G<<|>>H\n\
                    Long<<|>>";
        let mut long_value = String::with_capacity(8192);
        for _ in 0..1024 {
            long_value.push_str("ABCDEFGH");
        }
        let body = format!("{body}{long_value}\n");
        let m = parse_manifest(&body);
        // We don't care about the exact shape — just that parsing
        // returned without panicking and produced a finite vector.
        assert!(m.lines.len() <= 32);
    }

    #[test]
    fn real_test02_excerpt_round_trips_key_data() {
        // A trimmed slice of the real `TEST02_p/Manifest.txt`
        // header so the parser stays anchored to the on-disk
        // shape callers will hand us.
        let body = "BackupType<<|>>2\n\
                    Version<<|>>7.02\n\
                    DateCreated<<|>>04/20/2026 12:06:10\n\
                    Name<<|>>TEST02\n\
                    Spid<<|>>BB77101618824907AFAE785E6A863597\n\
                    BackupRefData<<|>>2\n\
                    ProjectType<<|>>0\n\
                    Serial_ID<<|>>260420120610\n\
                    PidIsAssociated<<|>>2\n\
                    Table<<|>>TEST02pid<<|>>T_Drawing\n\
                    Table<<|>>TEST02pid<<|>>T_Equipment\n\
                    View<<|>>TEST02pid<<|>>V_GlobalTagList\n";

        let m = parse_manifest(body);
        assert_eq!(m.first_field("BackupType"), Some("2"));
        assert_eq!(m.first_field("Version"), Some("7.02"));
        assert_eq!(m.first_field("Name"), Some("TEST02"));
        assert_eq!(
            m.first_field("Spid"),
            Some("BB77101618824907AFAE785E6A863597")
        );
        assert_eq!(m.tables().len(), 2);
        assert_eq!(m.views().len(), 1);
    }
}
