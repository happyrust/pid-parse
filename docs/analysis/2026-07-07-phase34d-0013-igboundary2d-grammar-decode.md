# Phase 34-D: 0x0013 igBoundary2d — 0x67 标签语法钉死 + 全字段 typed audit-only 解码器

> 日期：2026-07-07
> 范围：Phase 34-D（34-C closeout 推荐的 `0x0013 igBoundary2d` 专属解码器
> slice）。先用专属探针在全部 20 条跨 fixture 记录上钉死 `0x67` 标签语法与
> 计数字段，再落地全字段命名的 **typed audit-only** 解码器
> （`decode_igboundaries`）。**不发射** 归一化几何实体（见「为什么 audit-only」）。
> 证据来源：`examples/probe_0013_igboundary2d_grammar.rs`。

命令：

```powershell
cargo run --quiet --example probe_0013_igboundary2d_grammar
```

## 结论速览

| 问题（34-C 遗留） | 答案（20/20 记录一致） |
|---|---|
| `0x67` 标签的含义与步长 | **每段固定前缀标签**：`segment_count` 组 `0x67 + 4×f64`（33 B/组），自 payload +28 起；不是顶点数组内插字节 |
| 顶点计数字段 | payload `+22` 的 `u32 segment_count`（fixture 全部 = 3），且尾部 `member_count`（u32）与之相等 |
| 顶点数组精确字节范围 | `+28 .. +28+33n`（n=3 → `+28..+127`），之后锚点 2×f64、`u8` flag、`u32 member_count`、n×8B 成员引用，**172 字节整** = `49 + 41n` |
| 边界语义（drawable / hatch / association？） | **association**：尾部成员引用逐条解析到同流真实 `igLine2d` 记录，几何逐段正向精确匹配（60/60） |

## 逐字节布局（payload 偏移，btf = 172, n = 3）

| 偏移 | 类型 | 字段 | fixture 取值 / 校验 |
|---|---|---|---|
| +0 | u32 | `oid` | 各异 |
| +4 | u32 | `parent_ref` | 各异（不校验） |
| +8 | u32 | `remaining_header` | == 12（校验） |
| +12 | u16 | `sub_type_word` | == 0x0010（校验） |
| +14 | u32 | `index` | 21（DWG-0202 族）/ 24（工艺管道-1） |
| +18 | u32 | header flag | == 1（校验） |
| +22 | u32 | `segment_count` | == 3；1..=64 且 `btf == 49+41n`（校验） |
| +26 | u8×2 | `sub_header_tail` | `[2, 1]`（暴露不校验） |
| +28+33i | u8 | 段标签 | == 0x67（校验） |
| +29+33i | f64×4 | 段 i `(start.x, start.y, end.x, end.y)` | 有限且 |v|≤1e9（校验） |
| +127 | f64×2 | `anchor` | 20/20 落在段 bbox 内（有限性校验） |
| +143 | u8 | `trailer_flag` | == 1（暴露不校验） |
| +144 | u32 | `member_count` | == segment_count（校验） |
| +148+8i | u32+u16+u16 | 成员 i：`member_oid, class_word, sub_word` | class 全部 0x00CB；sub = 13/12/12 |

探针汇总（20 条记录全量 dump 后统计）：

```text
total 0x0013 records: 20
closed loops (1e-9):  20        # 段首尾相接成闭环（精确 bit 等值 19/20 差几个 ulp）
member class all 0x00CB: 20/20
member oid resolved:  60/60     # 全部解析到同流 0x0018 igLine2d
member geometry match: 60/60    # member[i] 线段几何 == segments[i]（正向）
anchor inside bbox:   20/20
distinct sub-headers: ×20 [01 00 00 00 03 00 00 00 02 01]（唯一）
distinct trailers:    ×20 trailer_len=45 flag=1 member_count=3 consumed_exact=true
```

（34-B 探针曾报出的「异常标签布局」如 `[28, 31, 41, ...]` 均为 f64 尾数字节
恰为 0x67 的巧合命中；按 33 字节组步长解析后 20/20 全部规整。）

## 为什么 audit-only（不发射几何）

34-C closeout 列出的三种候选语义中，**association（重列成员段端点）** 被
成员引用表坐实：每条 igBoundary2d 尾部的 3 个 `member_oid` 都解析到同流
**已有** 的 canonical `igLine2d` 记录，且第 i 个成员的 `(start, end)` 与第
i 段 payload 坐标正向逐位匹配（60/60，容差 1e-9 内全对）。这些成员线本身
已经由 `decode_iglines` 发射为 `Decoded Line` 实体——若再把 boundary 发射
为闭合 `Polyline`，同一几何会被双计。

于是按 GraphicGroup / 0x0010 的 audit-only 先例落地，但升级为 **全字段
typed**（172 字节每一字节都有字段名），闭环性由
`SheetIgBoundary2dDecoded::is_closed_loop(1e-9)` / DTO 字段 `closed_loop`
提供给下游（如需按闭合区域填充渲染，数据齐备）。若未来 fixture 出现
**无成员引用可解析** 的独立 boundary，再评估几何发射。

## 落地清单（Phase 34-D 实现）

- `src/parsers/sheet_records.rs`：`PSM_TYPE_CODE_IGBOUNDARY2D` /
  `IGBOUNDARY2D_SEGMENT_TAG` / `IGBOUNDARY2D_MAX_SEGMENT_COUNT` /
  `IGBOUNDARY2D_FIXED_PAYLOAD_LEN` / `IGBOUNDARY2D_PER_SEGMENT_LEN`、
  `SheetIgBoundary2dDecoded`（+ `Segment` / `MemberRef`）、
  `decode_igboundaries` + `decode_igboundary_at`（9 条校验规则，见 rustdoc），
  16 个单元测试（canonical / 每条校验拒绝 / 截断与噪声 panic-safety /
  背靠背双记录）。
- `src/model.rs`：`DecodedIgBoundary2dRecord`（+ `Segment` / `MemberRef`
  DTO，含 `closed_loop`）+ `SheetGeometry::decoded_igboundaries`。
- `src/streams/cluster.rs`：Sheet 流水线接入（空集判定同步扩展）。
- `src/byte_audit/aggregate.rs`：整记录按 `TraceConfidence::Decoded`
  认领（全字段 typed；audit-only 仅指不发射几何）。
- `src/schema.rs`：schema needle 棘轮新增 `DecodedIgBoundary2dRecord` /
  `decoded_igboundaries` / `member_oid` / `segment_count` / `closed_loop` 等。
- `tests/parser_panic_safety.rs`：对抗矩阵新增两个入口。
- `tests/parse_real_files.rs`：跨 fixture 棘轮
  `igboundaries_decoder_emits_typed_audit_records_with_provenance`
  （每 fixture 精确计数 5/10/5、其余 0；60/60 成员几何匹配、闭环、锚点
  bbox、provenance 断言）。
- 顺手修复：`geometry_fixture_availability_report_line_is_human_readable`
  的 `registered=5` 陈旧断言（34-A 后注册表已是 6 个 fixture，属未提交
  改动引入的潜在失败，与本 slice 无关但被全量门禁暴露）。

## 验证（2026-07-07 实跑）

```text
cargo build --locked --workspace --all-targets          # ok
cargo test  --locked --workspace --all-targets          # 全绿（lib 944 通过；parse_real_files 104 通过）
cargo clippy --locked --workspace --all-targets -- -D warnings   # ok
cargo fmt --all -- --check                              # ok
missing-docs 棘轮：current=0 == baseline=0              # ok（bash 不可用，按脚本等价 PowerShell 复算）
```

## 边界维持

- `0x0010` / `0x00FA` / `0x0030` 的 audit / typed 边界不变；
- `0x003D igSmartFrame2d` 维持 `StructuralCandidate / NeedsReader`，
  页尺寸标量仍不满足 `ROADMAP-PAGE-TRANSFORM`；
- `0x0020 igRectangle2d` 维持 ownership-gated negative；
- 写出层（`_Data.xml` / `_Meta.xml`）行为零变化。
