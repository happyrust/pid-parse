# Phase 34：Sheet 几何完整解析开发计划

> 日期：2026-06-23  
> 目标：把“PID 文件里的几何数据完全解析”拆成可验证的 parser promotion
> 切片，避免把 probe / audit / structural bytes 误交付为 decoded geometry。

## 1. 当前结论

当前实现已经能稳定解码主要 Sheet PSM 几何族：

- `0x3FE6` / `0x0018`：GLine2d / igLine2d
- `0x0084`：igLineString2d
- `0x005E`：igPoint2d
- `0x004D`：igTextBox
- `0x00CE`：igSymbol2d
- `0x0030`：JStyleOverride annotation

但 6 个本地 `.pid` fixture 的 `/Sheet6` byte-audit 仍有 leftover，且
`pid_inspect --geometry-summary` 仍输出 `ProbeOnly Unknown`。因此当前状态是
“主要几何族可用”，不是“完整解析全部几何数据”。

2026-06-26 Phase 34-A inventory 已落地：
`docs/analysis/2026-06-26-phase34-geometry-completeness-inventory.md`。该文档
确认 fixture scope mismatch 后，follow-up 已把 geometry registry 与 Phase 34
PSM probe examples 统一到同一组 6 个本地 PID。下一步是在
`0x0020 igRectangle2d` decoder 前补 byte-layout note。

2026-06-26 Phase 34-B pre-decoder readiness note 已落地：
`docs/analysis/2026-06-26-phase34-0020-igrectangle2d-readiness.md`。结论是
`0x0020` 仍是 preferred drawable candidate，但在 ownership 与 field layout
证明前不得实现 decoder。

2026-06-26 ownership review 已落地：
`docs/analysis/2026-06-26-phase34-0020-ownership-review.md`。normalized probe
显示 `0x0020` 当前命中都在 `/Sheet6615` 或 nested `/JSite204\Sheet6`，不是
primary top-level `/Sheet6`，因此按 ownership-gated candidate 处理。

## 2. 本轮 Probe 事实

现有 probe 输出的跨 fixture undecoded PSM 候选：

| Type | IDA / matrix identity | Hits | Fixtures | 判断 |
|---:|---|---:|---:|---|
| `0x0013` | `igBoundary2d` | 20 | 3 | 命中最多，但更像 boundary / constraint；不能直接 emit 可渲染几何 |
| `0x003D` | `igSmartFrame2d` | 12 | 6 | 结构 / frame 记录；已有 `sub_564464D0` 线索，但不等于普通几何 |
| `0x0020` | `igRectangle2d` | 4 | 3 | 明确 IGDS 几何类型；当前命中在 `/Sheet6615` 和 nested JSite Sheet，先按 ownership-gated 候选处理 |

本地 fixture 当前没有有效命中 `igCircle2d` (`0x0059`)、`igArc2d`
(`0x0061`)、`igEllipse2d` (`0x0063`)、`igEllipticalArc2d` (`0x007E`)
或 `igBSplineCurve2d` (`0x005D`)。

## 3. 推荐执行序列

```text
34-A Geometry Completeness Inventory
  -> 34-B igRectangle2d decoder slice
  -> 34-C Boundary / SmartFrame evidence closeout
  -> 34-D GraphicGroup / 0x0010 non-geometry gate
  -> 34-E Fixture expansion for absent arc/circle/ellipse/bspline
  -> 34-F Full geometry contract update
```

### 34-A：Geometry Completeness Inventory

产物：

- 汇总 6 个 fixture 的 decoded / inferred / probe-only entity counts。
- 汇总 `/Sheet*` byte-audit leftover。
- 汇总 undecoded PSM type histogram。
- 把每个 remaining type 标成 `GeometryCandidate` /
  `StructuralCandidate` / `RelationCandidate` / `NeedsFixture`。

Done：

- inventory 明确说明“完整解析”的可量化定义。
- 不改 parser。

### 34-B：`0x0020 igRectangle2d` decoder slice

首个实现切片只覆盖 `0x0020`，原因是它是当前唯一明确的未解直接几何类型。

实现步骤：

1. 增加 `SheetIgRectangle2dDecoded` DTO 与 `decode_igrectangles` /
   `decode_igrectangle_at`。
2. 用现有 3 条 fixture 样本建立红测试，字段全部绑定 half-open byte ranges。
3. 接入 `SheetGeometry` / schema / cluster pipeline。
4. 在 `geometry.rs` 中 emit `PidGraphicKind::Polyline` 或新增 rectangle
   表达前先评估下游 contract；默认优先用闭合 polyline 表示，避免扩大
   public enum。
5. 更新 byte-audit trace，确保 decoded / audit / probe / leftover movement
   可解释。
6. 加入 panic-safety。

Stop condition：

- 如果 3 条样本不能证明字段 layout，保持 probe-only，不实现 DTO。

### 34-C：`0x0013` / `0x003D` evidence closeout

`0x0013 igBoundary2d` 和 `0x003D igSmartFrame2d` 不能直接当作普通几何。

产物：

- 记录 payload shape、邻接记录、size bucket。
- 若没有 reader evidence，写 negative / deferred closeout。
- 若找到 IDA reader 或 controlled fixture，再单独开 parser slice。

### 34-D：audit-only family guard

保持以下边界：

- `0x00FA GraphicGroup` 仍是 audit-only header + raw tail，除非证明 child /
  reference list。
- `0x0010` 仍是 TypedAudit，不能命名 `sub_kind`，不能 emit geometry。
- `0x0030` 仍是 JStyleOverride annotation，不恢复旧 PrimitiveArc 解释。

### 34-E：Fixture expansion

要真正覆盖“所有几何类型”，必须获得含有以下类型的 fixture 或 controlled
fixture：

- `igCircle2d` (`0x0059`)
- `igArc2d` (`0x0061`)
- `igEllipse2d` (`0x0063`)
- `igEllipticalArc2d` (`0x007E`)
- `igBSplineCurve2d` (`0x005D`)

没有 fixture 或 native reader 前，不声明这些类型 decoded。

### 34-F：完成标准

Phase 34 完成不等于 vendor PID 格式完全公开；它表示当前 fixture 集合内：

- 所有可证明的 drawable PSM geometry type 都有 decoded 输出。
- 非 drawable / structural / relation records 不被误 emit 为 geometry。
- `/Sheet*` leftover 有明确分类：decoded / typed audit / probe /
  needs fixture / needs IDA。
- README / atlas / roadmap 更新为当前证据状态。

## 4. Gate 命令

Planning gate：

```powershell
plannotator annotate goals/phase34-full-sheet-geometry-decode --gate --json
cargo fmt --all -- --check
git diff --check
```

Implementation gate for `0x0020`:

```powershell
cargo test --locked --lib parsers::sheet_records -- --nocapture
cargo test --locked --test parse_real_files rectangle -- --nocapture
cargo test --locked --test parser_panic_safety -- --nocapture
cargo test --locked --lib schema -- --nocapture
cargo test --locked --lib byte_audit -- --nocapture
cargo build --locked --workspace --all-targets
cargo test --locked --workspace --all-targets
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo rustdoc --lib --locked -- -W missing-docs
```

## 5. Stop-And-Challenge

必须暂停的情况：

1. 想把 `0x0013` / `0x003D` 直接 emit 为 drawable geometry。
2. 想恢复 `0x0030` arc 解释。
3. 想把 `0x0010.leading_word` 改名为 `sub_kind`。
4. 想实现 arc/circle/ellipse/bspline，但没有 fixture 或 reader evidence。
5. 想改变 public geometry enum，而闭合 polyline 足够表达 rectangle。
6. 任何 parser promotion 缺少 byte range、fixture ratchet、panic-safety 或
   byte-audit movement。
