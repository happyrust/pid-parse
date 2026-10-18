# 2026-10-18 — Dashed ink reads dash by dash

Scope: `pid-plot` reader (`src/read.rs`), writer (`src/write.rs`), and what the
five-drawing comparison reports now say.

## What the reader said before

`InkDevice::draw_path` read a stroke's geometry and width and threw the dash
array away, so a dashed line entered `PlotInk` as one solid segment. `hayro`
hands the pattern over in `StrokeProps` (`dash_array`, `dash_offset`), so the
information was there; nothing consumed it. The reference plots never show the
problem: every dash in them is its own pre-split path item and no reference
page carries a dash array at all. Our own plots do carry one, which meant the
comparison saw solid ink where the page prints a dash and a gap.

## What changed

- **`read::DashWalk`** — one walker both sides share. It normalises the pattern
  (odd counts repeat, negative/non-finite entries count as zero, an all-zero
  pattern walks solid), starts a subpath at the phase `dash_offset` names, and
  splits one run into the pieces the pattern inks. It keeps phase across
  consecutive runs and `restart()`s at every `MoveTo`, which is the PDF rule.
- **Reader** scales the pattern by the CTM (`transform_scale`, the same factor
  the line width uses) and splits each `LineTo`/`ClosePath` run. A run that the
  sheet-frame rule drops still advances the phase.
- **Writer** now emits one subpath per shape (`m` then `l`…) instead of an
  `m`/`l` pair per straight piece, so a dash pattern flows along a whole
  polyline rather than restarting at every vertex. Pieces are grouped into runs
  where a dropped edge (zero length, non-finite, sheet frame) breaks the chain.
- **`read::sheet_edge_mm`** is public now and `write.rs` uses it instead of its
  own copy of the rule, so the frame a plot drops and the frame the reader drops
  cannot drift apart.
- **`write::stroked_segments`** runs the same walker over the same chains, so
  `write_five_drawings` still matches a read-back endpoint for endpoint.

## What it moved

| drawing | coverage before → after | extra before → after |
|---|---|---|
| I井场 | 0.99848 → 0.99846 | 55.66 → 55.66 mm |
| A | 0.99922 → 0.99922 | 14.64 → 14.64 mm |
| B | 0.99726 → 0.99728 | 8.63 → 8.63 mm |
| 一 (低温分离器A) | 0.99810 → 0.99809 | 29.72 → 29.72 mm |
| 二 (换热器CD) | 0.99832 → 0.99833 | 40.23 → 38.74 mm |

Why so little: the comparator samples every `SAMPLE_MM = 0.5` and calls a sample
covered within `COVER_MM = 0.5`. A gap has to exceed 1 mm before its middle is
out of reach, and the sheet dash gaps are 1.27 mm, so only some phases of some
dashes register. The symbol-body pen is dash-dot (`7.0 / 1.75 / 0.35 / 1.75`),
whose 1.75 mm gaps are visible, which is where 二's 1.5 mm came from. Fit stayed
robust either way: I井场 still pairs 671 long lines and all five keep
`within_threshold`.

## The residual that dominates I井场

One line accounts for most of what is left: a vertical sheet line at
x ≈ 551.7 mm running y ≈ 97 → 713 mm (a template fold line). Measured on the
plot, not the model:

- ours — one stroke, `d [5.4 3.6] 0` in points = **1.905 mm on, 1.270 mm off**,
  width 0.180 mm. That is the file's `0x002F` dash id 52 `[-1.905, -1.270]`,
  and it is the only pattern any resolved line style names (57 of them).
- reference — pre-split items of **1.86 mm on + 0.17 mm cap + 1.02 mm off**
  (period 3.05), width 0.169 mm, dash array `[] 0`.
- 3.05 / 1.02 is exactly 2:1, i.e. the file's *other* dash record, id 95
  `[-3.500, -1.750]`, at a 0.58 scale — not id 52 at any scale.

The file carries all three root `StyleCluster` dash records: 27
`[-7.620, -inf]` (the complex dash), 52 `[-1.905, -1.270]`, 95
`[-3.500, -1.750]`; `JSite37311\StyleCluster` carries 17 `[-3.500, -1.750]`
and 54 `[-7.000, +1.750, +0.350, +1.750]`. `line_styles_for_file` resolves
57 styles to id 52's pattern and none to id 95's, so either the printed plot
ran with a plot-time linetype table/scale, or the line-style → dash join picks
id 52 for a style that should name 95. Both are open; nothing here distinguishes
them yet.

Where the two do agree, they agree closely: the symbol-body dash-dot prints as
`7.11 / 1.69 / 0.34 / 1.70` against our `7.0 / 1.75 / 0.35 / 1.75`, i.e. within
0.11 mm per segment.

## Guard rails

- `read::tests` covers the walker: pieces of a known pattern, phase carried
  across a gap, an offset that starts inside the pattern, an odd array that
  repeats plus `restart()`, an empty/all-zero pattern walking solid, and the
  frame rule in millimetres.
- `write_five_drawings` still round-trips all five plots through the PDF and
  PyMuPDF, endpoint for endpoint.
- `read_reference_plots` is untouched: reference pages carry no dash arrays, so
  their segment counts cannot move.
