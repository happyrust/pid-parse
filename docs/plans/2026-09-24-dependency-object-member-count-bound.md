# `DependencyObject` 的 `+14` 是成员数：上限 16 换成字节自洽校验 · 小计划（2026-09-24 开单，同日批准并落地）

> 起因：OpenCADStudio 计划 `docs/plans/2026-09-24-pid-import-status-and-next-steps.md` H2——`docs/analysis/2026-09-24-the-last-five-refusals.md` 判定
> DWG-0201 `/Sheet6` 那条被拒的 `0x00FA` 是「规则过紧」，按该计划 N-D11「判成解码器缺口再开单」→ 本单。开单时没改代码。
> **2026-09-24 Plannotator 批准**（`{"decision":"approved"}`，无批注），D-D1–D-D3 按推荐执行；E1 / E2 见「进度」。

## 一句话

`decode_dependency_objects` 要求 `group_kind_word`（payload `+14`）∈ 1..=16，可这个字是**成员数**：被接受的记录最短长度恰是 `36 + 8·k`，
DWG-0201 那条 22 个成员的组 212 = 36 + 8·22，字节形状与别的记录一样，只因 22 > 16 被拒。把上限换成「`k ≥ 1` 且 `36 + 8·k ≤ btf`」，
这条就能解出来；它不出几何，所以屏幕不变，变的是 census 与 OCS 导入摘要里 0201 的「没画」1 → 0。

## 事实（2026-09-24，pid-parse `8a83602`）

| 项 | 数 | 出处 |
|---|---|---|
| 被拒的那条 | `0x00322A`，oid 781，parent 6，`+8..14` 全零，`+14` = 22，btf 212 | `examples/probe_the_last_refusals.rs` |
| 最短长度与 `k` | 0201 接受的 135 条：`k` = 1 最短 44、2 最短 52、4 最短 68——`36 + 8·k`；0202 / 工艺同样成立 | 同上 |
| 字节形状 | 16 字节头（oid / parent / 六个零 / `k`）+ `k` × `(u32 成员 oid, u16 1)` + `k` × `u16 1` + 尾部属性块（最短 20 字节，自描述，见 `2026-08-04-graphicgroup-tail-property-block.md`） | 同上 |
| 该族出不出几何 | 不出：`SHEET_RECORD_FAMILIES` 里 `emits_geometry: false`，trace class `Probed` | `src/model/sheet_families.rs:265–278` |
| 谁读它 | `semantics.rs:395, 413`（语义连接的 oid 池与组成员）；census 的 claimed ranges | `rg decoded_dependency_objects src` |
| 语料上别的 `k` | 1 / 2 / 4 / 5 / 10，最大 10；22 只此一条 | 探针 |

## 决策（等批；⭕ = 推荐）

| # | 决策 | 结论 |
|---|---|---|
| D-D1 | 校验怎么换 | ⭕ 删 `group_kind_word > 16`，改为 `k ≥ 1` 且 `36 + 8·k ≤ btf`（成员表与标志数组放得下，尾块至少 20 字节）。备选：逐条核成员表是 `(oid, 1)`——更严，但标志位是不是恒为 1 只有语料证据，先不收紧成新规则 |
| D-D2 | DTO 要不要把成员表解出来 | ⭕ 本单不解：`raw_reference_payload` 原样，字段名 `group_kind_word` 保留、文档改说「成员数」。成员表是 `semantics.rs` 另一条线的事，要解再开单 |
| D-D3 | 验收口径 | ⭕ census 0201 1 → 0；`parse_real_files` 里钉 `0x00FA` 条数的棘轮 +1（实现时按实际改并写明）；`semantic_join` 不变或写明变化；**OCS 四图 `--export` 与 T2 新基线字节相同**（该族不出几何）；OCS 摘要 0201 `missing` 1 → 0、`pid_batch_report` 基线那一行跟着改 |

## 工作项

- **E1 pid-parse**：`decode_dependency_object_payload` 换校验；单测一条合成的 22 成员记录能解、`k = 0` 与放不下的 `k` 仍拒；`render_gap_census` 0201 → `(1 → 0)`，注释改写；CHANGELOG / 分析文档 / `task_plan.md`。
- **E2 OCS**：跑 `pid_import`、四图 `--export` 对哈希、`pid_batch_report` 重出基线（只 0201 的 `missing` / `refused` 两格变）。

## 登记不做

| 项 | 理由 |
|---|---|
| 解出成员表、接进语义 | D-D2 |
| 其他 `Probed` 族的上限复查 | 没有别的拒收指向它们；有了再看 |

## 进度

- **E1（本次提交）**：`decode_dependency_object_payload` 的规则 5 换成 `k ≥ 1` 且 `DEPENDENCY_OBJECT_FIXED_LEN (36) + DEPENDENCY_OBJECT_BYTES_PER_MEMBER (8) · k ≤ btf`；
  `group_kind_word` 文档改说成员数。合成单测：新增 `dependency_object_bounds_its_member_count_by_the_room_for_members`（22 成员 212 字节能解、23 成员拒）；
  原来两条用 `k = 2` 配 44 字节的合成记录改成 52 字节（语料里 `k = 2` 最短就是 52，44 字节放不下两个成员，新规则理应拒它）。
  棘轮：`dependency_object_decoder_ratchets_fixture_counts_and_header_fields` 0201 135 → 136（四图 352 → 353，等于宽口径探针数），里面那条 `(1..=16)` 断言换成同一条字节账；
  `render_gap_census` 0201 `(1, 0)` → `(0, 0)`，注释改写。验证：`--lib` **1119**、`parse_real_files` 135、`render_gap_census` 4、其余集成套件全绿，clippy `-D warnings` 零告警，rustfmt 干净；
  doctest 在本机链接失败（`link.exe` 1140 / 1318 / 1180，环境问题，未验证）。
- **E2（OCS，代码未动）**：因 OCS 主工作树当时正被另一会话改（`rvt` 接入，未提交、编不过），在 OCS `72ae8f42` 的干净 worktree 里对本仓 E1 验：`pid_import` **51/51**、
  `--lib io::pid` **54/54**；`pid_batch_report` 六张图 **DXF 哈希全部不变**（四主图 = OCS T2 新基线），变的只有 0201 一行：`missing` 1 → 0、`refused` `0x00FA:1` → 空。
  OCS 的批量基线 CSV 随之更新（OCS 那边的提交）。

## 门禁记录

- 2026-09-24：OCS 计划 2026-09-24 H2 的结论「规则过紧，另开小单」→ 本单（会话 opus-5-5-1）。三条决策等批。
- 2026-09-24：Plannotator 批准，用户「小单在 Plannotator 里批了，开工 E1 / E2」→ E1 / E2 开工（会话 opus-5-5-1）。
