//! Parse configuration shared by the public API and reader pipeline.
//!
//! Keeping these data-only types below [`crate::api`] prevents the CFB and
//! stream layers from depending back on the public facade. The facade
//! re-exports both types so existing callers can continue to use
//! [`crate::api::ParseOptions`] and [`crate::api::ParseProfile`].

/// High-level parse profile.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParseProfile {
    /// Full-fidelity parse. This is the default and preserves all current
    /// parser behavior.
    Full,
    /// Lightweight inventory/triage parse that skips expensive semantic and
    /// derived passes.
    Light,
}

/// Tunables that control how aggressively [`crate::api::PidParser`] decodes a
/// `.pid`.
///
/// All fields default to "maximal fidelity" — full XML parse, full `JSite`
/// properties, and full unknown-stream retention. Shrink them when a bulk scan
/// only needs a subset of the model:
///
/// - `profile` — high-level full vs light parse profile.
/// - `scan_strings` — per-stream UTF-16 string probes.
/// - `parse_xml` — `SmartPlant`-embedded XML fragments.
/// - `parse_jsite_properties` — `JSite` dynamic property blobs.
/// - `keep_unknown_streams` — retain decoded diagnostics for unknown streams
///   (`crate::model::PidDocument::unknown_streams` and embedded `JSite`
///   raw-stream summaries). Package-side raw bytes are always retained for
///   writer passthrough.
/// - `max_preview_strings` — cap on the per-stream string preview collected
///   during scan.
#[derive(Debug, Clone)]
pub struct ParseOptions {
    /// High-level parse profile. [`ParseProfile::Full`] preserves existing
    /// behavior; [`ParseProfile::Light`] skips expensive semantic and derived
    /// passes for inventory-style callers.
    pub profile: ParseProfile,
    /// Enable per-stream UTF-16 / ASCII string probes.
    pub scan_strings: bool,
    /// Enable `SmartPlant`-embedded XML fragment decoding (`Drawing` /
    /// `General` metadata, rules, formats, …).
    pub parse_xml: bool,
    /// Enable decoding of `JSite` dynamic property blobs (can be expensive on
    /// big files with many sites).
    pub parse_jsite_properties: bool,
    /// Retain decoded diagnostics for streams that don't match any registered
    /// decoder. This does not control package-side raw byte retention;
    /// [`crate::writer::PidWriter`] passthrough remains byte-preserving even
    /// when this is `false`.
    pub keep_unknown_streams: bool,
    /// Upper bound on preview strings kept per stream during scans.
    pub max_preview_strings: usize,
}

impl Default for ParseOptions {
    fn default() -> Self {
        Self {
            profile: ParseProfile::Full,
            scan_strings: true,
            parse_xml: true,
            parse_jsite_properties: true,
            keep_unknown_streams: true,
            max_preview_strings: 64,
        }
    }
}

impl ParseOptions {
    /// Build an explicit light parse profile for bulk inventory / triage.
    ///
    /// This keeps stream inventory and package raw bytes, but disables XML
    /// body parsing and `JSite` property decoding by default. The reader also
    /// skips heavier semantic and derived passes while this profile is active.
    pub fn light() -> Self {
        Self {
            profile: ParseProfile::Light,
            parse_xml: false,
            parse_jsite_properties: false,
            max_preview_strings: 16,
            ..Self::default()
        }
    }
}
