# pid-parse PRD Follow-up Execution Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Turn the new current-state PRD into the canonical entry point for the next parser/publish development cycle, then start the first documentation-linking task.

**Architecture:** Keep the first execution slice documentation-only and low-risk: make the PRD discoverable from README and architecture docs, then use the PRD's P1/P2 sequence for future implementation work. Do not change parser behavior until fixture/baseline availability is confirmed.

**Tech Stack:** Markdown docs, existing `pid_inspect --byte-audit` CLI, existing `.github/scripts/check-byte-audit-baselines.sh`, Rust/Cargo verification gates.

---

## Execution Status

- Task 1 started: README already links to `docs/prd-pid-parse-current-state.md`; `docs/architecture-guide.md` now links to the same PRD from the project overview.
- Task 2 checked: `.github/scripts/check-byte-audit-baselines.sh` exists, but no `test-file/*.pid` fixture and no `docs/baselines/*.byte-audit.json` baseline are present in this checkout. Real byte-audit baseline generation is blocked on fixture availability.

### Task 1: Make PRD Discoverable

**Files:**
- Modify: `README.md`
- Modify: `docs/architecture-guide.md`
- Existing: `docs/prd-pid-parse-current-state.md`

**Steps:**

1. Confirm README has a current-state docs section linking to `docs/prd-pid-parse-current-state.md`.
2. Add a short PRD link near the top of `docs/architecture-guide.md`, so readers who start from architecture can jump to product status and roadmap.
3. Run lints/diagnostics for touched Markdown files.

**Expected Result:** New readers can find the PRD from both README and the architecture guide.

### Task 2: Prepare Real Fixture Baseline Work

**Files:**
- Existing: `docs/byte-audit-guide.md`
- Existing: `.github/scripts/check-byte-audit-baselines.sh`
- Future: `docs/baselines/<fixture>.byte-audit.json`

**Steps:**

1. Confirm whether private `.pid` fixtures exist under `test-file/`.
2. If a fixture is available, generate a byte-audit JSON baseline:
   ```bash
   cargo run --locked --bin pid_inspect -- test-file/<fixture>.pid --byte-audit --json > docs/baselines/<fixture>.byte-audit.json
   ```
3. Run:
   ```bash
   bash .github/scripts/check-byte-audit-baselines.sh
   ```
4. If fixtures are absent, keep public CI soft-skip behavior and record that P1 is blocked on fixture availability.

**Expected Result:** The team knows whether P1 can proceed now or is blocked by private fixture availability.

### Task 3: Start PSM Deepening Design

**Files:**
- Existing: `src/parsers/psm_tables.rs`
- Existing: `src/streams/psm_tables.rs`
- Existing: `src/model.rs`
- Existing: `src/byte_audit/aggregate.rs`
- Future: `docs/plans/YYYY-MM-DD-psm-table-deepening.md`

**Steps:**

1. Inspect current `PsmClusterTable` and `PsmSegmentTable` model fields.
2. Draft decoded/audit/raw target structs before changing parser behavior.
3. Require at least two real fixtures before upgrading coverage status from `PartiallyDecoded`.

**Expected Result:** PSM work has a design gate before code changes, avoiding single-fixture overfitting.

### Task 4: Defer Sheet Geometry Until PSM Design Lands

**Files:**
- Existing: `src/parsers/sheet_probe.rs`
- Existing: `src/parsers/sheet_endpoint_records.rs`
- Existing: `src/layout.rs`
- Future: `docs/plans/YYYY-MM-DD-sheet-geometry-deepening.md`

**Steps:**

1. Keep existing sheet text/endpoint byte-audit traces as current baseline.
2. Wait for PSM cluster/segment provenance design before naming deeper Sheet geometry fields.
3. Use `--probe-sheet-chunks` evidence to rank record types by frequency.

**Expected Result:** Sheet geometry work starts with provenance anchors instead of isolated heuristics.
