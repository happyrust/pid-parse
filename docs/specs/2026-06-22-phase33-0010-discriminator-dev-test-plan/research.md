# Research: Phase 33 PSM 0x0010 Discriminator

## Decision: Keep `0x0010` As `TypedAudit` Until Reader Evidence Exists

Rationale:

- Existing parser evidence proves a stable envelope and raw payload capture.
- `leading_word` distribution is useful but not a semantic discriminator.
- Several size buckets are heterogeneous at payload offset `+0`, so a single
  fixed-position discriminator is not proven.
- The current atlas explicitly blocks promotion without native reader evidence
  or controlled fixture evidence.

Alternatives considered:

- Promote `leading_word` to `sub_kind`: rejected because it would encode a
  positional byte as business semantics.
- Promote size buckets as sub-families: rejected because size is not a semantic
  discriminator and may reflect payload shape only.
- Emit `0x0010` geometry: rejected because no geometry meaning is proven.

## Decision: Do Not Use `sppid.dll` Or `core.dll` For Promotion

Rationale:

- The live `sppid.dll` instance is VB6 / COM application glue with no direct
  raw `.pid` persistence strings or `IOContext::DoIO` evidence.
- The live `core.dll` instance is a broad AVEVA platform module and not a
  SmartPlant raw `.pid` byte-layout source by itself.
- The authoritative atlas records the same negative boundary.

Alternatives considered:

- Continue broad search in `sppid.dll`: rejected because prior searches found
  no persistence surface and it risks wasting cycles.
- Treat `core.dll` sheet vocabulary as PID evidence: rejected because E3D sheet
  tokens do not prove SmartPlant Sheet stream layout.

Execution note (2026-06-23):

- `list_instances` returned only reachable `sppid.dll` (`127.0.0.1:13337`)
  and `core.dll` (`127.0.0.1:13338`).
- No preferred Phase 33 target (`radsrvitem.dll`, `J2DSrv.dll`, `style.dll`,
  `jengine.dll`, `XceedRAD.dll`, `OLESITE.dll`, or `OLECRT.dll`) was reachable.
- The selected path is tooling-gated negative closeout:
  `docs/analysis/2026-06-23-phase33-0010-discriminator-ida-evidence.md`.
- Parser confidence remains unchanged; `0x0010` stays `TypedAudit`.

## Decision: Prioritize RAD / 2D Server Modules

Rationale:

- `radsrvitem.dll` is the best candidate for RAD persisted object dispatch,
  type tables, and `PSMSerializeIn` / `PSMSerializeOut`.
- `J2DSrv.dll` may contain 2D record or Sheet/RAD projection helpers.
- `style.dll` is historically positive for `JStyleOverride 0x0030`, and
  `0x0010` is referenced near JStyle data.
- `OLESITE.dll` and `OLECRT.dll` are useful only if the trace crosses into OLE
  site/storage ownership.

Alternatives considered:

- Start with `PSMspacemap`: rejected for this phase because handle math does
  not prove raw page layout.
- Start with page transform: rejected because the source coordinate transform
  remains broader than `0x0010` and requires controlled fixture evidence.

## Decision: Separate Evidence Collection From Parser Implementation

Rationale:

- The repository has explicit confidence classes and forbidden shortcuts.
- A doc-only evidence phase can close negatively without code churn.
- Parser implementation should start only after byte ranges and fixture
  ratchets are already known.

Alternatives considered:

- Add an exploratory parser first: rejected because it would create public
  schema pressure before semantics are proven.
- Add hidden parser fields without schema exposure: rejected because byte-audit
  and downstream consumers still need honest confidence boundaries.

## Decision: Test Plan Must Include Negative Closeout

Rationale:

- IDA modules may be unavailable.
- Candidate functions may only prove runtime dispatch, not persisted byte reads.
- Negative evidence is useful because it prevents repeated broad searches and
  keeps the roadmap honest.

Alternatives considered:

- Treat unavailable modules as no-op: rejected because future work needs a
  clear re-open trigger.
- Keep searching until a hit appears: rejected because the repository already
  has enough negative evidence to require targeted work.
