# A sheet that states no sheet still names its border

2026-11-01. Four of the fifty-three plant backup sheets declare no page size:
`D井场` (A0 by ink), `Q井场` (A1), `换热器CD` (A1) and
`集注站 凝析油一级闪蒸分离器B` (A2). `§2.13` of issue #41 left open whether a
measured page should also pick border furniture. It should — but only when the
sheet names the template it wants.

## What the four sheets are

A page comes from the drawing's own `igSmartFrame2d` (`page_dimensions_mm`).
These four carry none, so `PaperModel::resolved_page_mm` measures instead: the
smallest standard sheet the ink fits, with 5 mm of slack. That is the same
number the PDF is given, so the page is not in doubt — only the furniture was.

Their raw bytes were searched for `.igr` / `.IGR` link records:

| sheet | linked templates in the file | measured page |
|---|---|---|
| `D井场` | none | 1189 × 841 |
| `Q井场` | `CPECCHB-SQ-A1.igr`, `CPECCH~3.IGR` (DOS 8.3 alias) | 841 × 594 |
| `换热器CD` | none | 841 × 594 |
| `集注站 凝析油一级闪蒸分离器B` | none | 594 × 420 |

`Q井场` is the only one that names anything, and its ink measures exactly the
`CPECCHB-SQ-A1.igr` extent, 0.00 mm off.

## What the one reference plot among them says

`换热器CD` is one of the five corpus drawings with a reference plot of its own.
It is a PASS (1370 primitives, 251 texts) and its plot carries **no frame and no
title block at all**. That settles the unnamed case: a sheet whose box merely
happens to match a template's extent does not wear it. A size coincidence is not
a request.

## The rule

`model.rs` now builds `furniture_page` in three branches:

1. the sheet declares a page → that page, unchanged;
2. no declared page and no linked template → no furniture (`换热器CD`, `D井场`,
   `集注站 凝析油一级闪蒸分离器B`);
3. no declared page but a linked template → `plottable_sheet_mm`, the smallest
   standard sheet the *plottable* ink fits, then the existing picker (name is
   the ticket, 10 mm window for a named page).

The measurement is taken over `!suppressed_from_plot` entities only
(`ink_extent_mm_where`), because ink on a hidden layer reaches no sheet: letting
it widen the box would choose a size up and dress the page in furniture of the
wrong size. `missing_templates` now keys off `furniture_page.is_some()`, so a
sheet that names a template and gets none is still reported by name.

## Readings

- templated sheets **49 → 50**; `CPECCHB-SQ-A1` **3 → 4** sheets
- placements stay **64** (that template places no symbol); labels **175** (a
  per-template count); named gaps **64**
- `Q井场`: 4363 → **4459** primitives, 589 → **626** texts, stacked **2** (the
  template's ten title-block cells, written twice — the same duplication
  `N井场` / `O井场` / `P井场` already carry), missing symbols **2**, pens gain
  `0.2500 / 0.3000 / 0.5000 / 0.7500 / 0.7999` from the template's own ink
- `style_witness`: clean sheets **33 → 32**, affected **20 → 21**; `0.3000`
  (0 drawing, 24 furniture) → (0, 32); `0.5000` (24, 12) → (24, 16)
- `export_plant_backup`, `border_template`, `style_witness` all green; the other
  three no-page sheets move not one primitive

## What is still unproven

`Q井场` has no reference plot. The decision rests on two facts that agree — the
sheet's own link record naming `CPECCHB-SQ-A1.igr` twice, and its ink measuring
that template exactly — which is the same standard the other forty-nine
templated sheets were picked on (name first, size second). If a reference plot
of `Q井场` ever arrives it decides the case; until then the page is dressed by
what the file itself asks for, and this note says so.
