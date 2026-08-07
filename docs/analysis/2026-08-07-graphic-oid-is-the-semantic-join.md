# `GraphicOID` 是语义 join —— 单图成立，判据修订为「两跳」

> 日期：2026-08-07
> 范围：`pid-parse`（Phase 38 S1，见 `docs/plans/2026-08-06-phase38-semantic-link-and-render-gap-closeout-cn.md` §2）
> 结论类型：**H38-1 在 DWG-0202 上成立（判据按实测修订为两跳规则）；A01 无判别力（覆盖缺口），
> 跨 fixture 确认未达成。** 不是 negative note，也不够宣布跨 fixture。
> 证据：`examples/probe_graphic_oid_join.rs` 对
> `test-file/export-test/publish-data/{DWG-0202GP06-01,A01}/` 两对
> `.pid` + `_Data.xml` 的实跑（只读，无解码器改动）。

## 1. 假设与判据

**H38-1**：`_Data.xml` 的 `IDrawingRepresentation/@GraphicOID` 与 Sheet 记录的
`oid` 是同一个 OID 空间，一条 `PIDRepresentation` 对应该 OID 的一条或一组 Sheet 记录。

判据按计划 §2.2：**不用命中率**（OID 是文档内稠密小整数，命中率是弱判据，
§8.1 的 `index` 误判负教训），用**落到哪一类记录族**：

- 图形族（发 `PidGraphicKind`）：`GLine2d` / `igLine2d` / `igLineString2d` /
  `igPoint2d` / `igTextBox` / `igSymbol2d`；
- 非图形族：`JStyleOverride`（样式）、`DependencyObject`（依赖边）、
  `igBoundary2d`（关联，不发图元）、`igSmartFrame2d`（页框）。

零假设有三个层次，全部算出来写在下面：同空间检验、单族全覆盖检验、图形类检验。

## 2. DWG-0202：39/39 在池，同空间铁证

发布侧 39 个 `GraphicOID`（互不相同，103…7342）；Sheet 侧类型化解码池 264 个
OID（1…65541，169 图形 / 95 仅非图形）。

| 检验 | 结果 | 零假设概率 |
|---|---|---|
| 同空间：39/39 发布 OID 都在解码池 | 成立 | **9.769e-58**（池内 250 个值落在发布窗口 103..7342 的 7240 个值里，39 连中） |
| 单族全覆盖：`igSymbol2d` 全族 23/23 被发布集包含 | 成立 | **5.256e-23** |
| 图形类：39 个在池 OID 全部落图形族 | **不成立**（27/39） | （若全中，P=4.572e-9） |

落点分布：

| 落点族 | 条数 | 类别 |
|---|---|---|
| `igSymbol2d 0x00CE` | 23（= 该族全部） | 图形 |
| `igPoint2d 0x005E` | 4 | 图形 |
| `DependencyObject 0x00FA` | 12 | 非图形 |

第一层结论：**两边是同一个 OID 空间**，这一点没有别的解释（9.8e-58）。
但严格版判据（39 个无一落非图形族）被 12 个 `DependencyObject` 落点打破——
按 Stop 条款先停，做只读旁证再下结论。

## 3. 旁证一：那 12 个落点不是死胡同，是聚合节点

`0x00FA` 的 payload 尾部带 OID 引用列（`+22` / `+34`，见
`2026-08-04-graphicgroup-tail-property-block.md` §3；该文 ⚠ 修正段已证实这些
引用是依赖关系的两端）。把 12 条记录的尾部 4 字节窗口对解码池求交：

| 发布 OID | 记录形状 | 依赖边指向 |
|---|---|---|
| 103 | btf=154 kind=4 | +34→1257 `igSymbol2d`，+50→139 `igLineString2d`，+26/+70→1 `JStyleOverride` |
| 562 | btf=154 kind=4 | +22→130 `igPoint2d`，+50→561 `igLineString2d`，+26→1 |
| 564 | btf=154 kind=4 | +34→1837 `igSymbol2d`，+50→556 `igLineString2d`，+26/+70→1 |
| 569 | btf=154 kind=4 | +34→2320 `igSymbol2d`，+50→580 `igLineString2d`，+26/+70→1 |
| 1317 | btf=154 kind=4 | +22→582 `igPoint2d`，+50→1316 `igLineString2d`，+26→1 |
| 2064 | btf=154 kind=4 | +34→1590 `igSymbol2d`，+50→2067 `igLineString2d`，+26/+70→1 |
| 2065 | btf=154 kind=4 | +22→2063 `igPoint2d`，+50→2066 `igLineString2d` |
| 2071 | btf=154 kind=4 | +22→1323 `igPoint2d`，+50→2060 `igLineString2d`，+26→1 |
| 2941 | btf=154 kind=4 | +34→612 `igSymbol2d`，+50→4478 `igLineString2d`，+70→1 |
| 3155 | btf=170 kind=4 | +34→4508 `igSymbol2d`，+50→4101 `igLineString2d` |
| 3830 | btf=186 kind=4 | +34→3140 `igSymbol2d`，+50→458 `igLineString2d` |
| 5576 | btf=186 kind=4 | +50→5474 `igLineString2d` |

**12/12 都依赖至少一个图形 OID；12/12 都在 `+50` 引用一条 `igLineString2d`。**
这批记录全是 `kind=4` 的大桶（btf=154/170/186），与尾列文档研究过的
btf=66/kind=2 桶不同形——`+50` 在这批桶里是引用列，是新观察。

判读：发布表示指到 `0x00FA` 时，指的是一个**聚合节点**（一条管线 = 符号 +
折线 + 端点的成组依赖），它的叶子就是画出来的图形。**没有一个发布 OID 是
死胡同。** 于是判据修订为：

> **两跳规则**：`GraphicOID` 或者直接命中图形族记录（27/39），或者命中一个
> `DependencyObject`、且该记录的依赖边一跳可达图形族（12/39）。
> 两跳之外仍到不了图形族的发布 OID 才计为反例；DWG-0202 上反例为 0。

这不是「改判据凑命中率」：原判据把 `DependencyObject` 归为反例的理由是
「依赖边不是图形」，而实测证明发布侧就是会指聚合节点——判据修订有字节证据
（引用列解析）支撑，且修订后的判据**更强**（要求把边解析出来，而不是放宽）。

## 4. 旁证二：A01 是覆盖缺口，不是反例

A01 发布 4 个 `GraphicOID`（24601/24606/24613/24615；图很小：1 台卧式罐 +
1 个管嘴 + 1 条管线，`_Data.xml` 才 8.2KB），但 4 个都不在 Sheet 类型化
解码池里（池仅 29 个 OID）。两种解释必须分开：**文件里根本没有这些标识符**
（配对错位，反例）vs **有但类型化解码没够到**（覆盖缺口，无判别力）。

对全部 75 条流做 LE u32 原始字节扫描：

| 发布 OID | 命中位置 |
|---|---|
| 24601 | `/PSMspacemap\0x00000000` ×1，`/Unclustered Dynamic Attributes` ×1 |
| 24606 | `/PSMspacemap\0x00000000` ×1，`/Unclustered Dynamic Attributes` ×1 |
| 24613 | **`/Sheet6` ×1**，`/PSMspacemap\0x00000000` ×1，`/Unclustered Dynamic Attributes` ×1 |
| 24615 | `/PSMspacemap\0x00000000` ×1，`/Unclustered Dynamic Attributes` ×1 |

四个值**都在文件里**，且分布是系统性的：每个都在 `PSMspacemap`（OID 空间
映射表）和 `Unclustered Dynamic Attributes` 里恰好出现一次，24613 还出现在
`Sheet6` 的未解码区域。单个 4 字节窗口的偶然命中概率对 63KB 语料约 1.5e-5，
四个目标在同两条流上各中一次，不是巧合能解释的。

判读：**A01 的 `.pid` 与 `_Data.xml` 配对没有错位**，缺席是因为 A01 的
Sheet 内容大部分不在现有类型化解码器够得到的地方（29 个池 OID 对 106KB 的
文件，覆盖明显薄；这张图恰好是 `GLine2d` unresolved unit-line 问题的
出处）。A01 对 H38-1 **无判别力**——它既不确认也不否定。

顺带记录：A01 池里有个 1344733186（0x50262002）的离群 OID，量级与全池
（其余 ≤ 66k）不符，疑似某族解码误命中，S2 做无声丢弃告警时值得一并看。

## 5. 结论

1. **同空间（H38-1 前半）**：成立。DWG-0202 上 P=9.8e-58；A01 的原始字节
   证据（4/4 在 `PSMspacemap`）方向一致、不矛盾。
2. **表示→图形（H38-1 后半）**：按两跳规则在 DWG-0202 上成立，反例 0/39；
   其中 `igSymbol2d` 全族被发布集覆盖（P=5.3e-23），12 个聚合节点全部一跳
   可达图形。
3. **跨 fixture 确认：未达成。** 计划 §2.2 要求两张图都成立；A01 无判别力，
   语料里没有第三对 `.pid` + `_Data.xml`。这一条必须如实说：S3（语义 API）
   若开工，其证据基础是**单 fixture 强证据**，不是跨 fixture。
4. 计划 §2.4 的「不成立→退化」不触发：本结果不是 negative。

## 6. 对 Phase 38 后续的影响

- **S2（无声丢弃→点名告警）不受影响**，照做。A01 的覆盖缺口正是 S2 要让
  用户看见的那类事实（`Sheet6` 里有发布侧指名的 OID 躺在未解码区域）。
- **S3（语义 API）**：证据等级如上，单图成立。开工与否是计划裁量点：
  若开工，`_Data.xml` 仍是可选输入（Stop 条款 2 不变），且 join 实现必须
  带两跳解析（直接命中 + `DependencyObject` 边解析），不能只做直接命中。
- **A01 要重获判别力**，路径是补 Sheet 解码覆盖（`Sheet6` 未解码区域、
  `PSMcluster` 侧），那是独立的取证切片，不该挤进本 phase。

## 7. 复现

```powershell
cargo run --example probe_graphic_oid_join -- `
  test-file\export-test\publish-data\DWG-0202GP06-01\DWG-0202GP06-01.pid `
  test-file\export-test\publish-data\A01\A01.pid
```

探针输出四段：族分布 + 三个零假设统计（§2）、`dependency-edge resolution`
（§3）、`raw-byte scan for absent oids`（§4）。
