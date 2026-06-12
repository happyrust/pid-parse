# Phase 26：PID 文件全格式分析说明开发计划

> 日期：2026-06-03  
> 目标：产出一份面向开发者与下游集成方的 `.pid` 文件格式分析说明，覆盖 CFB 容器、已知 stream/storage、magic/signature、字节布局、解析状态、byte-audit 证据、下游模型映射与剩余未知区。  
> 约束：说明必须区分 `Decoded` / `Probed` / `IdentifiedOnly` / `Leftover`，不得把 investigation 证据写成稳定格式合同。

## 0. 背景与当前事实

`pid-parse` 当前已经形成分层解析体系：

- 外层 `.pid` 是 OLE/CFBF 复合文档，`PidParser::parse_file` / `parse_package` 是入口。
- `parse_package` 保留所有 raw stream bytes，是 round-trip writer、diff 与 byte-audit 的事实源。
- `inspect::coverage` 维护顶层 stream/storage coverage 状态。
- `byte_audit_report` 维护 per-stream consumed / leftover 字节证据。
- Phase 14 以后，`Sheet*` 几何已经从早期 probe 推进到多类 PSM typed decoder，但仍保留多个 audit-only / probe-only 家族。

本阶段不是新增 parser，而是把现有格式知识系统化，形成“可读 + 可验证 + 可继续开发”的格式说明。

## 1. 交付物

### 1.1 主文档

新增：

`docs/analysis/2026-06-03-pid-file-format-analysis-cn.md`

建议章节：

1. 文件整体结构：OLE/CFBF、storage、stream、raw byte preservation。
2. 顶层目录地图：top-level streams 与 known storage prefixes。
3. magic / signature 对照表。
4. stream-by-stream 格式说明。
5. Sheet record families：decoded、audit-only、probe-only 分层。
6. Dynamic Attributes 与 relationship endpoint 的证据链。
7. PSM tables 与 cluster/segment 索引关系。
8. 下游模型映射：`PidDocument`、`PidPackage`、`SheetGeometry`、`NormalizedPidGeometry`、`ObjectGraph`、`CrossReferenceGraph`、`PidImportView`。
9. coverage / byte-audit 验证方法。
10. 未知区与 reopen 条件。

### 1.2 机器可验证附录

可选新增：

- `docs/analysis/2026-06-03-pid-format-coverage-snapshot.json`
- `docs/analysis/2026-06-03-pid-format-byte-audit-snapshot.json`

只有在 fixture 可用且命令可顺利运行时生成；否则主文档记录“未生成快照”的原因。

## 2. Grill-Me 决策树

### Q1：说明文档是“规范”还是“当前实现说明”？

**推荐答案：当前实现说明 + 证据等级，不写成最终规范。**

理由：`pid-parse` 覆盖了大量 `.pid` 格式，但仍存在 `Probed`、`IdentifiedOnly` 与 `Leftover`。如果写成规范，会误导下游把未命名字段当稳定合同。

落地规则：

- 稳定字节布局可写为“已确认”。
- probe 线索必须标注“investigation-only”。
- audit-only DTO 不得暗示业务语义已确认。

### Q2：是否要覆盖 publish XML / MDF 管线？

**推荐答案：放入附录，不并入 `.pid` 主格式说明。**

理由：MDF publish 是独立管线，输入是 `Export.mdf` / legacy SQLite，不是 `.pid` CFB 内部格式。主文档只在“相关但独立”章节说明边界。

### Q3：是否把 Sheet 几何写成完整 decoded？

**推荐答案：只对已落地 typed decoder 家族写 `Decoded`；对 `GraphicGroup`、`0x0010`、coordinate/page metadata、text probe 等继续保留 audit/probe 口径。**

硬边界：

- 不声明 `PidPageTransform::Available` 已可由 page dimensions 推出。
- 不把 `0x0010.leading_word` 写成 `sub_kind`。
- 不把 endpoint topology 写成 CAD 坐标来源。

### Q4：是否需要跑真实 fixture 生成表格？

**推荐答案：需要，但作为验证附录，不阻塞主文档。**

主文档可以先基于代码与现有计划/发现完成；若 fixture 可用，再追加 `--coverage --json` / `--byte-audit --json` 的快照与摘要。

### Q5：说明粒度到哪里为止？

**推荐答案：到 stream/record-family 级，不逐 byte 穷举所有未命名 payload。**

每类格式用统一模板：

```text
名称：
路径：
外层类型：
magic / signature：
字节布局：
当前 parser：
输出模型字段：
coverage 状态：
byte-audit 状态：
已知限制：
下一步证据需求：
```

## 3. 执行阶段

### Phase 26-A：事实源审计

- [ ] 复核 `src/inspect/mod.rs` 的 known stream/storage registry。
- [ ] 复核 `src/inspect/coverage.rs` 的 coverage 状态与 note。
- [ ] 复核 `src/byte_audit/aggregate.rs` 的 registered parser 列表。
- [ ] 复核 `AGENTS.md`、`README.md`、`docs/prd-pid-parse-current-state.md`、`docs/sppid/v0.10.x-status.md`、`docs/architecture-guide.md` 中的过时描述。

Done 条件：

- 得到一份“说明应采用的新口径”清单。
- 明确哪些旧文档偏乐观或已过时。

### Phase 26-B：格式地图主文档

- [ ] 新增 `docs/analysis/2026-06-03-pid-file-format-analysis-cn.md`。
- [ ] 按统一模板覆盖已知 top-level stream/storage。
- [ ] 单独写 `Sheet*` record families 分层：decoded / audit-only / probe-only。
- [ ] 单独写 Dynamic Attributes → relationship → Sheet endpoint 证据链。
- [ ] 单独写 `PidPageTransform` guardrail 与 coordinate/page metadata negative evidence。

Done 条件：

- 文档能回答“一个 `.pid` 文件包含哪些已知格式、解析到什么程度、剩余未知在哪里”。

### Phase 26-C：验证与快照

- [ ] 选择 1-3 个代表性 fixture。
- [ ] 运行 `pid_inspect <file.pid> --coverage --json`。
- [ ] 运行 `pid_inspect <file.pid> --byte-audit --json`。
- [ ] 将摘要写入主文档附录；如生成 JSON 快照，保存到 `docs/analysis/`。

Done 条件：

- 主文档的 coverage 状态与实际 CLI 输出不矛盾。
- 若命令无法运行，文档记录阻塞原因。

### Phase 26-D：文档交叉链接与收口

- [ ] 更新 `README.md` 的文档索引，指向新的格式说明。
- [ ] 视需要更新 `docs/format-notes.md`，标记其为早期简版或链接新版。
- [ ] 更新 `task_plan.md` / `progress.md` / `findings.md`。

Done 条件：

- 新读者能从 README 或格式 notes 进入新版说明。
- 计划三件套与实际交付物一致。

## 4. Stop-And-Challenge

任一条件触发时必须暂停并重新对齐：

1. 文档措辞会让 probe/audit-only 结果看起来像 decoded contract。
2. 需要把 `PidPageTransform::Available` 写成当前已支持。
3. 需要命名 `0x0010` sub-kind、GraphicGroup child list 或其它未 IDA-confirmed 字段。
4. fixture 快照与代码 registry 明显矛盾。
5. 发现 `docs/format-notes.md` / README / AGENTS 与当前实现冲突，且会误导下游。

## 5. 推荐下一步

先执行 Phase 26-A + 26-B，产出主文档草案。随后再跑 fixture coverage/byte-audit，把真实样本快照作为附录补上。
