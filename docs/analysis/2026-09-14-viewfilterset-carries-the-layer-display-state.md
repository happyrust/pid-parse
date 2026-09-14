# `0x0057 Top ViewFilterSet` 里存着图层的显示状态（2026-09-14）

> 承接 `2026-08-27-tag-184-is-the-sheet-layer-view-hierarchy.md`。那一轮认出了这条记录
> 的头部（`+0` oid、`+12` 形式号 2、`+16` JSheet、`+20..+28` 常量）和尾部的名字表，把
> `+32` 记为「不是图层数、没认」，把中间那段记为「一段显示状态字节（`FF 02 00 …`）」。
> 计划 `OpenCADStudio/docs/plans/2026-09-07-jdim-driving-dimensions-and-layer-panel.md`
> 的 L1 项要求先把 49 条记录的长度账做平，再问显示状态在哪。本文两件都结了。
>
> probe：`examples/probe_viewfilterset_display_state.rs`；
> 棘轮：`tests/parse_real_files.rs::view_filter_sets_state_each_sheets_layer_display_and_close_exactly`；
> 解码器：`src/parsers/view_filter_sets.rs`，挂到 `PidDocument::view_filter_sets`，
> 并把答案写回每个 `SheetLayer::displayed` / `locatable` 和几何实体的
> `PidSourceLayer::displayed`。
>
> **等级：corpus。** 四张主图 49 条 + A01 4 条 = **53/53 精确收尾**；位图的含义靠语料对照组
> 读出（见 §3），没有读 `viewfil.dex` 的原生序列化器。字段名里「显示 / 可定位 / 活动图层 /
> 覆盖」是本文给的读法，不是厂商的名字。

## 1. 长度账：53/53

```text
+0   u32 oid ; +4 u32 0 ; +8 u32 0 ; +12 u32 2 ; +16 u32 JSheet ; +20 u32 1 ;
+24  u32 1 ; +28 u32 0 ; +32 u32 活动图层号 ; +36 u16 0
然后 6 × { u8 0xFF ; u16 len ; len 字节 }            —— 按图层号索引的位图
然后 u16 n ; u16 2 ; n × 覆盖项                       —— 逐图层显示覆盖
     覆盖项 = { u16 图层号 ; u8 kind ; u8 1 ; u16 0 ;
                [kind & 2: u32 COLORREF ; f64 线宽(m)] ; u32 }
然后 12 × u8 0
然后 u32 count ; count × { u32 字符数 ; UTF-16 名 ; u16 图层号 }
```

按这个读法 53 条记录**每一条**在最后一个名字表条目的 `u16` 之后恰好结束，多一字节少一字节
都没有。八种长度（160 / 198 / 222 / 232 / 252 / 276 / 418 / 426 / 434 / 442 / 490）全部由
「位图长度 + 覆盖项数 + 名字数」解释。

**6 张位图的长度**在 49 条 JSite 记录里恒为 `[2, 2, 2, 2, 1, 1]`，四张主图与 A01 的顶层记录
是 `[19, 19, 2, 2, 1, 1]`（D06 顶层例外，仍是 2 字节——它的最大图层号是 15）和
`[36, 36, 2, 2, 1, 1]`（DWG-0202，最大图层号 177）。前两张位图跟着图层号的范围走，
后四张恒定且全 `FF`，没有读法。

**08-27 记的「一段显示状态字节 `FF 02 00 …`」**就是第一张位图的 `{FF ; len=2 ; 2 字节}`
头——那时候没看出 `02 00` 是长度。

## 2. 名字表的 `u16` 是图层号，条目通过 manager 落到唯一对象：309/309

名字后面那两个字节此前算进了「间隙」。它是这个图层的**图层号**——与 `JSheetLayer` 记录
`+16` 的那个 u16 同一个数。JSite 集合里它是 `0..n-1` 的一个排列；顶层集合里它带洞
（D06：`Default:0 Labels:1 DrawingBorder:2 Notes:3 WaterMark:4 Hidden:5 HiddenObjects:6
ConsistencyChecks:7 HeatTrace:8 Label:9 Heat Trace:10 ClaimedOnlyByOthers:15`），
DWG-0202 的 `NotClaimed` 是 `177`，工艺图的 `NotClaimed` 是 `35`。

**（名字, 图层号）本身在一个存储里不唯一**——`DWG-0201/JSite329` 的 `Label:0` 对应 17 个
`JSheetLayer` 对象，每个视图过滤集一份（这正是 08-27 说的「同名图层是多份对象」）。
把范围收到「这个集合的 JSheet → 登记它的 `JSheetLayerManager`（tag 183）→ 该 manager 登记的
图层里同名同号的那个」，**309/309 条目各落到恰好一个对象**（四主图 283 + A01 26）。
`JSheetLayer` 的 `parent_ref` 不是那条 JSheet（0/309），走 manager 才通。

反过来看，四主图 290 个图层对象里 **283 个被某个集合管着，7 个没有**：每个嵌套存储各有
一个 `Default #0`（oid 8，manager 7，对象数 0），是 JSite 登记本自己那张 sheet 的默认层，
那张 sheet 没有视图过滤集。A01 多两个：`/JSite204` 的两个 `Default`（103 与 1 个对象）——
那是 A01 那份「Imagineer Document」嵌套图的正文，它的显示状态文件里**没有**。

## 3. 第一张位图是显示状态

位 `n` = 图层号 `n`，`1` = 显示。对照组全部对得上：

| 读法要求 | 实测 |
|---|---|
| 顶层 `Hidden` / `HiddenObjects` 关（现行名字判据） | 五张图 10/10 关；`HiddenObjects` 在全部 53 个集合里 48/48 关，`Hidden Objects`（带空格）5/5 关 |
| `Default` 开 | 53/53 开 |
| 符号定义缓存里 `Dimension` / `Construction` 关（D2：驱动尺寸与构造几何不上屏） | 五张图凡列出即关：`Dimension` 11/11、`Construction` 11/11 |
| 定义里的 `Label` / `Jacket` / `Heat Trace` 关（标签模板、夹套与伴热变体不随放置画出） | 48/48、43/43、45/45 关 |
| 未分配的图层号 | 位为 1（默认显示），不影响任何对象 |

顶层五张图的读数完全一致：**`Hidden` `HiddenObjects` `Label` `Heat Trace` 关，其余开**。
`Label`（单数）和 `Heat Trace`（带空格）是从符号定义带进顶层 manager 的名字，顶层上对象数
都是 0，关不关看不见；`Labels` 与 `HeatTrace` 开。**`Invisible`** 只出现在两个定义缓存里
（`D06/JSite145` 集合 27、`DWG-0202/JSite793` 集合 3937），**两处都是开**——现行名字判据把
它当隐藏，文件不这么说；语料里没有画出的实体落在它上面，所以 OpenCADStudio 的输出不变，
但判据换成文件状态后这一处的答案会翻过来。

JSite 集合（符号定义的视图）读数：`Default` 开，`Label` `Jacket` `HiddenObjects`
`Heat Trace` `Dimension` `Construction` 关——一个放置的符号只画 `Default` 层上的本体，
标签模板、夹套变体、伴热变体、尺寸、构造线都不画，与 SmartPlant 的实际显示一致。

**§6 那个空缺（`Default` 从不被 184 边指，49/49）**由此有了形状：显示状态不是「登记偏离
基线的图层」的清单，而是每个图层号一位的位图，`Default` 和别的层一样有自己的一位，只是
恒为 1。184 边为什么不指 `Default` 仍未解释，但它不再是显示状态的载体候选。

## 4. 第二张位图：读作「可定位」

顶层四张图（D06 / 0201 / 0202 / A01）它与第一张逐位相同；差异只出在两处：

- 工艺图顶层：`WaterMark:4` 与 `HeatTrace:8` 第一张为 1、第二张为 0——**显示但不可选**，
  水印层正是这么用的；
- 定义缓存里 `Dimension` / `Construction`（以及工艺 `JSite7559` 的 `Hidden Objects:3`）
  第一张为 0、第二张为 1——不显示，但可定位位仍保留默认。

SmartSketch 的图层对话框正是「Display」「Locate」两列。以此把第二张读作 locate；字段名
`locatable` 是本文给的，等 `viewfil.dex` 的读器坐实。

## 5. `+32` 是活动图层号

08-27 记它「36/49 等于图层数、差值不恒定、没认」。换个候选：**它等于名为 `Default` 的图层
的图层号，53/53**（顶层 0；JSite 集合 1 / 4 / 5 / 6，随各定义的编号走）。语料里活动图层
恒为 `Default`，所以「活动图层号」与「Default 的图层号」分不开；解码器叫它
`active_layer_number`，以「活动图层」为读法、以「= Default 的号」为实测。

## 6. 覆盖项：认领状态层的灰显

顶层五张图各带 1 条（工艺 2 条），JSite 集合 0 条：

| 图 | 图层 | kind | COLORREF | 线宽 | 尾字 |
|---|---|---|---|---|---|
| D06 | `ClaimedOnlyByOthers:15` | 3 | `00949494`（灰） | 0.00018 m | 11 |
| DWG-0201 | `NotClaimed:75` | 3 | `003F3F3F`（深灰） | 0.00018 m | 11 |
| DWG-0202 | `NotClaimed:177` | 3 | `003F3F3F` | 0.00018 m | 11 |
| 工艺 | `DrawingBorder:2` | 1 | — | — | 0 |
| 工艺 | `NotClaimed:35` | 3 | `003F3F3F` | 0.00018 m | 11 |
| A01 | `ClaimedOnlyByOthers:21` | 3 | `00949494` | 0.00018 m | 11 |

`NotClaimed` / `ClaimedOnlyByOthers` 是 SmartPlant P&ID 多人认领的状态层，灰显是它们的
惯例；`0.00018 m = 0.18 mm` 是一条细线宽。`kind` 的位 1 决定有没有颜色与线宽这 12 字节
（六条全部按此收尾），尾字 `11` / `0` 未认。DTO 里带 `kind` / `colour` / `line_width` /
`trailing_word`，消费方目前一个都不用。

## 7. 顺带：`0x0088 JSheetLayerGroup` 没有可用的分组

L1 ④ 问它是不是面板分组的现成事实。16 条记录（四主图 11 + A01 5）**逐字节同形**：
`oid ; 0 ; 0 ; u32 1 ; u32 <Default 层的 oid> ; u32 0 ; u32 7 + "Default" ; u32 0`——
每个存储一个叫 `Default` 的组，成员就是那一个 `Default` 图层。没有第二个组名，没有多成员。
**不是分组事实**，图层面板不用它。

## 8. 落到消费方

- `PidDocument::view_filter_sets`（按存储）：集合、其 sheet、活动图层号、逐条目的
  `(name, layer_number, displayed, locatable, layer_oid)`、覆盖项。
- `SheetLayer::displayed` / `locatable` / `view_filter_set_oid`：283/290 个图层对象有答案。
- `PidSourceLayer::displayed`：几何实体自带「文件说这层显不显示」。OpenCADStudio 的
  `is_hidden_sheet_layer` 名字判据由此可以换成文件状态（计划 L1 的最后一步）。
- 金样只多 `displayed` 一个字段，实体不变（`UPDATE_GEOMETRY_GOLDEN=1` 重封）。

## 9. 还开着的

- `viewfil.dex` 的原生读器没读：位图 3–6、覆盖项的 `kind` / 尾字、`+32` 的「活动」
  一说，都停在 corpus 级。
- 位图 1 / 2 的字节长度（2 / 19 / 36）从当前图层号推不出来（工艺最大 35 却给 19 字节），
  像是 manager 曾分配过的号的高水位；不影响读，因为它自带长度。
- 184 边为什么不指 `Default`（49/49）仍未解释。

## 10. 复现

```powershell
cd pid-parse
cargo run --example probe_viewfilterset_display_state
cargo test --test parse_real_files view_filter_sets_state
cargo test --lib parsers::view_filter_sets
```
