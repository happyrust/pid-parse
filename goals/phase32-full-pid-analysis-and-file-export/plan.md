# Plan: Phase 32 Full PID Analysis And File Export

> **[DRAFT — awaiting Plannotator gate]**

## 1. Solution Overview

```text
.pid source
   ↓ PidParser::parse_package + parse_file
[PidPackage raw bytes] + [PidDocument structured facts]
   ↓ format atlas + confidence ledger
[Decoded / TypedAudit / Probe / IdentifiedOnly / Unknown]
   ↓ export bundle writer
<drawing>.pid.bundle/
   ├ manifest.json
   ├ raw/
   ├ decoded/
   ├ geometry/
   ├ audit/
   ├ writer/
   └ publish/ (optional MDF-backed)
```

Phase 32 treats analysis, parsing, file export, writer, and publish as one product surface with strict evidence boundaries.

## 2. Why This Approach

| 候选 | 优点 | 缺点 | 决策 |
|---|---|---|---|
| A. format atlas + bundle contract first | 降低后续 schema churn；把 evidence/confidence 讲清楚 | 前期文档多 | **采用** |
| B. 直接写 `--export-bundle` | 快速看到目录输出 | 容易把 probe/decoded 混在一起 | 暂缓到 32-C |
| C. 继续 broad IDA | 可能碰运气找到线索 | Phase 30/31 已证明低收益 | 拒绝 |
| D. 继续写 parser promotion | 代码进展快 | 容易越过证据门禁 | 拒绝，除非 32-D 触发条件满足 |

## 3. Phase Slices

| Slice | Purpose | Files | Done when | Risks |
|---|---|---|---|---|
| P0 | 收束当前 Phase 30/31 文档增量 | `docs/analysis/*phase30*`, `*phase31*`, planning files | diff/status 清楚；用户授权后可 commit | 未授权 commit |
| A | PID format atlas | `docs/analysis/2026-06-16-pid-format-atlas-cn.md` | stream/storage confidence matrix 完整 | 文档与代码不同步 |
| B | Export bundle contract | `docs/pid-export-bundle-contract.md` | manifest + directory contract + examples | bundle 过大 / 隐私 |
| C | CLI implementation plan | `pid_inspect --export-bundle` design | implementation slices 和 tests 明确 | 过早实现 |
| D | Byte-layout gated backlog | Phase 31-B table | 每主题有 continue / negative / evidence-needed 状态 | 证据不足却 promotion |
| E | Writer/round-trip boundary | writer docs + plan | editable vs read-only bundle files 明确 | 从 probe 反写 |
| F | Publish XML separation | publish bundle contract | MDF-backed optional export 规则明确 | 混淆 `.pid` 与 MDF |
| G | Verification + docs | README/docs/schema/golden plan | CI smoke 和 five gates 明确 | gate 太慢 |

## 4. Export Bundle Contract Draft

```text
<drawing>.pid.bundle/
  manifest.json
  raw/
    streams.json
    streams/<escaped-path>.bin
  decoded/
    document.json
    metadata.json
    object_graph.json
    cross_reference.json
    import_view.json
    layout.json
  geometry/
    normalized_geometry.json
    decoded_entities.json
    audit_entities.json
    probe_entities.json
  audit/
    coverage.json
    byte_audit.json
    unknown_streams.json
    confidence_ledger.json
  writer/
    round_trip_plan.json
    diff_summary.json
  publish/
    data.xml
    meta.xml
    publish_diff.json
```

Default export should include manifest, decoded document, geometry summary, and audit reports. Full raw stream bytes require explicit opt-in.

## 5. Implementation Order After Gate

1. Write `docs/analysis/2026-06-16-pid-format-atlas-cn.md`.
2. Write `docs/pid-export-bundle-contract.md`.
3. Add `ExportBundlePlan` DTO and pure writer function.
4. Add `pid_inspect --export-bundle <dir>`.
5. Add opt-in raw stream output.
6. Add optional publish XML integration only when MDF path is provided.
7. Add tests and examples.
8. Run full gates.

## 6. Decision Points

| Point | Decision | Default |
|---|---|---|
| Plannotator gate | approve / request changes / unavailable | If unavailable, keep DRAFT |
| Raw stream export | default on/off | off by default |
| Bundle schema version | semantic version in manifest | start `1` |
| Publish bundle | always / opt-in | opt-in |
| Parser promotion during Phase 32 | allow/disallow | disallow unless 32-D evidence trigger |
| Commit strategy | single checkpoint / split | ask user |

## 7. Risk Register

| Risk | Likelihood | Impact | Mitigation |
|---|---|---|---|
| Bundle mixes decoded and probe facts | Medium | High | Separate directories + confidence ledger |
| Raw export leaks proprietary fixture bytes | Medium | High | opt-in + manifest warning |
| Format atlas becomes stale | Medium | Medium | map each row to parser/test/doc anchor |
| CLI implementation expands too fast | Medium | Medium | contract review before code |
| Remaining byte-layout topics tempt guessing | High | High | 32-D evidence triggers + blockers |
| Publish XML and `.pid` parse semantics get conflated | Medium | Medium | optional `publish/` subtree, MDF-backed only |

## 8. Completion Paths

### Planning-only complete

Use when this phase only creates/approves the plan:

```json
{"type":"planning_complete","phase":"32","artifacts":["docs/plans/...","goals/phase32-.../*"],"implementation_started":false}
```

### Implementation-ready

Use when Plannotator approves and user authorizes execution:

```json
{"type":"gate_approved","phase":"32","gate":"plannotator","next":"Slice A format atlas"}
```

### Deferred

Use when gate is unavailable:

```json
{"type":"gate_deferred","phase":"32","gate":"plannotator","reason":"browser panel unavailable or command timeout","next":"wait for user gate"}
```
