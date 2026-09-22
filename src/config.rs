//! Parse configuration shared by the public API and reader pipeline.
//!
//! Keeping these data-only types below [`crate::api`] prevents the CFB and
//! stream layers from depending back on the public facade. The facade
//! re-exports both types so existing callers can continue to use
//! [`crate::api::ParseOptions`] and [`crate::api::ParseProfile`].

/// High-level parse profile.
///
/// Which passes each profile runs is answered by the `runs_*` methods on
/// [`ParseOptions`], one per pass, so the reader asks "does this profile run
/// the sheet probes?" rather than "is this Light?" -- and a fourth profile
/// would be one more row in those methods, not one more boolean threaded
/// through the reader.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParseProfile {
    /// Full-fidelity parse. This is the default and preserves all current
    /// parser behavior.
    Full,
    /// Lightweight inventory/triage parse that skips expensive semantic and
    /// derived passes.
    Light,
    /// Everything a renderer needs to draw the sheet, and nothing that only
    /// an inspector or a reverse-engineering probe reads (plan
    /// `docs/plans/2026-09-21-a-geometry-parse-profile.md`). Runs the stream
    /// inventory, the tagged text (the page-size fallback reads the
    /// template name), the `JSite` pass (cached symbol bodies, their stroke
    /// styles, the parametric chain, the symbol paths), the cluster pass
    /// (every `Sheet*`'s record families, both censuses, the style tables),
    /// the dynamic attributes, the PSM tables (sheet layers and their
    /// display state), the sheet endpoint rescan, the object graph, the
    /// cross-reference and the geometry hints -- the last three because the
    /// connectivity links a renderer draws take their endpoint positions
    /// from them. Skips the sheet text / coordinate probes and the spatial
    /// analysis, the string scan, the object inventory, the document
    /// registry, `DocVersion2`, the summary information and the layout.
    /// On the corpus the Decoded and Inferred entities come out identical
    /// to `Full`'s; only `ProbeOnly` evidence is missing.
    Geometry,
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

    /// Build the [`ParseProfile::Geometry`] options: what a renderer parses a
    /// `.pid` with. XML and `JSite` properties stay on (the page-size
    /// fallback and the symbol paths come from them); the string scan and
    /// the unknown-stream diagnostics are off, since nothing drawn reads
    /// them.
    pub fn geometry() -> Self {
        Self {
            profile: ParseProfile::Geometry,
            scan_strings: false,
            parse_xml: true,
            parse_jsite_properties: true,
            keep_unknown_streams: false,
            ..Self::default()
        }
    }

    // The pass gates. Each answers for one pass of the reader pipeline, so
    // the pipeline never compares profiles itself.

    /// Whether the OLE `SummaryInformation` stream is decoded. Report
    /// material; a renderer draws nothing from it.
    #[must_use]
    pub fn runs_summary(&self) -> bool {
        self.profile != ParseProfile::Geometry
    }

    /// Whether the `TaggedTxtData` XML bodies are decoded. Off under `Light`
    /// regardless of `parse_xml`; the `Geometry` profile needs the drawing
    /// metadata's template name for the page-size fallback.
    #[must_use]
    pub fn runs_tagged_text(&self) -> bool {
        self.parse_xml && self.profile != ParseProfile::Light
    }

    /// Whether the `JSite*` storages are decoded -- the cached symbol
    /// bodies, their stroke styles, the parametric chain and the symbol
    /// paths all come from this pass.
    #[must_use]
    pub fn runs_jsites(&self) -> bool {
        self.parse_jsite_properties && self.profile != ParseProfile::Light
    }

    /// Whether each `Sheet*` is run through the heuristic text / coordinate
    /// probes and the spatial analysis. Their yield is `ProbeOnly` evidence
    /// and inferred points a renderer never draws; the record-family
    /// decoders and both censuses run regardless.
    #[must_use]
    pub fn runs_sheet_probes(&self) -> bool {
        self.profile != ParseProfile::Geometry
    }

    /// Whether the semantic passes run: dynamic attributes, PSM tables,
    /// the sheet endpoint rescan, the object graph, the cross-reference and
    /// the geometry hints. `Light` skips them all; `Geometry` needs every
    /// one -- the sheet layers and their display state come from the PSM
    /// tables, and the connectivity links' endpoint positions come from the
    /// geometry hints, which are scored against the object graph and the
    /// cross-reference's relationship links.
    #[must_use]
    pub fn runs_semantic_passes(&self) -> bool {
        self.profile != ParseProfile::Light
    }

    /// Whether the document registry (`AppObject`, `JTaggedTxtStgList`,
    /// version history) and `DocVersion2` are decoded. Audit material.
    #[must_use]
    pub fn runs_registry(&self) -> bool {
        self.profile == ParseProfile::Full
    }

    /// Whether the derived views are built: the object inventory and the
    /// layout model. Neither feeds the normalized geometry.
    #[must_use]
    pub fn runs_derived_passes(&self) -> bool {
        self.profile == ParseProfile::Full
    }
}
