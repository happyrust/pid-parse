# PSM 解码器 deepening 重构 RFC —— 收拢七层模板为两个 seam

> 日期：2026-07-16
> 来源：`/improve-codebase-architecture` 架构评审 → `/grilling` 定稿（候选 1）
> 状态：待评审 / 未开工
> 词汇：module · interface · implementation · depth · deep · shallow · seam ·
> adapter · leverage · locality（沿用 `/codebase-design` 词汇，勿替换成
> component / service / API / boundary）

---

## 1. 问题（friction）

AGENTS.md 自述有一套「reusable seven-layer decoder template」，且「validated
6×」。这正是**重复的 shallow 结构**信号：每新增一个 PSM 记录族，要横改 5 个
文件里近乎复制的脚手架。

已查证的证据：

| 复制点 | 位置 | 复制次数 |
|---|---|---|
| `decode_<type>s` 的 walk+advance 扫描循环 | `src/parsers/sheet_records.rs:2386,2574,2760,2958,3115,3290,3412,3632,3891,4021,4338` | 11 |
| `decode_<type>_at` 的 PSM 头解析（`type_code=word&0x3FFF` / `flags=word>>14` / `bytes_to_follow=u32@+2`） | 同上，每个 `_at` 开头 | 11 |
| `Sheet<Type>Decoded`（L4 解析 DTO） | `src/parsers/sheet_records.rs` | 11 |
| `Decoded<Type>Record`（L3 模型 DTO）+ `impl From` 桥接 | `src/model.rs:1051-1739`（13 个 From） | 12 |
| `SheetGeometry.decoded_<type>` 字段 | `src/model.rs:854-948` | 11 |
| `cluster.rs` 的 import + wiring | `src/streams/cluster.rs:20-27` | 11 |
| `geometry.rs` 逐族 for-arm 拼 `PidGraphicEntity` + note | `src/geometry.rs:729,774,821,869,910,960,1018` | 11 |

对**各族 DTO** 做删除测试：字段真不同（PrimitiveLine=origin/direction/param；
igLine2d=start/end + parent_ref/sub_type_word；igSymbol=insertion + 2×2
transform；jstyle=Annotation 且 confidence=Inferred），删掉后复杂度重现 →
DTO 在赚钱，**保留 typed**。真正冗余的是 DTO 周围的 wiring。

## 2. 已定决策（grilling 结论）

1. **总方向**：方案 A —— trait 只抽脚手架，DTO 保持 typed（associated type）。
   （否掉 enum 塌缩 B 与无类型 map C；C 会违反 CONTEXT.md 的 Typed-Audit 语言。）
2. **seam 落点**：方案 A1 —— 两个 seam 各归其层，避免 L4 反向依赖上层几何（分层
   DAG 见 ARCHITECTURE.md：parsers 是 L4，geometry/derivation 更上层）。
3. **DTO 家族**：保留双家族（L4 解析 DTO 不引 serde / L3 模型 DTO 承担
   `Serialize + Deserialize + JsonSchema` 与 schema ratchet）；`From` 桥接照留；
   registry 只吃 wiring，不碰 DTO。

## 3. 目标设计（deep modules）

### 3.1 L4 解码 seam（`src/parsers/sheet_records.rs`，不引 serde）

吃掉「扫描循环」+「PSM 头解析」两处复制。

```rust
/// 一个 PSM 记录族的解码器。完全属于 L4：只依赖 &[u8] 与自身解析 DTO，
/// 绝不依赖 model 序列化或 geometry 发射。
pub trait PsmRecordDecoder {
    type Record;                                    // 各族 typed 解析 DTO
    fn type_code(&self) -> u16;
    fn decode_at(&self, data: &[u8], off: usize) -> Option<Self::Record>;
    fn advance_of(&self, r: &Self::Record) -> usize;
    /// 现被复制 11× 的 walk+advance；默认实现，极少需 override。
    fn scan(&self, data: &[u8]) -> Vec<Self::Record> { /* 通用循环 */ }
}

/// 同族共享的 PSM 头解析，替换 11 份手写头拆解。
struct PsmHeader { type_code: u16, type_flags: u16, bytes_to_follow: u32, oid: u32, header_end: usize }
fn parse_psm_header(data: &[u8], off: usize) -> Option<PsmHeader>;
```

- 每族 = 一个 unit struct 实现该 trait，`decode_at` 里只剩**该族独特**的
  payload 校验（igLine 的 `remaining_header==12`、GLine 的单位向量容差、
  igsymbol 的 transform 等）。
- trait 用**泛型**方式使用（associated type，非 object-safe），因此不需要异构
  registry；它的价值是消除 scan-loop 与 header 复制。
- 旧自由函数 `decode_iglines` 等在迁移期先保留为**薄包装**转调 trait，最后统一
  收尾。

### 3.2 L6 发射 seam（`src/geometry.rs`）

塌掉 `build_normalized_geometry` 的 11 段 for-arm。

```rust
trait GeometryEmitter {
    /// 把本族的 decoded 记录发射成 0..n 个 PidGraphicEntity。
    /// audit-only 族（0x0013 / 0x0010）实现为 no-op（不 push）。
    fn emit(&self, sheet: &SheetStream, out: &mut Vec<PidGraphicEntity>);
}
const EMITTERS: &[&dyn GeometryEmitter] = &[ /* 每族一个 */ ];
```

- per-family 差异全部留在各自 emitter：`PidGraphicKind`（Line / SymbolInstance /
  Annotation）、`confidence`（Decoded vs Inferred，如 jstyle 是 Inferred）、
  逐族的 note 证据链、audit-only 不发射。对这些做删除测试会重现 → 该保留。
- geometry.rs 主体变成 `for e in EMITTERS { e.emit(sheet, &mut entities) }`。

### 3.3 保持不变

- 双 DTO 家族 + `From` 桥接。
- CONTEXT.md 证据分级：Decoded / Typed Audit / audit-only 不得抹平；
  `0x0013 igBoundary2d`、`0x0010` 继续「只审计不发射 `PidGraphicKind`」。
- 对外公开 API（`pub use model::*` 等）零破坏。

## 4. 分阶段执行（tiny-commit 清单）

每个 commit 遵守仓库 5 道 pre-commit 门禁（build / test / clippy -Dwarnings /
fmt --check / check-missing-docs）+ red-green + **one-commit-one-PR**（`fix`/`refactor`
前缀，squash-merge --delete-branch）。全程守恒：黄金快照不变、`missing_docs`
计数不变、全测试绿。

### Phase 0 — 安全网（1 PR）
- [ ] `tests/` 新增 cross-fixture 黄金快照测试：对 6 个本地 `.pid` 跑
      `build_normalized_geometry`，快照全部 `PidGraphicEntity`（kind + confidence +
      provenance.note + byte_range），fixture 缺失 soft-skip。
- [ ] 跑 5 门禁确认干净起点。
- 验收：快照测试绿；后续每个 Phase 都不得改动该快照。

### Phase 1 — L4 去重 + igLine2d 试点（1 PR）
- [ ] 加 `parse_psm_header` 私有 helper + 单测。
- [ ] 加 `PsmRecordDecoder` trait（含默认 `scan`）+ rustdoc。
- [ ] 实现 `IgLine2dDecoder`，`decode_iglines` 改为薄包装转调 `decoder.scan()`。
- [ ] red-green：断言新路径与旧 `decode_iglines` 对 6 fixture **逐字节/逐记录一致**。
- 验收：黄金快照不变；`missing_docs` 不变。

### Phase 2 — 逐族迁移（10 PR，每族一个）
顺序（按简单→复杂）：igPoint2d → igTextBox → igSymbol2d → igLineString2d →
primitiveLine(GLine2d) → jstyleOverride → graphicGroup → subRecords0x0010 →
attributeFragment → igBoundary2d。
- [ ] 每 PR：实现该族 `*Decoder`，删掉该族复制的扫描/头解析，旧自由函数转薄包装。
- [ ] 每 PR：断言黄金快照不变 + 该族原有单测全绿。
- 验收：每族独立 PR 自洽可回滚。

### Phase 3 — L6 去竖井（1–2 PR）
- [ ] 加 `GeometryEmitter` trait + `EMITTERS` 表。
- [ ] 逐族把 geometry.rs 的 for-arm 迁进对应 emitter；`build_normalized_geometry`
      主体收敛为遍历 `EMITTERS`。
- [ ] audit-only 族（0x0013 / 0x0010）实现 no-op emitter，显式测试「不发射」。
- 验收：黄金快照**逐条不变**（这是本 Phase 最关键的守恒）。

### Phase 4 — 顺手拆 model（候选 6，1 PR）
- [ ] 把 12 个 `Decoded*Record` + `SheetGeometry` + `From` 桥接挪进
      `src/model/sheet.rs` 子模块，`pub use` 保对外零破坏。
- 验收：对外 API 路径不变；全测试绿。

### Phase 5 — 收尾（1 PR）
- [ ] 移除迁移期的薄包装（若无外部依赖），或降为 `pub(crate)`。
- [ ] 刷新 AGENTS.md「七层模板」段 → 「新增族 = 写一个 `PsmRecordDecoder` +
      一个 `GeometryEmitter`」；刷新 ARCHITECTURE.md。
- [ ] 5 门禁全绿 + `cargo audit`。

## 5. 收益（用词汇表术语）

- **locality**：新增 PSM 族从「改 5 文件」降到「加 1 个 decoder + 1 个 emitter」。
- **leverage**：一个 trait 撑起 N 个族 + M 个测试；scan/header 复制归零。
- **interface 收窄；实现吸收样板**：11 份扫描循环 + 11 份头解析 + 11 段 emit
  arm 收进 deep 实现。
- **deletion test 全过**：族竖井删掉→复杂度回到 registry（真在赚钱）；DTO 与
  emitter 的 per-family 逻辑删掉→重现（该保留）。
- **未覆盖族落地成本骤降**：igArc2d(`0x0061`) / igRectangle2d(`0x0020`) 等按
  ownership 解禁后，接入只需两个小 adapter。

## 6. 风险与约束

- **分层守恒**：L4 decoder 绝不引用 `PidGraphicEntity` / geometry（已查证
  sheet_records.rs 当前对 geometry 零依赖，迁移中必须保持）。
- **证据分级守恒**：不得把 audit-only 族误发射为可渲染几何（会违反 ADR-0001
  evidence-complete read-only 与 CONTEXT.md）。
- **黄金快照是唯一真值**：任何 Phase 若改动快照即视为回归，必须定位到「有意的
  行为变更」才允许，且需在同 PR 更新快照并说明。
- **schema ratchet / missing_docs**：新增 pub item 必须带 rustdoc；模型 DTO 若变动
  需同步 `schema.rs` 与 baseline。
