# pid-parse draws its own Rendered Plot instead of plotting through OpenCADStudio

Status: Accepted (grill-with-docs Q2, 2026-10-12)

A Plot Comparison is meant to say what pid-parse read right or wrong in a drawing, so the Rendered Plot is drawn from pid-parse's own parse (geometry, style tables and the `.sym` bodies the drawing names), not by OpenCADStudio's `--plot-svg` / `--plot-pdf`, even though OpenCADStudio already composes symbols, styles and text for display. Through OpenCADStudio, every difference against the Reference Plot could be the parser or the importer's own drawing policy (pid-cad#4 draws the literal value by default), and each one would need that triage before it says anything about the parse.

## Considered Options

- Plot through OpenCADStudio: the fastest way to a first PDF; rejected because the importer's choices land in every difference.
- Move OpenCADStudio's `src/io/pid` composition into pid-parse and share it: the cleanest end state, but a refactor across two repositories before the first comparison number exists.

## Consequences

- Symbol placement, style resolution and text layout get a second implementation next to OpenCADStudio's `src/io/pid`; the two can drift, and a difference between them is not by itself a parse defect.
