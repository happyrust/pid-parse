//! The reason catalog: every branch of the export path that draws nothing,
//! named once.
//!
//! [`DropReason`] is a closed set of 43 codes, one per row of appendix A of
//! `.kiro/specs/pdf-export-ledger/requirements.md`. The declaration order is
//! that appendix's row order (A.1 → A.2 → A.3 → A.4 → A.5 → A.8), so the
//! derived [`Ord`] **is** "where the code sits in the appendix" — which is what
//! requirement 1.5 and 5.3 mean by "the first one in appendix A" (`min()`).
//!
//! Each code is one of two tiers ([`Tier`]): a *suspect* drop may be a coverage
//! gap and is ratcheted down only, while an *intentional* drop is what
//! `SmartPlant` itself or the file's own declaration says not to draw, and is
//! pinned exactly.
//!
//! Three things that are counted but are **not** drops live in their own types:
//! [`PartialMark`] (a primitive that reached the page incompletely),
//! [`Asymmetry`] (the furniture domain drawing something the drawing domain
//! would have dropped) and [`PlacementSource`] (where a symbol body came from).

use serde::{Deserialize, Serialize};

/// Which of the two accounts a drop belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Tier {
    /// Possibly a coverage gap: a decoder refused something, a shape was
    /// rejected, a pen width was zero. Pinned at its present value and allowed
    /// only to fall.
    Suspect,
    /// What `SmartPlant`'s own plot rules or the file's own declaration says
    /// not to draw: a hidden layer, the `ConsistencyChecks` overlay, a record
    /// carrying the native skip bit. Pinned exactly.
    Intentional,
}

impl Tier {
    /// The tier's name, as the ledger and the gate print it.
    #[must_use]
    pub fn code(self) -> &'static str {
        match self {
            Self::Suspect => "suspect",
            Self::Intentional => "intentional",
        }
    }
}

/// The layer of the pipeline a drop happened at.
///
/// The first four map one-to-one onto the four conservation equations; the
/// last two are the page facts and the text normalizer, which have no
/// conservation equation of their own.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReasonLayer {
    /// A record that never became an entity (A.1).
    Parse,
    /// An entity that produced no page primitive (A.2).
    Entity,
    /// A symbol-body primitive that produced no page primitive (A.3).
    Body,
    /// A page primitive the writer did not draw (A.4's `off_sheet` and A.5).
    Primitive,
    /// A stroke of a primitive the writer did not draw (A.5's `segment_*`).
    Segment,
    /// A character or a line the text normalizer removed (A.4).
    TextNormalization,
    /// A page-level fact: a template that could not be read, or one that no
    /// file answers (A.8).
    Page,
}

/// A drop that has a name, in appendix A's order.
///
/// Every variant's rustdoc states its criterion and where in the code that
/// criterion is written, because the value of this enum is that a reader can
/// go from a count back to the branch that produced it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DropReason {
    // ---------------------------------------------------------------- A.1
    /// A graphic-class record whose type code has no decoder at all
    /// (`NormalizedPidGeometry::dropped_graphic_records`).
    RecordNoDecoder,
    /// A record whose own family's decoder walked over it and refused its
    /// shape (`NormalizedPidGeometry::refused_graphic_records`).
    RecordRefused,
    /// A record the file itself retired: its type word carries the `0x8000`
    /// native-skip bit `PSMSerializeIn` tests before reading the oid
    /// (`NormalizedPidGeometry::skipped_graphic_records`).
    RecordNativeSkip,
    /// A `Sheet*` stream whose geometry is `None` — the stream could not be
    /// opened, or no family decoded a record from it and the probe found
    /// nothing. Its three censuses are lost with it in
    /// `sheet_geometry_from_probe` (`src/streams/cluster.rs`).
    SheetStreamWithoutGeometry,
    /// A record inside a nested `JSite*/PSMcluster0` storage: symbol-body
    /// content, drawn through the placement that references it rather than as
    /// page content of its own.
    RecordSymbolBodyCache,
    /// A cached symbol definition no placement references — a parametric
    /// symbol's template body, for instance — so its records reach no page.
    CacheDefinitionUnplaced,
    /// A decoded record whose family registers a deliberate no-op emitter in
    /// `EMITTERS`: `igSmartFrame2d`, `DependencyObject`, the `0x0010`
    /// sub-records, the attribute fragments, `igRectangle2d`, `igDimension`.
    RecordAuditOnlyFamily,
    /// A decoded record whose byte range falls outside the stream it was read
    /// from: each emitter's `source_range(..)` returned `None` and it
    /// `continue`d.
    RecordEmitterOutOfRange,
    /// A decoded record the emitter refused on shape: `igLineString2d` with
    /// unequal x/y vertex counts or no vertex at all, `igBoundary2d` with
    /// fewer than three segments, `igBspCurve2d` sampled to fewer than two
    /// points.
    RecordEmitterShapeRejected,

    // ---------------------------------------------------------------- A.2
    /// `suppressed_from_plot`: the entity's source layer says
    /// `displayed == Some(false)`. Read at both call sites — the drawing
    /// domain's main loop and `push_entity_with`.
    LayerHidden,
    /// `suppressed_from_plot`: the entity's source layer is named
    /// `ConsistencyChecks`, the authored review overlay every reference plot
    /// omits.
    LayerConsistencyChecks,
    /// The drawing domain's main loop: `record_kind` is
    /// `SheetRecordKind::EndpointPair`, a guess that a pair of doubles are the
    /// endpoints of a line. Counted as `PaperModel::skipped_endpoint_pairs`
    /// too.
    EndpointPairGuess,
    /// A branch the routing cannot reach: `push_entity_with` handed a
    /// `SymbolInstance`, or `push_symbol_instance` handed something else.
    /// Should be zero; counted so that a routing change that makes it
    /// reachable is visible.
    KindMisrouted,
    /// A drawing-domain `Line` on a declared page whose one axis is within
    /// 1 mm of the sheet edge and whose other axis spans more than 0.9 of the
    /// page — the frame rail a border template would have drawn.
    LineOnSheetEdge,
    /// An `igBoundary2d` ring whose quantized-vertex-plus-colour key is
    /// already in `seen_sheet_fills`. That set is shared by the drawing and
    /// furniture domains, so the first taker may be either.
    FillDuplicate,
    /// A `Point` whose line style did not resolve, whose style carries no
    /// marker, or which has no marker at all (requirement deviation D2: "no
    /// marker" is this code; "a marker whose strokes are all degenerate" is
    /// [`Self::PointMarkerDegenerate`]).
    PointNoMarker,
    /// A `Point` with a marker whose every stroke is degenerate
    /// (`PointMarker::draws()` is false while a marker exists).
    PointMarkerDegenerate,
    /// `PidGraphicKind::Annotation`. No emitter produces one today, so this
    /// should be zero; counted so that an emitter that starts producing one is
    /// visible rather than silently undrawn.
    KindAnnotation,
    /// `PidGraphicKind::Unknown`: probe evidence and `JStyleOverride` records,
    /// which carry byte provenance but no reading a page could draw.
    KindUnknown,
    /// A placement that names a symbol path no library answers — cached body
    /// and library body both absent. The writer draws no placeholder box; the
    /// count is `PaperModel::missing_boxes` too.
    SymbolMissing,
    /// A placement with neither a symbol path nor a resolvable cached body.
    /// Silent until the ledger: nothing anywhere counted it.
    SymbolUnresolvable,
    /// A placement whose body resolved but every one of its body primitives
    /// dropped under A.3, so the placement drew nothing.
    SymbolBodyEmpty,

    // ---------------------------------------------------------------- A.3
    /// A body primitive filtered out by its layer: the cached body's
    /// `visible_styled`, or the library body's `displayed_primitives` (an
    /// illegal view direction, or a layer the file does not display).
    BodyLayerHidden,
    /// A body text whose whole run is the `NULL` placeholder
    /// (`symbol_text_is_null_placeholder`).
    BodyNullPlaceholder,
    /// A body primitive that lands as a `Line` on the sheet edge
    /// (`push_symbol_primitive`'s `sheet_edge`); the furniture domain's
    /// placements take the same path.
    BodyLineOnSheetEdge,
    /// A `Boundary` member circle whose placed radius is at most 1e-4 mm, so
    /// it fills nothing.
    BodyBoundaryCircleDegenerate,
    /// A `Boundary` member that is neither a `Circle` nor a `Line`. The reader
    /// declares this cannot occur; should be zero.
    BodyBoundaryMemberUnsupported,

    // ---------------------------------------------------------------- A.4
    /// A page primitive removed by the layout's final `retain` because
    /// `beyond_the_sheet` refused it: non-finite points, a span past
    /// `SHEET_REACH` page diagonals, or wholly off the sheet. Counted as
    /// `PaperModel::off_sheet_primitives` too.
    OffSheet,
    /// A control character `label_line_ranges` removed, counted per character.
    /// A tab turned into a space is a substitution, not a drop, and is not
    /// counted.
    TextControlChar,
    /// A trailing empty line `label_line_ranges` popped, counted per line.
    TextTrailingEmptyLine,

    // ---------------------------------------------------------------- A.5
    /// `push_layer` draws only the `decoded` and `inferred` confidence layers
    /// (`is_plotted`); probe-only evidence stays in the model and off the page.
    ProbeOnly,
    /// `paint_primitive`: a `Fill` ring with fewer than three points, or with
    /// a non-finite coordinate.
    FillDegenerate,
    /// `paint_primitive`: a pen width that is non-finite or not positive. The
    /// default pen used when no line style resolved is not this.
    PenWidthInvalid,
    /// `shape_chain` yielded fewer than two points: a polyline with under two
    /// vertices, or an arc or circle whose `arc_points` came back empty
    /// (radius at most zero, non-finite, or a sweep under 1e-8). The pen state
    /// is still written to the PDF.
    ChainEmpty,
    /// `stroke_runs`: a segment endpoint that is not finite, counted per
    /// segment.
    SegmentNonFinite,
    /// `stroke_runs`: a segment shorter than 1e-4 mm, counted per segment.
    SegmentTooShort,
    /// `stroke_runs`: a non-furniture segment `read::sheet_edge_mm` calls on
    /// the sheet edge — within 0.282 mm on one axis and past 0.9 of the page
    /// on the other. Counted per segment.
    SegmentOnSheetEdge,
    /// `show_text`: the text is the empty string.
    TextEmpty,
    /// `show_text`: the rotation or the origin is not finite.
    TextPlacementNonFinite,
    /// `show_text`: the height is non-finite or not positive — including the
    /// `Some(0.0)` the layout fills in when neither the record nor its style
    /// states one. A `None` that falls back to `DEFAULT_TEXT_MM` is not this.
    TextHeightInvalid,
    /// `show_text`: the text is empty after trimming.
    TextWhitespaceOnly,

    // ---------------------------------------------------------------- A.8
    /// `load_border_template`: a template candidate that could not be read
    /// (`read_archive_file`) or parsed (`parse_file` / `parse_reader`), after
    /// which the search `continue`d.
    TemplateCandidateUnreadable,
    /// The page names a template and no file answers it, so the page wears no
    /// furniture. Counted as `PaperModel::missing_templates` too.
    TemplateMissing,
}

/// Every code, in appendix A's row order.
///
/// The order is the contract: requirement 1.5 and 5.3 pick "the first reason
/// in appendix A" by taking the minimum, so this array and the derived `Ord`
/// must not drift apart. `all_codes_match_the_appendix` holds them against the
/// appendix's own table.
pub const ALL: [DropReason; 43] = [
    DropReason::RecordNoDecoder,
    DropReason::RecordRefused,
    DropReason::RecordNativeSkip,
    DropReason::SheetStreamWithoutGeometry,
    DropReason::RecordSymbolBodyCache,
    DropReason::CacheDefinitionUnplaced,
    DropReason::RecordAuditOnlyFamily,
    DropReason::RecordEmitterOutOfRange,
    DropReason::RecordEmitterShapeRejected,
    DropReason::LayerHidden,
    DropReason::LayerConsistencyChecks,
    DropReason::EndpointPairGuess,
    DropReason::KindMisrouted,
    DropReason::LineOnSheetEdge,
    DropReason::FillDuplicate,
    DropReason::PointNoMarker,
    DropReason::PointMarkerDegenerate,
    DropReason::KindAnnotation,
    DropReason::KindUnknown,
    DropReason::SymbolMissing,
    DropReason::SymbolUnresolvable,
    DropReason::SymbolBodyEmpty,
    DropReason::BodyLayerHidden,
    DropReason::BodyNullPlaceholder,
    DropReason::BodyLineOnSheetEdge,
    DropReason::BodyBoundaryCircleDegenerate,
    DropReason::BodyBoundaryMemberUnsupported,
    DropReason::OffSheet,
    DropReason::TextControlChar,
    DropReason::TextTrailingEmptyLine,
    DropReason::ProbeOnly,
    DropReason::FillDegenerate,
    DropReason::PenWidthInvalid,
    DropReason::ChainEmpty,
    DropReason::SegmentNonFinite,
    DropReason::SegmentTooShort,
    DropReason::SegmentOnSheetEdge,
    DropReason::TextEmpty,
    DropReason::TextPlacementNonFinite,
    DropReason::TextHeightInvalid,
    DropReason::TextWhitespaceOnly,
    DropReason::TemplateCandidateUnreadable,
    DropReason::TemplateMissing,
];

impl DropReason {
    /// Every code, in appendix A's order.
    #[must_use]
    pub fn all() -> &'static [DropReason] {
        &ALL
    }

    /// The appendix's reason code — which is also the serde name, so a
    /// baseline file reads as the table it was checked against.
    #[must_use]
    pub fn code(self) -> &'static str {
        match self {
            Self::RecordNoDecoder => "record_no_decoder",
            Self::RecordRefused => "record_refused",
            Self::RecordNativeSkip => "record_native_skip",
            Self::SheetStreamWithoutGeometry => "sheet_stream_without_geometry",
            Self::RecordSymbolBodyCache => "record_symbol_body_cache",
            Self::CacheDefinitionUnplaced => "cache_definition_unplaced",
            Self::RecordAuditOnlyFamily => "record_audit_only_family",
            Self::RecordEmitterOutOfRange => "record_emitter_out_of_range",
            Self::RecordEmitterShapeRejected => "record_emitter_shape_rejected",
            Self::LayerHidden => "layer_hidden",
            Self::LayerConsistencyChecks => "layer_consistency_checks",
            Self::EndpointPairGuess => "endpoint_pair_guess",
            Self::KindMisrouted => "kind_misrouted",
            Self::LineOnSheetEdge => "line_on_sheet_edge",
            Self::FillDuplicate => "fill_duplicate",
            Self::PointNoMarker => "point_no_marker",
            Self::PointMarkerDegenerate => "point_marker_degenerate",
            Self::KindAnnotation => "kind_annotation",
            Self::KindUnknown => "kind_unknown",
            Self::SymbolMissing => "symbol_missing",
            Self::SymbolUnresolvable => "symbol_unresolvable",
            Self::SymbolBodyEmpty => "symbol_body_empty",
            Self::BodyLayerHidden => "body_layer_hidden",
            Self::BodyNullPlaceholder => "body_null_placeholder",
            Self::BodyLineOnSheetEdge => "body_line_on_sheet_edge",
            Self::BodyBoundaryCircleDegenerate => "body_boundary_circle_degenerate",
            Self::BodyBoundaryMemberUnsupported => "body_boundary_member_unsupported",
            Self::OffSheet => "off_sheet",
            Self::TextControlChar => "text_control_char",
            Self::TextTrailingEmptyLine => "text_trailing_empty_line",
            Self::ProbeOnly => "probe_only",
            Self::FillDegenerate => "fill_degenerate",
            Self::PenWidthInvalid => "pen_width_invalid",
            Self::ChainEmpty => "chain_empty",
            Self::SegmentNonFinite => "segment_non_finite",
            Self::SegmentTooShort => "segment_too_short",
            Self::SegmentOnSheetEdge => "segment_on_sheet_edge",
            Self::TextEmpty => "text_empty",
            Self::TextPlacementNonFinite => "text_placement_non_finite",
            Self::TextHeightInvalid => "text_height_invalid",
            Self::TextWhitespaceOnly => "text_whitespace_only",
            Self::TemplateCandidateUnreadable => "template_candidate_unreadable",
            Self::TemplateMissing => "template_missing",
        }
    }

    /// Whether this drop may be a coverage gap, per appendix A's tier column.
    #[must_use]
    pub fn tier(self) -> Tier {
        match self {
            Self::RecordNoDecoder
            | Self::RecordRefused
            | Self::SheetStreamWithoutGeometry
            | Self::RecordEmitterOutOfRange
            | Self::RecordEmitterShapeRejected
            | Self::KindMisrouted
            | Self::PointMarkerDegenerate
            | Self::KindAnnotation
            | Self::SymbolMissing
            | Self::SymbolUnresolvable
            | Self::SymbolBodyEmpty
            | Self::BodyBoundaryCircleDegenerate
            | Self::BodyBoundaryMemberUnsupported
            | Self::OffSheet
            | Self::FillDegenerate
            | Self::PenWidthInvalid
            | Self::ChainEmpty
            | Self::SegmentNonFinite
            | Self::TextPlacementNonFinite
            | Self::TextHeightInvalid
            | Self::TemplateCandidateUnreadable
            | Self::TemplateMissing => Tier::Suspect,
            Self::RecordNativeSkip
            | Self::RecordSymbolBodyCache
            | Self::CacheDefinitionUnplaced
            | Self::RecordAuditOnlyFamily
            | Self::LayerHidden
            | Self::LayerConsistencyChecks
            | Self::EndpointPairGuess
            | Self::LineOnSheetEdge
            | Self::FillDuplicate
            | Self::PointNoMarker
            | Self::KindUnknown
            | Self::BodyLayerHidden
            | Self::BodyNullPlaceholder
            | Self::BodyLineOnSheetEdge
            | Self::TextControlChar
            | Self::TextTrailingEmptyLine
            | Self::ProbeOnly
            | Self::SegmentTooShort
            | Self::SegmentOnSheetEdge
            | Self::TextEmpty
            | Self::TextWhitespaceOnly => Tier::Intentional,
        }
    }

    /// Which layer of the pipeline this drop is counted at.
    #[must_use]
    pub fn layer(self) -> ReasonLayer {
        match self {
            Self::RecordNoDecoder
            | Self::RecordRefused
            | Self::RecordNativeSkip
            | Self::SheetStreamWithoutGeometry
            | Self::RecordSymbolBodyCache
            | Self::CacheDefinitionUnplaced
            | Self::RecordAuditOnlyFamily
            | Self::RecordEmitterOutOfRange
            | Self::RecordEmitterShapeRejected => ReasonLayer::Parse,
            Self::LayerHidden
            | Self::LayerConsistencyChecks
            | Self::EndpointPairGuess
            | Self::KindMisrouted
            | Self::LineOnSheetEdge
            | Self::FillDuplicate
            | Self::PointNoMarker
            | Self::PointMarkerDegenerate
            | Self::KindAnnotation
            | Self::KindUnknown
            | Self::SymbolMissing
            | Self::SymbolUnresolvable
            | Self::SymbolBodyEmpty => ReasonLayer::Entity,
            Self::BodyLayerHidden
            | Self::BodyNullPlaceholder
            | Self::BodyLineOnSheetEdge
            | Self::BodyBoundaryCircleDegenerate
            | Self::BodyBoundaryMemberUnsupported => ReasonLayer::Body,
            Self::TextControlChar | Self::TextTrailingEmptyLine => ReasonLayer::TextNormalization,
            Self::OffSheet
            | Self::ProbeOnly
            | Self::FillDegenerate
            | Self::PenWidthInvalid
            | Self::ChainEmpty
            | Self::TextEmpty
            | Self::TextPlacementNonFinite
            | Self::TextHeightInvalid
            | Self::TextWhitespaceOnly => ReasonLayer::Primitive,
            Self::SegmentNonFinite | Self::SegmentTooShort | Self::SegmentOnSheetEdge => {
                ReasonLayer::Segment
            }
            Self::TemplateCandidateUnreadable | Self::TemplateMissing => ReasonLayer::Page,
        }
    }
}

/// A primitive that reached the page but not whole (appendix A.6).
///
/// Not a drop: the ink is there. Counted so that "on the page" is not mistaken
/// for "on the page as the drawing states it".
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PartialMark {
    /// `show_text` squeezed the run along its own direction until it fitted.
    /// Counted as `PlotAudit::runs_condensed` too.
    TextCondensed,
    /// The run was squeezed to `MIN_CONDENSE` and still ran past the edge, so
    /// the page clip took its tail. Counted as
    /// `PlotAudit::runs_clipped_at_the_condensation_floor` too.
    TextClippedAtFloor,
    /// The ink reaches more than 0.5 mm past the page edge and the page clip
    /// cuts it. Counted as `PaperModel::primitives_past_the_edge(0.5)` too —
    /// **not** as `export_plant_backup`'s `EDGE_SLACK_MM` of 0.1.
    PastTheEdge,
    /// The primitive reached the page with at least one stroke dropped under
    /// A.5.
    SegmentsDropped,
}

impl PartialMark {
    /// Every mark.
    #[must_use]
    pub fn all() -> &'static [PartialMark] {
        &[
            Self::TextCondensed,
            Self::TextClippedAtFloor,
            Self::PastTheEdge,
            Self::SegmentsDropped,
        ]
    }

    /// The mark's code, which is also its serde name.
    #[must_use]
    pub fn code(self) -> &'static str {
        match self {
            Self::TextCondensed => "text_condensed",
            Self::TextClippedAtFloor => "text_clipped_at_floor",
            Self::PastTheEdge => "past_the_edge",
            Self::SegmentsDropped => "segments_dropped",
        }
    }

    /// Whether measuring this mark needs a typeface.
    ///
    /// The two text marks do — condensing and clipping are decisions about the
    /// width a face gives the run. A ledger taken without faces therefore
    /// carries neither key (design deviation I1).
    #[must_use]
    pub fn needs_fonts(self) -> bool {
        matches!(self, Self::TextCondensed | Self::TextClippedAtFloor)
    }
}

/// The furniture domain drawing what the drawing domain would have dropped
/// (appendix A.7). Counted, never changed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Asymmetry {
    /// A furniture `SymbolInstance` on a hidden or `ConsistencyChecks` layer:
    /// the furniture loop does not consult `suppressed_from_plot`, so it draws.
    FurnitureSymbolOnSuppressedLayer,
    /// A furniture `EndpointPair`: the furniture loop does not check the record
    /// kind, so it lays the entity out.
    FurnitureEndpointPairDrawn,
}

impl Asymmetry {
    /// Every asymmetry.
    #[must_use]
    pub fn all() -> &'static [Asymmetry] {
        &[
            Self::FurnitureSymbolOnSuppressedLayer,
            Self::FurnitureEndpointPairDrawn,
        ]
    }

    /// The asymmetry's code, which is also its serde name.
    #[must_use]
    pub fn code(self) -> &'static str {
        match self {
            Self::FurnitureSymbolOnSuppressedLayer => "furniture_symbol_on_suppressed_layer",
            Self::FurnitureEndpointPairDrawn => "furniture_endpoint_pair_drawn",
        }
    }
}

/// Where a placement's body came from (requirements 4.1, 4.2).
///
/// [`Self::Unresolvable`] is the default: a placement record is built before
/// its body is resolved, and "nothing answered" is the state that has to be
/// overwritten rather than the state that quietly looks successful.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize,
)]
#[serde(rename_all = "snake_case")]
pub enum PlacementSource {
    /// The drawing's own definition cache carried the body.
    Cached,
    /// An external `.sym` body answered the placement's symbol path.
    Library,
    /// A symbol path was named and no body answered it.
    Missing,
    /// Neither a symbol path nor a resolvable cached body.
    #[default]
    Unresolvable,
}

impl PlacementSource {
    /// The source's code, which is also its serde name.
    #[must_use]
    pub fn code(self) -> &'static str {
        match self {
            Self::Cached => "cached",
            Self::Library => "library",
            Self::Missing => "missing",
            Self::Unresolvable => "unresolvable",
        }
    }
}

/// Which of the two accounts an entity or primitive belongs to.
///
/// The drawing is the `.pid`'s own records; the furniture is the `.igr` border
/// template's. They are kept apart because they take different layout paths —
/// and because the two asymmetries in [`Asymmetry`] are exactly the places
/// where one path drops what the other draws.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LedgerDomain {
    /// The drawing's own records and entities.
    Drawing,
    /// The border template the page wears.
    Furniture,
}

impl LedgerDomain {
    /// Both domains, drawing first.
    #[must_use]
    pub fn all() -> &'static [LedgerDomain] {
        &[Self::Drawing, Self::Furniture]
    }

    /// The domain's code, which is also its serde name.
    #[must_use]
    pub fn code(self) -> &'static str {
        match self {
            Self::Drawing => "drawing",
            Self::Furniture => "furniture",
        }
    }
}

/// One value per [`LedgerDomain`], reached by `match` rather than by index.
///
/// The two domains are a fixed pair, so they are stored as two fields instead
/// of a map: a `match` on the domain cannot go out of bounds, while an index
/// into a two-element array can.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct DomainPair<T> {
    /// The drawing domain's value.
    pub drawing: T,
    /// The furniture domain's value.
    pub furniture: T,
}

impl<T> DomainPair<T> {
    /// The value for `domain`.
    #[must_use]
    pub fn get(&self, domain: LedgerDomain) -> &T {
        match domain {
            LedgerDomain::Drawing => &self.drawing,
            LedgerDomain::Furniture => &self.furniture,
        }
    }

    /// The value for `domain`, mutably.
    pub fn get_mut(&mut self, domain: LedgerDomain) -> &mut T {
        match domain {
            LedgerDomain::Drawing => &mut self.drawing,
            LedgerDomain::Furniture => &mut self.furniture,
        }
    }

    /// Both values, drawing first — the order [`LedgerDomain::all`] states.
    #[must_use]
    pub fn iter(&self) -> [(&'static str, &T); 2] {
        [
            (LedgerDomain::Drawing.code(), &self.drawing),
            (LedgerDomain::Furniture.code(), &self.furniture),
        ]
    }
}

#[cfg(test)]
// A test's job is to fail loudly on a bad value, so the four production bans
// are lifted here and only here (design: "测试模块按需 #[allow]").
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]
mod tests {
    use super::*;

    /// The appendix's code table, transcribed in its row order.
    ///
    /// Written out here rather than derived from [`ALL`] so that the two can
    /// disagree: this is the test's own copy of the specification.
    const APPENDIX_A: [&str; 43] = [
        // A.1 parse: records that never became entities
        "record_no_decoder",
        "record_refused",
        "record_native_skip",
        "sheet_stream_without_geometry",
        "record_symbol_body_cache",
        "cache_definition_unplaced",
        "record_audit_only_family",
        "record_emitter_out_of_range",
        "record_emitter_shape_rejected",
        // A.2 layout: entities that produced no page primitive
        "layer_hidden",
        "layer_consistency_checks",
        "endpoint_pair_guess",
        "kind_misrouted",
        "line_on_sheet_edge",
        "fill_duplicate",
        "point_no_marker",
        "point_marker_degenerate",
        "kind_annotation",
        "kind_unknown",
        "symbol_missing",
        "symbol_unresolvable",
        "symbol_body_empty",
        // A.3 layout: symbol-body primitives
        "body_layer_hidden",
        "body_null_placeholder",
        "body_line_on_sheet_edge",
        "body_boundary_circle_degenerate",
        "body_boundary_member_unsupported",
        // A.4 layout: page primitives and text normalization
        "off_sheet",
        "text_control_char",
        "text_trailing_empty_line",
        // A.5 write: primitives and segments
        "probe_only",
        "fill_degenerate",
        "pen_width_invalid",
        "chain_empty",
        "segment_non_finite",
        "segment_too_short",
        "segment_on_sheet_edge",
        "text_empty",
        "text_placement_non_finite",
        "text_height_invalid",
        "text_whitespace_only",
        // A.8 page facts
        "template_candidate_unreadable",
        "template_missing",
    ];

    #[test]
    fn all_codes_match_the_appendix() {
        let actual: Vec<&str> = ALL.iter().map(|reason| reason.code()).collect();
        assert_eq!(
            actual,
            APPENDIX_A.to_vec(),
            "DropReason::ALL must list appendix A's codes in its row order"
        );
        assert_eq!(ALL.len(), 43, "the catalog is 43 codes");
    }

    #[test]
    fn tiers_split_22_suspect_21_intentional() {
        let suspect = ALL
            .iter()
            .filter(|reason| reason.tier() == Tier::Suspect)
            .count();
        let intentional = ALL
            .iter()
            .filter(|reason| reason.tier() == Tier::Intentional)
            .count();
        assert_eq!(suspect, 22, "appendix A's suspect column");
        assert_eq!(intentional, 21, "appendix A's intentional column");
        assert_eq!(suspect + intentional, 43);
    }

    #[test]
    fn serde_names_equal_codes() {
        for reason in ALL {
            let json = serde_json::to_string(&reason).expect("a unit variant serializes");
            assert_eq!(
                json,
                format!("\"{}\"", reason.code()),
                "the serde name must be the reason code"
            );
            let back: DropReason = serde_json::from_str(&json).expect("and reads back");
            assert_eq!(back, reason);
        }
    }

    #[test]
    fn the_derived_order_is_the_appendix_order() {
        // Requirement 1.5 and 5.3 take the first reason in appendix A, which is
        // `min()` over this enum. That only means "first in the appendix" while
        // the declaration order and `ALL` agree.
        let mut sorted = ALL;
        sorted.sort();
        assert_eq!(sorted, ALL, "Ord must follow the appendix's row order");
    }

    #[test]
    fn every_layer_has_at_least_one_code() {
        for layer in [
            ReasonLayer::Parse,
            ReasonLayer::Entity,
            ReasonLayer::Body,
            ReasonLayer::Primitive,
            ReasonLayer::Segment,
            ReasonLayer::TextNormalization,
            ReasonLayer::Page,
        ] {
            let count = ALL.iter().filter(|reason| reason.layer() == layer).count();
            assert!(count > 0, "{layer:?} has no reason code");
        }
    }

    #[test]
    fn partial_marks_and_asymmetries_are_four_and_two() {
        assert_eq!(PartialMark::all().len(), 4, "appendix A.6");
        assert_eq!(Asymmetry::all().len(), 2, "appendix A.7");
        let font_marks: Vec<&str> = PartialMark::all()
            .iter()
            .filter(|mark| mark.needs_fonts())
            .map(|mark| mark.code())
            .collect();
        assert_eq!(font_marks, ["text_condensed", "text_clipped_at_floor"]);
    }

    #[test]
    fn a_domain_pair_reads_each_domain_by_match() {
        let pair = DomainPair {
            drawing: "d",
            furniture: "f",
        };
        assert_eq!(*pair.get(LedgerDomain::Drawing), "d");
        assert_eq!(*pair.get(LedgerDomain::Furniture), "f");
        let seen: Vec<(&str, &&str)> = pair.iter().to_vec();
        assert_eq!(seen, [("drawing", &"d"), ("furniture", &"f")]);
    }
}
