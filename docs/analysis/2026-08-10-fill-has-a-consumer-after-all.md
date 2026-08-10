# 填充有消费者：全语料 20 条 `igBoundary2d` 全部指到 `JStyleSimpleFill`

> 日期：2026-08-10
> 范围：`pid-parse`（解码）+ `OpenCADStudio`（消费）
> 结论类型：**推翻**——Phase 39 判 E「可见占比实测为 0，解了也没得画」不成立。
> 前置：`docs/plans/2026-08-07-phase39-render-gap-triage-cn.md` §1 E、
> `2026-08-05-geometry-index-is-the-style-link.md`（几何 `index` → 样式）
> 工具：`examples/probe_fill_style_consumers`

## 0. 一句话

Phase 39 把填充族 `0x002A` / `0x002B` 封存，理由是「Sheet 流里没有任何填充/实心面
几何族，36 条填充样式无几何消费者」。**消费者一直在，只是没往那儿看**：
`0x0013 igBoundary2d` 就是那个「面」族，全语料 **20 条边界记录，20 条**都经
`JStyleOverride` 指到一条 `JStyleSimpleFill`。

## 1. 实测

`style_link` 的模块文档其实早就顺口说过反话——「an override referencing a
`JStyleSimpleFill` is a well-formed record describing a filled object」——
计划与它不可能都对。把每条几何记录的 `index` 按 `style_link` 的走法（直接一跳 +
经 `JStyleOverride` 一跳）落地：

| 几何族 | 落到填充 | 没落到 |
|---|---:|---:|
| **`igBoundary2d`** | **20** | **0** |
| `igLine2d` | 0 | 326 |
| `igLineString2d` | 0 | 153 |
| `igPoint2d` | 0 | 186 |
| `igTextBox` | 0 | 184 |

**20/20 全中，849 条线/点/文字一条都不沾。** 这个分布本身就是判据：填充是「面」的
属性，只有面族引用它——如果这是巧合，它不会这么干净地按族分层。

链路是三条 override：

| fixture | override id | 指向 | 边界条数 |
|---|---:|---|---:|
| DWG-0202GP06-01 | 21 | `0x002A` id 20（46 字节载荷） | 5 |
| 工艺管道及仪表流程-1 | 24 | `0x002A` id 23（46 字节载荷） | 10 |
| DWG-0202（publish 副本） | 21 | `0x002A` id 20 | 5 |

被引用的全是 `0x002A JStyleSimpleFill`（实心），没有一条 `0x002B JStyleHatchFill`
（图案）被引用。DWG-0201 / D06 / A01 定义了填充样式但没有边界记录，所以它们那边
确实无消费者——**「无消费者」是三张图的局部事实，被当成了全语料结论。**

## 2. 可见占比不是 0，是两张图上的 15 个面

去掉 publish 副本，语料里有 **15 个被填充的封闭区域**：DWG-0202 五个、工艺管道-1
十个。它们现在在 OCS 里**一个像素都不画**——不是画成白的，是根本不发：
`igBoundary2d` 的 emitter 是注册过的 no-op，理由记在册上是「与成员线重复」
（边界的成员线已经由 `igLine2d` / `igLineString2d` 画出来了）。

所以现状是：图纸上有 15 块实心面，导入后只剩它们的轮廓线。这不是「解了也没得画」，
是「有得画，但两头都还没接」。

## 3. 顺带一个对不上的数

计划与格式指南 §5 记的是 `0x002A` 25 条、`0x002B` 11 条，合计 36。本次按
`DocumentStyleTable::from_stylecluster_bytes`（**精确走记录链**）数出来是 **22**
（每个文档 2–3 条 `0x002A` + 1 条 `0x002B`）。

差值未解释。**提出一个假设并标明它是假设**：指南那张表的计数来自滑动扫描式探针，
而链式走查只认记录起点——今天 `0x3FE6` 那一课（见
`2026-08-10-gline2d-is-the-iso-page-ratio-not-a-record.md`）正是同一种差异造成的。
要证实，把两种计数在同一条流上并排跑一次即可，成本很低；**在跑之前不要拿 22 去改
指南**。

## 4. 它们是流向箭头，而且已经接上了

**20/20 边界在 `1e-9` 下闭合成环**（解码器自带的 `is_closed_loop` 早就这么说，
本轮重测确认）。而且形状全都一样：**3 段、1.6×3.2mm 或 3.2×1.6mm 的三角形**——
这是 P&ID 管线上的**实心流向箭头**。它们此前被画成空心三角（成员线画出三条边，
没人填），现在按图纸说的填上。

两侧改动：

| 仓 | 改动 |
|---|---|
| `pid-parse` | `style_link` 新增 `resolve_fill` / `fill_styles_for_file` / `ResolvedFill`（**只说「填不填」，不说「填什么颜色」**——载荷未解，两件事可以分开） |
| `pid-parse` | `IgBoundary2dEmitter` 从 no-op 改为发闭合 `Polyline`，note 里写明「环重列了成员线，渲染方必须填而不是描」 |
| `OpenCADStudio` | 新增 `PID-FILL` 图层；`build_fill` 把带填充的环画成实心 `HATCH`，颜色取图层默认 |

**颜色是图层默认，不是从图纸读的**，`build_fill` 的文档里写清了这一点：
猜一个颜色比用一个声明过的默认值更糟，而两者都比留空心接近事实。

## 5. 本轮没有做、也不该顺手做的

- **没有解 `0x002A` 的任何字段。** 46 字节里哪几个是颜色、哪几个是透明度/图案 id，
  一个都没看，也不该在这里猜。这是下一刀：手法同 `0x002E` / `0x002F`
  （CLSID → `style.dll` vtable → 版本分发序列化器 → 字段偏移），`dlls/style.dll.i64` 还在。
- **没有处理「面与线重叠」的一般情形。** 当前语料里每个环都有填充，所以「发环」
  不会产生重复描边。**一条没有填充的边界会**——`pid-parse` 照样发环，由消费方决定
  不描它。OCS 已经这么做（`build_fill` 只在 `fill_for` 命中时接管），但这条约束
  只写在 note 里，没有测试守着。
- **没有改指南里的 25 / 11。** 见 §3。

## 6. 复现

```powershell
cargo run --example probe_fill_style_consumers
```
