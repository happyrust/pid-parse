# Phase 25 跨 fixture 几何 inventory 收敛快照

> 日期：2026-05-29
> 范围：在 IDA 路径受阻（`radsrvitem.dll` 不可得）的前提下，按"非 IDA 保守路径"
> 收敛当前 cross-fixture normalized geometry inventory 与 `probe_only_unknowns`，
> 并把 `docs/plans/2026-05-23-phase25-next-development-plan-cn.md` 中已过时的
> `inferred_lines = 0` 背景数据正式取代。
> 复现命令：
> ```powershell
> cargo test --test parse_real_files -- --nocapture `
>   geometry_fixture_inventory_reports_normalized_geometry_counts `
>   normalized_geometry_probe_baseline_on_real_fixture `
>   available_pid_fixtures_geometry_evidence_inventory_tracks_promoted_hints `
>   dwg0201_produces_inferred_endpoint_lines `
>   dwg0201_emits_decoded_primitive_lines_without_inferred_regression
> ```
> 本快照仅做 read-only 验证 + 文档收敛：**未新增 typed RAD DTO、未改 normalized
> geometry transform、未 promotion 任何 probe-only 记录**。

---

## 0. 收敛结论（一句话）

5 个可用 fixture 全部通过 inventory 与 baseline 断言；`inferred_lines` 在
DWG-0201 已稳定在 **49 floor + 26 decoded GLine2d**（计划文档旧值 `0` 作废），
全部 `probe_only_unknowns` 仍为 audit-only，**没有在缺 IDA 证据下被 promotion**。

---

## 1. 跨 fixture normalized geometry inventory（5 fixture）

来源：`geometry_fixture_inventory_reports_normalized_geometry_counts`
（`d`=Decoded / `i`=Inferred / `p`=Probe-only）。

| Fixture | category | points d/i/p | lines d/i/p | polylines d/i/p | texts d/i/p | symbols d/i/p | unknowns d/i/p | other |
|---|---|---|---|---|---|---|---|---:|
| `DWG-0201GP06-01.pid` | dwg | 75 / 117 / 0 | 26 / 49 / 0 | 39 / 0 / 0 | 45 / 0 / 0 | 4 / 0 / 0 | 0 / 0 / 19 | 20 |
| `DWG-0202GP06-01.pid` | dwg | 31 / 76 / 0 | 42 / 3 / 0 | 28 / 0 / 0 | 45 / 0 / 0 | 5 / 0 / 0 | 0 / 0 / 42 | 30 |
| `工艺管道及仪表流程-1.pid` | non_ascii | 36 / 64 / 0 | 218 / 0 / 0 | 49 / 0 / 0 | 43 / 0 / 0 | 17 / 0 / 0 | 0 / 0 / 12 | 47 |
| `export-test/publish-data/A01/A01.pid` | publish_a01 | 4 / 132 / 0 | 1 / 0 / 0 | 3 / 0 / 0 | 9 / 0 / 0 | 1 / 0 / 0 | 0 / 0 / 19 | 1 |
| `export-test/publish-data/DWG-0202GP06-01/DWG-0202GP06-01.pid` | publish_dwg | 31 / 76 / 0 | 42 / 3 / 0 | 28 / 0 / 0 | 45 / 0 / 0 | 5 / 0 / 0 | 0 / 0 / 42 | 30 |

`line_producing_fixtures` = 全部 5 个；`fixtures_seen = 5`，无 missing fixture。

**一致性校验**：`DWG-0202GP06-01.pid` 的 publish 副本（`publish_dwg`）与独立
fixture 每一格数字逐项相同 → publish 通道未引入几何漂移。

---

## 2. `probe_only_unknowns` 收敛

| Fixture | probe_only_unknowns | 说明 |
|---|---:|---|
| `DWG-0201GP06-01.pid` | 19 | audit-only（`0x0010` / GraphicGroup 家族残留） |
| `DWG-0202GP06-01.pid` | 42 | audit-only |
| `工艺管道及仪表流程-1.pid` | 12 | audit-only |
| `A01/A01.pid` | 19 | audit-only |
| `publish-data/DWG-0202GP06-01.pid` | 42 | 与独立 DWG-0202 一致 |
| **去重后总计（独立 fixture）** | **92** | 19 + 42 + 12 + 19（不含 publish 副本重复的 42） |

这些 `probe_only_unknowns` 全部对应 audit-only PSM 记录族（`0x0010` 多态子记录 /
GraphicGroup / JStyleOverride 模板），按 Phase 18/19/20 边界**保持 probe-only，
不命名 `sub_kind`、不发 `PidGraphicKind`**。在没有 IDA-confirmed RAD class 身份前
（见 §4 阻塞），不为其创建 typed DTO —— 与 Phase 25 计划 §3 Stop-And-Challenge
第 2、3 条一致。

---

## 3. DWG-0201 `/Sheet6` probe baseline（单 sheet 明细）

来源：`normalized_geometry_probe_baseline_on_real_fixture`。

| 字段 | 值 |
|---|---:|
| text | 9 |
| coord | 64 |
| endpoint (ep) | 59 |
| hint | 53 |
| decoded_line (GLine2d) | 2 |
| decoded_igline | 24 |
| decoded_iglinestring | 39 |
| decoded_igpoint | 75 |
| decoded_igtextbox | 45 |
| decoded_igsymbol | 4 |
| decoded_jstyle_override | 20 |
| **geometry.entities 总计** | **394** |

`dwg0201_produces_inferred_endpoint_lines`：`inferred_points = 117`，
`inferred_lines = 49`。
`dwg0201_emits_decoded_primitive_lines_without_inferred_regression`：
`inferred_lines = 49`（≥ 49 floor 未退化），`decoded_GLine2d_lines = 2`。

> **取代旧值**：`docs/plans/2026-05-23-phase25-next-development-plan-cn.md` §0 背景表
> 中 `inferred_lines (H7CAD 可见) = 0` 为 Phase 10 Slice 3 阶段快照，**现已作废**。
> endpoint pair → inferred line 闭环实际已落地：59 个 endpoint 中 49 条配对成功，
> 剩余 10 条由单端 endpoint（only_a / only_b）解释，属预期非配对，不再围绕
> 旧 `inferred_lines = 0` 推进。

---

## 4. 几何证据 hint inventory + IDA 阻塞状态

来源：`available_pid_fixtures_geometry_evidence_inventory_tracks_promoted_hints`。

| 指标 | 值 |
|---|---:|
| fixtures / sheets / windows | 5 / 3 / 6337 |
| record_shape_classes | 328 |
| identities | 437 |
| same_object / wrong_object | 17 / 420 |
| identity_supported | 44 |
| max_identity_score | 105 |
| identity_over_threshold | 28 |
| promotable | 67 |
| text_candidates | 578 |
| **text_over_threshold** | **0** |
| object_geometry_hint_count | 67 |

`text_over_threshold = 0` → text probe 仍全部 probe-only，未达 promotion 门槛，
与 `sheet6_text_window_report_keeps_text_probe_only_until_position_is_proven`
gate 一致。

**IDA 阻塞（维持 negative）**：当前 IDA Pro MCP active instance 仅
`D:\AVEVA\Everything3D3.1\core.dll.i64` reachable，缺 PID PSM 关键字符串
（`PSMSerializeIn/Out`、`GLine2d`、`igLine2d`、`BytesToFollow`、`guidtab.h` 均无命中）；
`radsrvitem.dll`（历史端口 13346）与 `style.dll`（13348）实例不可达，本地亦未发现
`radsrvitem.dll` 文件。因此 `0x0010` / GraphicGroup 的 RAD class 语义无法继续推进，
保守路径维持 audit-only。

---

## 5. 下一步 gate（待用户确认）

1. **是否启动保守 parser 切片**：仅增强 audit/provenance 与缺口报告
   （例如把 §2 的 `probe_only_unknowns` 按 PSM size bucket 归类输出到独立
   inventory 报告），**不新增 typed RAD DTO**。
2. **是否提供 `radsrvitem.dll`**（或在 IDA 中打开并恢复 13346 实例）：这是唯一能
   解锁 `0x0010` sub-kind 语义的路径；在此之前不创建 typed RAD class DTO。
3. 若两者都暂不推进，本快照即为 Phase 25 inventory 的收敛存档，DWG/A01/非 ASCII
   三类 fixture 的几何覆盖已稳定，无回归。

---

## 6. 引用

- `tests/parse_real_files.rs`：`geometry_fixture_inventory_reports_normalized_geometry_counts`、
  `normalized_geometry_probe_baseline_on_real_fixture`、
  `available_pid_fixtures_geometry_evidence_inventory_tracks_promoted_hints`、
  `dwg0201_produces_inferred_endpoint_lines`、
  `dwg0201_emits_decoded_primitive_lines_without_inferred_regression`
- `docs/plans/2026-05-23-phase25-next-development-plan-cn.md`（§0 背景表被本快照取代部分）
- `docs/analysis/2026-05-23-phase25-slice-a-probe-output.md`（f64 空间分布）
- `AGENTS.md` Phase 14 decoder suite + `0x0010` audit-only 边界
