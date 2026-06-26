# Phase 32：完整 PID 格式分析与文件化解析总体开发计划

> 日期：2026-06-16
> 输入：Phase 29/30 parser + byte-audit 收束、Phase 30/31 IDA 证据刷新、当前 `pid-parse` 读写/publish 能力
> 目标：把“继续分析 `.pid` 格式”和“把 `.pid` 解析成可交付文件集合”统一成一条可执行路线，明确每阶段产物、证据门禁、验证命令和停止条件。

---

## 0. 总体目标

最终交付不是单个 parser 函数，而是一套可审计的 `.pid` 文件解析产品：

1. **格式分析资产**：持续更新的 PID format atlas，列出所有已知 stream/storage、字节布局、证据来源、confidence 与剩余 unknown。
2. **结构化解析 API**：`PidParser::parse_file` / `PidParser::parse_package` 输出稳定 `PidDocument` / `PidPackage`，保留 raw provenance。
3. **文件化导出**：把单个 `.pid` 拆解为一个目录化 bundle，包含 manifest、raw streams、decoded JSON、geometry、object graph、cross-reference、byte-audit 和可选 XML/publish 产物。
4. **保真写回**：保持 passthrough-first writer，可做 metadata / XML / controlled byte patch，不把未证明语义写回。
5. **交付门禁**：所有 promotion 必须由 byte-range provenance、fixture ratchet、byte-audit baseline 和必要时 IDA / controlled fixture 双证据支撑。

---

## 1. 当前能力基线

| 领域 | 当前状态 | Phase 32 判断 |
|---|---|---|
| CFB/OLE 容器 | 已能枚举树、stream、CLSID、timestamp、state bits | 作为文件化导出的 raw layer |
| Metadata/XML/Summary | 多数 fully decoded，部分可写 | 进入 decoded JSON + editable metadata layer |
| Object graph / crossref / layout | 已可生成可读图和关系 provenance | 进入 semantic bundle，但保留 confidence |
| Sheet/PSM geometry | 多个 typed decoder 已落地；`0x0030` 已纠正为 `JStyleOverride` | 进入 decoded/audit/probe 三层输出 |
| Publish XML | MDF-first A01 主线成熟，DWG 侧 soft-gated | 作为独立 publish bundle，不混同 `.pid` raw decode |
| Writer | passthrough round-trip + metadata/XML replacement | 只扩大已证明修改面 |
| Byte-audit | 框架成熟，仍需 baseline 化 | 作为每个 bundle 的必备验收报告 |
| IDA evidence | `style.dll` / `OLESITE.dll` / `OLECRT.dll` 有关键正证据；app/automation broad sweep 收敛 | 后续只追 direct stream-reader 线索 |

---

## 2. 推荐执行序列

```text
P0 → 32-A → 32-B → 32-C → 32-D → 32-E → 32-F → 32-G
收束   格式图谱  文件化合同  导出实现  深水区证据  写回/roundtrip  发布/验收
```

### P0：Phase 29/30/31 增量 checkpoint

**目标**：先把当前未提交的 Phase 30/31 文档增量收束为可 review checkpoint。

**Scope**：

- `docs/analysis/2026-06-12-phase30-ida-gated-next-actions.md`
- `docs/analysis/2026-06-12-phase30-sppid-backend-idb-sweep.md`
- `docs/analysis/2026-06-12-phase31-commit-readiness-review.md`
- `docs/analysis/2026-06-13-phase31-olecrt-storage-entrypoints.md`
- `docs/plans/2026-06-12-phase31-post-ida-development-plan-cn.md`
- `task_plan.md` / `findings.md` / `progress.md`

**Done**：

- `git diff --check` 通过。
- 用户明确授权后再 commit。
- 若暂不 commit，Phase 32 只作为 planning 增量继续保留。

### Phase 32-A：PID format atlas

**目标**：把 `.pid` 格式知识从散落的 README / PRD / findings / phase docs 汇总为可维护的 format atlas。

**产物**：

- `docs/analysis/2026-06-16-pid-format-atlas-cn.md`
- stream/storage confidence matrix：
  - `Decoded`
  - `TypedAudit`
  - `Probe`
  - `IdentifiedOnly`
  - `Unknown`
- 每个条目记录：
  - stream/storage path
  - parser module
  - model field
  - byte-audit trace
  - fixture coverage
  - IDA / controlled fixture evidence
  - remaining blockers

**禁止**：仅因文档汇总而改变 Rust parser confidence。

### Phase 32-B：文件化解析 bundle 合同

**目标**：定义“把 PID 解析为文件”的目录合同，先定格式再写实现。

推荐 bundle：

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

**设计原则**：

- raw bytes 永远可追溯，不因 decoded 层存在而丢失。
- decoded / audit / probe 分层落到文件名和 manifest。
- bundle manifest 记录 `pid-parse` 版本、source file hash、fixture label、generation command。
- 未证明的字段保留 raw/probe 输出，不出现在 decoded JSON 的稳定字段中。

### Phase 32-C：`pid_inspect --export-bundle` 实现

**目标**：新增 CLI 出口，把现有 `PidDocument` / `PidPackage` / reports 组合成 bundle。

**实现切片**：

1. `ExportBundlePlan` DTO：控制是否包含 raw streams、geometry、audit、publish。
2. `export_bundle(package, document, plan, out_dir)`：只做文件写出，不重新解析。
3. `pid_inspect --export-bundle <dir>`：默认输出 manifest + raw index + decoded document + audit。
4. `--export-bundle-raw-streams`：显式打开 raw stream `.bin` 写出，避免默认产生过大目录。
5. `--export-bundle-publish <mdf>`：可选挂接 MDF publish XML，不与 `.pid` parse 强绑定。

**测试**：

- synthetic package bundle shape。
- real fixture soft-skip bundle shape。
- manifest source hash / command-line provenance。
- Windows path escaping。
- raw stream opt-in。

### Phase 32-D：剩余 byte-layout 深水区专项

**目标**：继续分析格式，但只在触发条件满足时推进。

| 主题 | 当前状态 | 继续条件 | 交付 |
|---|---|---|---|
| `PSMspacemap` raw page | handle model 已知，page bytes 未知 | direct stream reader/writer 或 controlled fixture | typed page decoder / negative closeout |
| StyleCluster prefix | shape 已特征化，reader 未知 | `StyleCluster` open/read/write path | prefix field naming 或保持 probe |
| `0x0010` discriminator | audit-only + leading_word | real branch/read sequence | sub-family typed decoder |
| GraphicGroup payload | header + raw tail | child/reference list 证据 | child list DTO 或 negative closeout |
| JSitesList stale tail | entries IDA-backed，tail 未知 | delete/compact writer path | tail semantics doc / parser update |
| `0x0089` / DA heads | byte pattern + export boundary | class/family reader evidence | field naming update |

### Phase 32-E：Writer 与 round-trip 边界加固

**目标**：让文件化解析产物可用于安全写回，但不扩大未证明编辑面。

**范围**：

- metadata XML edit。
- Summary / DocumentSummary 已证明字段。
- arbitrary stream replacement。
- experimental SheetPatch 保持显式 unsafe/experimental。
- bundle 中输出 `round_trip_plan.json`，说明哪些文件可编辑、哪些只读。

**禁止**：

- 从 decoded geometry JSON 反写 Sheet bytes。
- 从 probe/audit 字段反写语义字段。
- 自动 compact / regenerate unknown streams。

### Phase 32-F：Publish XML 分线收束

**目标**：把 MDF publish XML 明确作为 parallel product line，和 `.pid` bundle 互相引用但不混淆。

**产物**：

- `publish/` bundle 子目录只在提供 MDF / legacy sqlite 时生成。
- A01 parity gate 保持 hard。
- DWG fixture 缺失时 soft-skip，并在 manifest 标记 `publish_dwg_status = "not_verified"`.

### Phase 32-G：产品化验收与文档

**目标**：形成用户可运行、CI 可守门、下游可消费的交付面。

**交付**：

- README 增加 `--export-bundle` 使用示例。
- `docs/pid-export-bundle-contract.md`。
- `docs/analysis/2026-06-16-pid-format-atlas-cn.md`。
- schema golden / JSON examples。
- CI smoke：小 fixture bundle shape + no panic + manifest hash。

---

## 3. Plannotator / goal package

本计划配套 goal package：

```text
goals/phase32-full-pid-analysis-and-file-export/
  brief.md
  plan.md
  verification.md
  blockers.md
  goal-prompt.md
  progress.jsonl
```

建议在执行代码切片前先对五件套跑 Plannotator gate。若 gate 无法完成，本计划保持 DRAFT / planning-ready，不进入 implementation。

---

## 4. Stop-And-Challenge

任一情况必须暂停：

1. 用户未明确授权 commit / push。
2. 想把 `Probe` / `AuditOnly` 字段升级为 `Decoded`，但没有 byte-range + fixture + IDA/controlled fixture 证据。
3. 想让 bundle 默认输出所有 raw stream bytes，导致大文件或敏感数据风险。
4. 想从 geometry/probe JSON 反写 `.pid` Sheet bytes。
5. 想继续 broad IDA 搜索，但没有 direct stream name / `IOContext::DoIO` / persist manager clue。
6. 新 bundle schema 会破坏既有 `PidDocument` schema，且没有 migration plan。

---

## 5. 推荐下一步

1. 跑 Plannotator gate 审阅 Phase 32 goal package。
2. 若通过，先做 P0 checkpoint。
3. 启动 Phase 32-A format atlas；它是后续 bundle 合同和 parser promotion 的共同索引。
4. Phase 32-B 合同通过 review 后，再实现 `pid_inspect --export-bundle`。
