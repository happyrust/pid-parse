# Phase 34 0x0020 Relaxed Neighbor Correlation (Closeout)

> Date: 2026-06-30
> Scope: Phase 34-B final evidence probe for `0x0020 igRectangle2d`. No parser,
> schema, writer, bundle confidence, or normalized geometry output changed by
> this note. This closes Phase 34-B for `0x0020` with a negative decision.

## Why This Probe Was Needed

The earlier neighbor correlation
(`docs/analysis/2026-06-26-phase34-0020-neighbor-correlation.md`) reported
"no decoded igLine2d neighbor" for every `0x0020` candidate. That was an
artifact of the **strict** decoder gate, not proof that the neighbours lack
geometry: `decode_igline_at` requires the payload `remaining_header` word
(`payload+8`) to equal `12`, and every neighbour of a `0x0020` record fails
that magic check even though it carries the `0x0018` type code and `btf=50`.

To make an honest decision, `examples/probe_psm_undecoded_shapes.rs` was
extended with a **relaxed** neighbour read that pulls the canonical igLine2d
`(start, end)` f64 quad at payload offsets `+18/+26/+34/+42` **without** the
`remaining_header == 12` gate, then correlates the `0x0020` candidate f64s
against those relaxed endpoints / bbox / extents / length.

Command:

```powershell
cargo run --quiet --example probe_psm_undecoded_shapes
```

## What The Relaxed Correlation Found

`0x0020` has 4 records, all `btf=78`, in two ownership families. The relaxed
read shows their neighbours are **non-canonical `0x0018` records**
(`remaining_header` is `6996` or `8`, never the canonical `12`), and the
candidate correlation differs sharply between the two families.

### Family A — `/Sheet6615` mini-sheet (DWG-0202 + publish duplicate)

Record `0x000008..0x00005C`, `flags=2`, first PSM record in the stream.
Next record: `0x0018 btf=50`, relaxed-decoded as `oid=4732`,
`start=(0.126271288517, 0.092378904764)`, `end=(0.253749665278, 0.092378904764)`
(a horizontal line), `length = extent.x = 0.127478376761`,
but `remaining_header = 6996`.

| `0x0020` candidate | Value | Relaxed neighbour match | Delta |
|---|---|---|---|
| `+18` | `0.126271288517` | `next~ start.x` | **0.0 (exact bits)** |
| `+26` | `0.092378904764` | `next~ start.y` | **0.0 (exact bits)** |
| `+34` | `0.127478376761` | `next~ extent.x` / `length` | **0.0 (exact bits)** |
| `+50` | `0.406862745098` | (best) `end.x`, delta `0.153` | no match |

Three of four candidate doubles match the adjacent line's start corner and
extent **exactly** (identical f64 bits, not "near"). But:

- `+42 = 0.0`, so there is **no height / second extent** field — a rectangle
  needs both extents and only one is present;
- `+50 = 0.406862745098` decodes from bytes `09 0A 0A 0A 0A 0A DA 3F`; the
  `0A 0A 0A 0A` mantissa run is padding-shaped noise that happens to form a
  finite double, not a stable field;
- the neighbour is itself a variant record (`remaining_header = 6996`), so
  even the line it "matches" is outside the validated igLine2d dialect.

### Family B — nested `/JSite204\Sheet6` (A01 publish fixture)

Two records, `flags=0`, embedded among page-border lines. The neighbours
relaxed-decode to the A2 page frame (e.g. `oid=294 start=(0.594,0.420)
end=(0.0,0.420)`, `oid=296 start=(0.0,0.420) end=(0.0,0.0)`), all with
`remaining_header = 8`.

| Sample | `0x0020` structured doubles | Relaxed neighbour match |
|---|---|---|
| `0x000594..0x0005E8` | `+34=0.594`, `+50=0.707070707071` (`+18=+26=+42=0`) | none (best deltas `0.176` / `0.289`) |
| `0x000A04..0x000A58` | `+18=0.025`, `+26=0.010`, `+34=0.559`, `+50=0.715563506261` | none (best deltas `0.025` / `0.035`) |

No candidate matches any neighbour endpoint, bbox, extent, or length. The
values look page-frame / title-block related instead: `+34=0.594` equals the
A2 page width in metres, `+50≈0.707≈1/√2` is the ISO A-series aspect ratio,
and `+18/+26` look like small margins (`0.025`, `0.010`).

## Decision: Negative Closeout

`0x0020 = identified drawable type name (igRectangle2d), ownership-gated, no
decoder.` Phase 34-B is closed without a parser slice.

The relaxed correlation strengthens — rather than reverses — the prior
negative decision, against the promotion gate in
`docs/plans/2026-06-19-pid-parser-roadmap-gates.md`:

1. **Field semantics are not stable across ownership families.** In
   `/Sheet6615` the `+18/+26/+34` doubles equal a neighbour line's
   corner/extent; in nested `/JSite204\Sheet6` the same offsets carry
   page-frame-like values with no neighbour correlation. One byte layout
   cannot be claimed for both.
2. **No rectangle invariant is provable.** `+42` is `0.0` in every sample, so
   there is no second extent / opposite corner / height; "rectangle" cannot be
   reconstructed from a single corner plus one extent.
3. **Neighbours are a variant dialect.** Every neighbour `0x0018` record has
   `remaining_header ∈ {8, 6996}`, never `12`, so these streams use a record
   dialect the validated igLine2d decoder rejects; correlating against them
   cannot anchor `Decoded` semantics.
4. **Ownership is unresolved.** No `0x0020` hit lands in a primary top-level
   `/Sheet6`; all are `/Sheet6615` mini-sheet or nested symbol-local records.
   Emitting them into top-level normalized geometry would conflate three
   coordinate scopes.

Per the roadmap gate (exact byte ranges for every field, stable cross-fixture
layout, proven record kind, native-reader or controlled-fixture evidence for
semantic names), none of the required proof exists.

Do not add `SheetIgRectangle2dDecoded`, `PidGraphicKind::Rectangle`, a
closed-polyline rectangle emission, byte-audit consumed-range movement, or any
writer / bundle confidence change.

## Reopen Conditions

Reopen `0x0020` only when a controlled fixture or native reader proves **both**:

1. the ownership projection — whether `/Sheet6615` and nested
   `/JSite204\Sheet6` records belong in top-level or symbol-local geometry; and
2. a stable bounded field set for two corners / extents (width **and** height)
   that holds across both ownership families, including the meaning of `+42`
   and `+50`.

## Probe Evidence Trail

- `examples/probe_psm_undecoded_shapes.rs` — now prints, per `0x0020` sample:
  immediate prev/next record summaries, nearest strict-decoded igLine2d,
  nearest **relaxed** igLine2d (with `remaining_header` and finite-in-range
  flag), candidate finite f64 offsets, and both strict and relaxed
  candidate-to-neighbour deltas.
- `docs/analysis/2026-06-26-phase34-0020-igrectangle2d-readiness.md` — prior
  readiness note.
- `docs/analysis/2026-06-26-phase34-0020-ownership-review.md` — ownership
  families.
- `docs/analysis/2026-06-26-phase34-0020-neighbor-correlation.md` — prior
  strict-only correlation (superseded by this relaxed pass).
