# Plan: Phase 16 PSM 0x0030 真实归属与 decoder 重写

## 1. 方案总览

Phase 16 是**带前置硬证据的 decoder 重写**，不是从 0 开始的探索。前置
证据已在 Phase 14/15/16 触发轮收齐：

- `0x0030` CLSID 已定（`47FCC338` → `j2dsrv.dll`）
- 64 字节 payload 字段位置已 probe 跨 4 fixture / 98 hit 共识
- attribute tail 含 plant tag + companion coord + reference 链已证实
- 当前 decoder 的 `axis_a.y ≈ 0` 过滤是错误约束（丢 ≈ 51% record）

剩下的工作是把这些证据**典型化**：拿真实字段名（IDA）+ 重命名 +
重写验证 + 全链路联动。

Phase 16 沿用 Phase 14/15 的七层模板，但 Slice 含义升级：

1. **Probe**（已完成）：`probe_garc2d_packed_bytes.rs` 3 轮迭代，证据
   落档 `docs/analysis/2026-05-15-garc2d-packed-int-tail.md` §1-§10。
2. **IDA 反编译**：加载 `j2dsrv.dll`，反编译 `47FCC338` 的 Save / Load /
   Validate / ClassFactory；记录字段表到 `docs/analysis/2026-05-1?-
   j2dsrv-47FCC338-fields.md`。
3. **类名与 DTO 重命名**：根据 IDA 拿到的 RTTI 字符串（或类的 Save
   函数名）决定 DTO 新名。**先与用户对齐命名再动代码**。
4. **Decoder API 重写**：`decode_primitive_arcs` → 新名；payload 字段映射
   按真实字段表；验证规则严格但不假阳；保留 audit-only raw tail。
5. **Unit tests**：canonical + 每一条新验证规则的 rejection + adversarial
   panic safety。
6. **Model / Pipeline 联动**：`model.rs` DTO rename + `From` 重写；
   `cluster.rs` 字段映射更新；`geometry.rs` emission 路径要求用户拍板
   是否新增 `PidGraphicKind` variant；`schema.rs` ratchet 同步。
7. **Pipeline integration test**：`tests/parse_real_files.rs` 的
   `primitive_arc_decoder_emits_decoded_arcs_with_provenance` 改名 +
   baseline 升级到 90+；其他 Phase 14/15 测试 baseline 全保。

## 2. 重命名候选（待 IDA 给出真实类名后定）

不确定真实类名前，候选命名集合：

| 候选 DTO 名 | 适用场景 |
|---|---|
| `SheetJ2DCompositeRecordDecoded` | IDA 给出的类名是 "J2DComposite*" / 通用复合 record |
| `SheetTaggedInstrumentDecoded` | IDA 给出的类名含 "Instrument" / "Tag" |
| `SheetAnnotatedPrimitiveDecoded` | IDA 给出的类名含 "Annotation" / "Annotated" |
| `SheetJ2D47FCC338RecordDecoded` | 最保守（用 CLSID 后缀命名） |

对应 decoder 函数名：

| 候选 decoder 名 |
|---|
| `decode_j2d_composite_records` / `decode_j2d_composite_at` |
| `decode_tagged_instruments` / `decode_tagged_instrument_at` |
| `decode_j2d_47fcc338_records` / `decode_j2d_47fcc338_at` |

**Slice C 完成前必须先与用户对齐最终命名**。

## 3. Slice 表

| Slice | 目的 | 主要文件 | Done when | 风险 |
|---|---|---|---|---|
| A | 加载 j2dsrv.dll 到 IDA + 定位 47FCC338 的 vtable / Save / Load / Validate | IDA instance（新建 port ≥ 13347） | vtable 地址、Save/Load/Validate 函数地址、RTTI 字符串（类名）落入 progress.jsonl | IDA 自动分析时间长 / hexrays 失败 |
| B | 反编译字段表 + 与 probe 字节布局对齐 | `docs/analysis/2026-05-1?-j2dsrv-47FCC338-fields.md` | 写出真实字段表（含字段名、类型、字节 offset、size），覆盖现有 probe 看到的 64B payload + tail 段 | tail 字段在 Save/Load 中不是固定 layout（动态序列化） |
| C | 与用户对齐 DTO / decoder 新名 + 是否新增 PidGraphicKind variant | 与用户问答 | 命名拍板 + variant 决策 | 命名争议 / variant 影响 stable schema |
| D | Parser DTO + decoder API 重写 | `src/parsers/sheet_records.rs` | 新 DTO + 新 decoder 单测全过；adversarial matrix 覆盖 | 验证规则过宽出现 false positive |
| E | Model + pipeline 接入 + schema ratchet | `src/model.rs`, `src/streams/cluster.rs`, `src/schema.rs` | stable DTO 字段定型；schema ratchet 通过；From impl 完整 | 字段重命名波及 enum / serde |
| F | Geometry emission 路径决策 | `src/geometry.rs` | 如新 variant 则 emit；否则 0x0030 record 不进 `PidGraphicEntity`（audit-only） | 下游消费者 break |
| G | Cross-fixture integration test 升级 | `tests/parse_real_files.rs`, `tests/parser_panic_safety.rs` | 4 fixtures 输出 ≥ 90 条 decoded J2D records；Phase 14/15 baseline 不退化 | A01 fixture 上 hit 仅 1 条，统计意义弱 |
| H | 收口 | `progress.jsonl`、`CHANGELOG.md`、verification.md | 5 道 gate 绿，每 AC 有证据，commit 准备好（等用户授权 push） | CI 时间长 |

## 4. Acceptance Criteria

- [ ] **AC1**：`j2dsrv.dll` 已加载到 IDA MCP，CLSID `47FCC338` 的
      vtable / Save / Load / Validate 函数地址记入 `progress.jsonl`。
- [ ] **AC2**：新增 analysis 文档记录真实字段表，覆盖 64B payload + tail
      已知字段；每字段含：byte offset、size、IDA 函数证据、probe 对账
      结果。
- [ ] **AC3**：DTO 与 decoder 已重命名并经用户拍板；CHANGELOG 记录
      破坏性改动。
- [ ] **AC4**：`decode_*` 单测覆盖 canonical record、wrong type、short
      header、truncated payload、invalid size、各字段 sanity rejection、
      panic-free random input。
- [ ] **AC5**：`tests/parser_panic_safety.rs` 覆盖新 public decoder entry。
- [ ] **AC6**：`tests/parse_real_files.rs` cross-fixture guard：4 fixtures
      decoded 总数 ≥ 90（vs 现 48），且 ≤ 98（不引入 false positive）。
- [ ] **AC7**：`model.rs` / `schema.rs` 暴露新 DTO；字段语义与 IDA + probe
      双重证据一致；audit-only tail 仍保留 raw bytes。
- [ ] **AC8**：`geometry.rs` emission 路径按用户决策落地（新 variant 或
      audit-only）；不再用 `PidGraphicKind::Arc` 错误标注 0x0030 record。
- [ ] **AC9**：Phase 14 其他 decoder（igLine2d / igLineString2d /
      igPoint2d / igTextBox / igSymbol2d / GLine2d）跨 fixture 计数与
      Phase 15 `decoded_graphic_groups` audit collection 计数全保。
- [ ] **AC10**：5 道 gate 通过：build / test / clippy -D warnings / fmt /
      missing_docs ratchet（baseline 不上升）。
- [ ] **AC11**：`progress.jsonl` 对 AC1-AC10 都有具体命令 / artifact /
      输出摘要。

## 5. Required Evidence

| Requirement | Evidence to inspect | Where recorded |
|---|---|---|
| AC1 | IDA `list_instances` 输出含 j2dsrv.dll、`decompile` 47FCC338 ClassFactory 的输出 | `progress.jsonl` |
| AC2 | analysis 文档含字段表 + IDA 函数地址 + probe 对账 | git diff + `progress.jsonl` |
| AC3 | DTO / decoder 新名 git diff + 用户对话引用 | `progress.jsonl` |
| AC4 | `cargo test --locked --lib parsers::sheet_records::tests::<new_name>` 通过 | `progress.jsonl` |
| AC5 | `cargo test --locked -j 1 --test parser_panic_safety` 通过 | `progress.jsonl` |
| AC6-AC9 | `cargo test --locked --workspace --all-targets` + 具体 fixture ratchet test --nocapture | `progress.jsonl` |
| AC10 | 5 道 pre-commit gate 输出 | `progress.jsonl` |

## 6. Phase Boundaries

最低可交付版本：

1. AC1 + AC2 完成（IDA 真实字段表落档）
2. AC3 名称对齐
3. AC4 + AC5 + AC6 + AC10 通过

可选条件性交付：

- AC8 的 `PidGraphicKind` variant 新增（用户拍板）
- 新 reference 链字段进 stable DTO（取决于 IDA 证据的稳定性）

如果 IDA 反编译显示 47FCC338 的 Save / Load 是动态字段序列化（不是
固定 offset），则 Phase 16 仅交付：**已识别的稳定字段 + raw tail
audit-only**，更深字段分析延后。

## 7. Completion Audit

声明完成前，逐项对照 AC1-AC11。任何 stable DTO 字段都必须能追溯到
**IDA + probe 双证据**；任何不能证明的 tail 字段必须留在 raw / audit
层；任何重命名都必须在 CHANGELOG 标注 breaking change。

## 8. 与既有阶段的关系

- **Phase 14**：Slice F-I 的 GArc2d decoder 是本阶段的修正对象。
  Phase 14 final summary §6.1 触发本阶段。
- **Phase 15**：`SheetGeometry::decoded_graphic_groups` audit collection
  与本阶段平行；本阶段不动 Phase 15 产物。0x00FA tail 中可能与 0x0030
  reference 链有交集（Phase 16 §3.3 提到 `+32..33` 出现 `0x00FA` =
  GraphicGroup），但 Phase 16 不解开该关联。
- **后续 Phase 17 候选**：J2DSrv 家族 0x29..0x2F + 0x31..0x35 共 12 个
  相同 a5/a6 模式的 record decoder；0x0010 sub-record；J2DComposite 与
  GraphicGroup 的 reference 链。

## 9. 工作树状态注意

本阶段开始时工作树含 Phase 15 的 351 行未提交改动。Phase 16 改动应该
**沿用同一未提交状态**继续叠加，最后由用户决定如何拆分 commit（建议
Phase 15 + Phase 16 各自 squash 成独立 commit）。**不在 Phase 16
内部执行 commit / push**。
