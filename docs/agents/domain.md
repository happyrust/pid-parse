# Domain Docs

How the engineering skills consume this repo's domain documentation.

Single-context repo.

## Before exploring, read these

- **`CONTEXT.md`** at the repo root — the evidence-based analysis glossary (Evidence-Complete Read Parsing, Decoded Data, Typed Audit Data, Probe Evidence, Identified-Only Data, Unknown Data, Raw Preservation, Versioned Reference Corpus, Coverage Gap, Controlled Fixture).
- **`docs/adr/`** — read ADRs that touch the area you're about to work in:
  - ADR-0001 — full parsing = evidence-complete read-only (no semantic write-back).
  - ADR-0002 — completeness measured against a versioned reference corpus; ratchet changes must be explicit.
  - ADR-0003 — Phase 35 mainline: P0 worktree closeout, then curve-family decoders behind semantic gates.

If any file doesn't exist, proceed silently.

## Use the glossary's vocabulary

Name domain concepts with the terms in `CONTEXT.md`; don't drift to the synonyms it explicitly avoids (e.g. use "Decoded Data", not "recognized/plausible data"; "Coverage Gap", not "parser failure").

The architecture-design vocabulary (module, interface, implementation, depth, seam, adapter, leverage, locality) comes from the `/codebase-design` skill — keep it distinct from the domain glossary.

## Flag ADR conflicts

If your output contradicts an existing ADR, surface it explicitly rather than silently overriding.
