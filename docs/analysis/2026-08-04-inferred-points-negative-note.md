# 被丢弃的 inferred Point：负结论 + `ig*` 类表补全

> 日期：2026-08-04
> 范围：`pid-parse`（解码）+ `OpenCADStudio`（渲染）
> 结论类型：**negative note**——不实现，并说明为什么不实现。
> 所有数字来自本次实跑（`examples/probe_inferred_points`、
> `examples/probe_psm_type_code_histogram`、`OpenCADStudio/examples/pid_probe`）
> 与 `radsrvitem.dll` 的 IDA 反编译，非引用历史文档。

## 0. 起因

`OpenCADStudio/src/io/pid.rs` 的 `build_inferred()` 只画 `Annotation` 与
`Line`，把 inferred `Point` 整类丢掉。四张 fixture 合计丢 321 条，是 inferred
里最大的一类，看上去像「有证据但没上屏」的最大一块。

本文测量这 321 条到底是什么，结论是：**丢得对，不应该画。**

## 1. 三个来源，分别定性

inferred `Point` 有三个上游，`geometry.rs:557-680` 各自 emit：

| 来源 | 机制 | 定性 |
|---|---|---|
| `coordinate_hints` | 在流上按 4 字节对齐滑窗，任意相邻两个非零 `i32` 且 \|v\| ≤ 1e6 就记一条，每流上限 64 条（`sheet_probe.rs:2213`） | **纯字节扫描，不是记录** |
| `object_geometry_hints`（i32 位置） | 窗口打分过门槛（`passes_primary_gate`，阈值 70） | 启发式，坐标空间不对 |
| `object_geometry_hints`（f64 回退） | 同上，取记录里的 `f64` 对 | 唯一在正确坐标空间里的 |

## 2. 实测：321 条里有多少能画

判据：decoded 坐标是「页面为单位的米」，四张图页幅都在 1m 以内，所以真坐标必须
落在 `|v| ≤ 1`。再扣掉退化到原点的、以及与已画 decoded 实体重合（< 0.1mm）的。

| fixture | 来源 | n | 在页内 | 在原点 | **新增可画** | max\|v\| |
|---|---|---:|---:|---:|---:|---:|
| DWG-0201 | coordinate_hints | 64 | 1 | 0 | 1 | 9.83e5 |
| DWG-0201 | geometry_hints i32 | 11 | 0 | 0 | **0** | 1.54e5 |
| DWG-0201 | geometry_hints f64 | 42 | 42 | 3 | **2** | 5.74e-1 |
| DWG-0202 | coordinate_hints | 69 | 2 | 0 | 2 | 9.18e5 |
| DWG-0202 | geometry_hints i32 | 6 | 1 | 1 | **0** | 6.33e5 |
| DWG-0202 | geometry_hints f64 | 1 | 1 | 1 | **0** | 2.23e-308 |
| D06 | coordinate_hints | 64 | 3 | 0 | 3 | 9.18e5 |
| 工艺管道-1 | coordinate_hints | 64 | 1 | 0 | 1 | 8.52e5 |

**321 条里「新增可画」合计 9 条，且没有一条站得住：**

- 7 条来自 `coordinate_hints`，坐标全是 `(1000.0, 0.0)mm`、`(1000.0, 1000.0)mm`、
  `(0.0, 1000.0)mm`——就是字节流里挨着的整数 `0` 和 `1` 被当成米放大了 1000 倍。
  是扫描噪声，不是位置。
- 2 条来自 DWG-0201 的 f64 hint：`(402.0, 573.8)mm` 在 420.3mm 高的页面上**超出
  页面**；`(244.0, 0.0)mm` 贴在页底边。都不像绘图对象。
- DWG-0201 那 42 条 f64 hint 里，剩下 37 条**与已画的 decoded 实体重合**——它们是
  已有对象的锚点副本，画出来是在原地叠第二个点。

## 3. IDA 交叉验证：`igPoint2d` 是唯一的点族

`radsrvitem.dll!sub_56448F70` 是 type_code → 类名的权威表。完整反编译后的族清单：

**几何族**：`0x13 igBoundary2d`、`0x18 igLine2d`、`0x20 igRectangle2d`、
`0x21 igComplexString2d`、`0x3D igSmartFrame2d`、`0x4D igTextBox`、
`0x59 igCircle2d`、`0x5D igBSplineCurve2d`、`0x5E igPoint2d`、`0x61 igArc2d`、
`0x63 igEllipse2d`、`0x7B igGroup`、`0x7E igEllipticalArc2d`、
`0x84 igLineString2d`、`0xCE igSymbol2d`、`277 igDimension`、`279 igBalloon`、
`280 igLeader`。

**约束族（`*Relation2d`，不是几何）**：`0x06 igPointOnRelation2d`、
`0x0F igParallelRelation2d`、`0x15 igPerpendicularRelation2d`、
`0x17 _TangentRelation2d`、`0x19 igKeyPointRelation2d`、
`0x40 igConcentricRelation2d`、`0x69 igSymmetricRelation2d`、
`0x6A igEqualRelation2d`、`0x6B igColinearRelation2d`、`0x77 igFixRelation2d`、
`0x82 igHorizontalRelation2d`、`0x85 igTangentRelation2d`。

**`0x5E igPoint2d` 是整张表里唯一的点族**，全 6 个 fixture 共 187 条全部解码
（34 字节定长，`oid / parent_ref / sub_type_word / index / f64 x / f64 y` 字段全读）。
不存在第二个点记录类型可供 inferred hint 去「补」。语料侧与原生 reader 侧两个方向
得到同一结论。

## 4. 顺带关掉三个悬空 type code

`docs/analysis/2026-07-27-pid-load-status-snapshot.md` 第 3 节把 `0x0006` /
`0x007B` / `0x0085` 记为「未定」。上面的类表直接定性：

| code | 类名 | 定性 |
|---|---|---|
| `0x0006` | `igPointOnRelation2d` | 参数约束，无独立坐标，**永不可画** |
| `0x007B` | `igGroup` | 分组容器，无几何 |
| `0x0085` | `igTangentRelation2d` | 参数约束，**永不可画** |

三个都不是几何缺口，应从缺口表移除而不是继续排期。

## 5. 顺带纠正一个统计口径错误

用「证据条数」算解码率会严重低估覆盖，因为分母里混了字节扫描 hint。按 **PSM 记录**
算才是真实覆盖：

| fixture | PSM 记录 | 几何族记录 | 实际 emit 几何 | GraphicGroup 容器 | 页框 | 未覆盖 |
|---|---:|---:|---:|---:|---:|---|
| DWG-0201 | 359 | 217 | 205¹ | 136 | 6 | 0 |
| DWG-0202 | 279 | 188 | 169 | 84 | 1 | 5× `0x0013` + 1× `0x0020` |
| D06 | 48 | 26 | 26 | 21 | 1 | **0** |
| 工艺管道-1 | 563 | 427 | 404 | 125 | 1 | 10× `0x0013` |

¹ 含 2 条 `GLine2d`（与 `igLine2d` 不同族）。

emit 数低于几何族记录数的差额基本都是空文本的 `igTextBox`：DWG-0202 是 56 条
`igTextBox` 只出 45 条 `Text`，DWG-0201 是 59 出 45。这些是没有内容的文本框，不画
是对的，但目前没有单独计数，值得在报告里显式区分「记录存在但内容为空」与
「记录没解出来」。

**D06 的 48 条记录 100% 有归属**（26 几何 + 21 分组 + 1 页框）。它渲染稀疏是因为
图本身就只有这么多内容——**没有一条 `igLine2d`**——不是解码漏了东西。此前
「D06 解码率只有 26%」的说法是口径错误造成的。

`0x0013 igBoundary2d` 是**故意不 emit**（与成员线重复），不是缺口。

## 6. 语料覆盖的真实边界

类表里有 9 个几何族在 6 个 fixture 里**一次都没出现**：`igCircle2d`、`igArc2d`、
`igEllipse2d`、`igEllipticalArc2d`、`igBSplineCurve2d`、`igComplexString2d`、
`igDimension`、`igBalloon`、`igLeader`。

其中 `igDimension` / `igBalloon` / `igLeader` 是真实 P&ID 常用的标注族。语料里没有
不等于格式里没有——**换一批客户图纸就可能直接撞上**。这是比 inferred Point 更值得
排期的风险，但需要先拿到含这些族的图纸才能取证。

## 7. 结论

1. **不实现** inferred `Point` 的绘制。321 条里 0 条是应当上屏的绘图内容。
2. 建议把 `pid.rs` 里 `build_inferred()` 丢弃 `Point` 的注释从「坐标是 ±900k 的
   原始 i32」升级为引用本文，把「为什么不画」从经验说法变成有量化依据的结论。
3. 三个悬空 type code 与「D06 解码率低」都应从缺口表移除。
4. 真正剩下的缺口只有两个：**文字样式**（硬阻塞于 `/StyleCluster 0x005A`）与
   **线宽/颜色/线型**（未取证）；外加一个语料风险：**标注族未见于语料**。
