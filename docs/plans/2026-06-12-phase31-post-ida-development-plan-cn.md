# Phase 31：Phase 29/30 收束后的下一阶段开发计划

> 日期：2026-06-12  
> 输入：Phase 29-A..M parser / byte-audit 收口，Phase 30-A..Q IDA 证据刷新  
> 目标：停止低收益 broad IDA 搜索，把当前成果整理为可 review 的增量，并为剩余 byte-layout 深水区制定带证据门禁的后续路线。

---

## 0. 当前事实快照

| 维度 | 当前状态 |
|---|---|
| Phase 29 parser / byte-audit | A..M 已完成；PSMcluster0 / StyleCluster / DA / nested JSite / JSitesList 等 accounting 与 parser trace 已明显收敛 |
| Phase 30 IDA | `style.dll` 确认 `0x0030 = JStyleOverride` 64-byte persistence；`OLESITE.dll` 确认 `JSitesList` / `JSite` writer-reader 证据 |
| SPPID backend sweep | `smartplantpid.exe`、`sppid.dll`、`sppidautomation*`、`sppiddwgprocess.dll`、`llama.dll` 等已扫；均未暴露 raw CFBF stream / `IOContext` reader |
| Parser 行为 | 当前 Phase 30-Q 不要求 Rust parser 变更 |
| 当前工作树 | Phase 30-Q docs 增量未提交 |
| 明确阻塞 | `PSMspacemap` raw page、StyleCluster prefix、`0x0010` discriminator、GraphicGroup payload、JSitesList stale-tail semantics |

---

## 1. 推荐执行序列

```text
P0 → Phase 31-A → Phase 31-B → Phase 31-C
      文档收束       byte-layout 证据门禁     语义模型候选
```

推荐先做 P0 / 31-A，因为当前主要风险不是缺代码，而是 Phase 29/30 增量太大，需要先变成可 review、可提交、可回滚的形态。

---

## 2. P0：当前文档增量收束

**目标**：把 Phase 30-Q 新增证据与 planning files 对齐，确认提交前状态清楚。

**Scope**：

- 确认新增文件：
  - `docs/analysis/2026-06-12-phase30-sppid-backend-idb-sweep.md`
- 确认同步文件：
  - `docs/analysis/2026-06-12-phase30-ida-gated-next-actions.md`
  - `task_plan.md`
  - `progress.md`
  - `findings.md`
- 保持 Rust parser 不变。

**Done 条件**：

- `git diff --check` 无 whitespace error。
- `git status --short` 只包含预期文档增量。
- 若用户授权 commit，提交信息建议：

```text
docs(analysis): plan Phase 31 after Phase 30 IDA sweep
```

---

## 3. Phase 31-A：review / commit 当前 Phase 29/30 成果

**目标**：把 Phase 29/30 累积成果转成一个可 review 的 checkpoint。

**推荐策略**：

1. **若仓库目标是尽快封存当前成果**：走 single milestone commit。
2. **若 review 成本更重要**：拆成 3 个 review unit：
   - parser / byte-audit changes；
   - probes / specs / snapshot docs；
   - Phase 30 IDA evidence docs。

**当前 Phase 31-A 本轮只制定计划，不自动提交。**

**提交前门禁**：

```powershell
cargo build --locked --workspace --all-targets
cargo test --locked --workspace --all-targets
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
cargo rustdoc --lib --locked -- -W missing-docs
```

**Done 条件**：

- 用户明确授权 commit / push 后再执行 git 写操作。
- commit 后 `git status` clean 或仅剩用户明确接受的 unrelated dirty files。

---

## 4. Phase 31-B：byte-layout 深水区证据门禁

**目标**：避免无证据地继续写 parser，把剩余深水区都改成“触发条件明确”的 backlog。

| 主题 | 当前状态 | 下一步触发条件 | 禁止动作 |
|---|---|---|---|
| `PSMspacemap` raw page | 只有 handle model / segment capacity 证据 | 新 IDB 直接引用 stream page reader/writer，或 controlled fixture 可证明 page 字段 | 仅凭 handle encoding 标记 page bytes decoded |
| StyleCluster prefix | prefix shape 已特征化，仍无 reader | 新 IDB 命中 `StyleCluster` writer/reader 或 style catalog slot IO | 用常量模板匹配冒充 parser |
| `0x0010` family | GUID / leading_word audit-only，discriminator 未恢复 | 找到真实 Read/DoIO sequence 或 sub-kind branch | 命名 `sub_kind` |
| GraphicGroup payload | header + raw tail audit-only | IDA 或 controlled fixture 证明 child/reference list | 命名 child OID list |
| JSitesList stale tail | `entries` IDA-backed，tail stale/delete 未证明 | 找到 delete/compact writer path | 把 `trailing_slots` 视为 active ids |

**Done 条件**：

- 每个主题都有 `Continue / Negative closeout / Needs new evidence` 之一。
- 没有 parser promotion 违反 Phase 18/19/23 guardrails。

---

## 5. Phase 31-C：语义对象模型候选路线

**目标**：把 `llama.dll` 与 `sppiddwgprocess.dll` 的发现放到正确层级：它们不解 raw bytes，但可能帮助后续语义模型 / publish / archive。

**候选交付物**：

- `docs/analysis/2026-06-12-phase31-logical-model-mapping-candidates.md`
- 从 `llama.dll` 字符串建立 `LM*` object taxonomy：
  - `LMADataSource`
  - `LMPlantItems`
  - `LMPipeRuns`
  - `LMDrawing`
  - `LMDrawingSite`
  - `LMRepresentations`
  - `LMRelationships`
  - `LMAAttribute`
- 从 `sppiddwgprocess.dll` 建立 archive/workshare flow inventory：
  - `ISPPidArchive_LoadSPItems`
  - `ISPPidArchive_DrawingPath`
  - `strDrawingXMLFile`
  - `strSharedItemsXMLFile`

**边界**：

- 不把 semantic object model 反推成 raw `.pid` byte layout。
- 不改 Rust DTO，除非能和现有 parsed facts 建立稳定映射。
- 不影响 Phase 31-B 的 byte-layout gating。

**Done 条件**：

- 输出 taxonomy / flow inventory。
- 明确哪些字段可帮助 canonical graph，哪些只能作为产品术语背景。

---

## 6. Stop-And-Challenge

任一触发必须暂停并与用户确认：

1. 计划要求 commit / push，但用户未明确授权。
2. 计划要求把 `Probed` / `AuditOnly` 提升为 `Decoded`，但没有 IDA reader/writer 或 controlled fixture 双证据。
3. 计划要求命名 `0x0010 sub_kind`、GraphicGroup child list、StyleCluster prefix 字段。
4. 计划要求继续 broad IDA 搜索，但没有新的 module path、string clue 或 function clue。
5. 计划要求改 JSON schema 字段名（例如 `entries` → `jsite_ids`），但没有 schema migration 需求。

---

## 7. 本轮工具问题记录

| 问题 | 影响 | 处理 |
|---|---|---|
| `planning-with-files` 的 `session-catchup.py` 在 `C:\Users\dpc\.claude\skills\planning-with-files\scripts\` 下不存在 | 不能自动恢复上次 session catchup | 已改用现有 `task_plan.md` / `progress.md` / `findings.md` 与 git 状态恢复上下文 |

---

## 8. 下一步建议

当前最稳妥的下一步是 **Phase 31-A：review / commit 当前文档与 Phase 29/30 成果**。  
如果用户想继续 IDA，则只接受有直接证据潜力的新模块路径或明确 string/function clue，不再对现有 application / automation modules 做 broad search。
