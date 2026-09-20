# 放置实例的实际参数在 `0x00ED JFlavorHolder` 里（2026-09-20）

> 接 `2026-09-18-the-parametric-chain-closes-on-the-template-not-the-instance.md` §8 第一条开口：
> 「放置实例的实际参数在文件的哪一处：五个实例四个被拉过，值不在缓存里」。**找到了**——在实例自己的
> `/JSite<N>/PSMcluster0` 里，一条 PSM 类型 `0x00ED` 的记录，`tools/psm_type_clsid.py` 解出它是 `symbol.dex`
> 的 **`JFlavorHolder`**。它按 `JSymbolInformation` 的变量顺序存一列 `f64`，就是这个实例被拉成现在这样的参数。
> 方法与数字见 §2–§4；复现 §6。**同日解码进 DTO**（计划 `OpenCADStudio/docs/plans/2026-09-20-a-placed-instance-states-its-own-driving-dimensions.md`
> F1）：`decode_flavor_holders` → `JSiteSymbolInformation::flavor_holders` / `instance_values_of` → `PidSymbolVariable::instance_value_m`，
> 棘轮 `a_placed_instance_states_its_own_parameters_in_its_flavor_holder`（A01 Drum 也按 `Left + Right` = 宽、`2 × Top` = 高钉住）。

## 1. 一句话

`Imagineer Document`（被放置点名的实例存储）里，`0x00BD JSymbolInformation` 那份副本记的是**库默认**，
而与它同存储的 `0x00ED JFlavorHolder` 记的是**实例的实际参数**——0201 Manifold 的 `Left' = 57.909 mm`、
`Right = 114.3 mm`、`Top' = 35.590 mm`，工艺 Black Box 的 `Left = 12.7`、`Right = 113.927`、`Bottom = 12.7`、
`Top = 78.067 mm`——与实例几何逐个数对上（§3）。`Server Document`（模板存储）里的 `JFlavorHolder` 不带值，
只用一个 oid 指着自己的 `JSymbolInformation`。

## 2. 怎么找到的

09-18 那份分析在 `Double Value` / `JDim` 两族里找 35.59、57.909、126.6 找不到。这次不按记录找，按**数**找：
把四图每个流的每个字节偏移都当 `f64` / `f32` 读，与拉过的实例的尺寸（米 / 毫米 / 英寸三种单位、含半宽半高与
对模板的比值）比，容差 2e-5。命中只有两处流：

| 图 | 数 | 命中 |
|---|---|---|
| 0201 | Manifold `Left'` 0.057909 m、`Top'` 0.035590 m | `/JSite396/PSMcluster0` +0x5ac / +0x5c6，同一条记录里，中间夹着 0.1143（`Right`）；另两处 0.03559 是两条 `igArc2d` 的半径（几何，已知） |
| 工艺 | Black Box 宽 0.126627 m、高 0.090767 m | 只在 `igLine2d` 的端点里（几何，已知）——**参数本身不是宽高**，是 `Left + Right` / `Bottom + Top`，见 §3 |

+0x5ac 那条记录的帧：`ed 00 | 59 00 00 00 | 42 00 00 00 …`——类型 `0x00ED`，负载 89 字节，oid 66。
`tools/psm_type_clsid.py 0xED` → `0C29A523-1143-11D0-AF3A-080036D72102`，`symbol.dex`，**`JFlavorHolder`**
（与 09-07 文档里的 `0x00EC JFlavorManager` 同族：manager 在根文档一符号一个，holder 在符号定义存储里一份 `SymbolInformation` 一个）。

## 3. 四图逐存储核对

每个嵌套 `PSMcluster0` 里 `JFlavorHolder` 与 `JSymbolInformation` **数量恒相等**（D06 6/6 + 1/1、0201 17/17 + 2/2、
0202 11/11、工艺 7/7 + 1/1）。带变量的那几份：

| 图 / 存储 | 角色 | `0x00BD` 变量（= 库默认） | `0x00ED` 值 | 与实例几何 |
|---|---|---|---|---|
| D06 `/JSite145` | 模板 | oid 22：Left/Right 0.06096、Bottom/Top 0.035306 | oid 13：**无值**，+40 = 22 | — |
| D06 `/JSite151` | 实例（Cone Roof Tank） | oid 13：同上 | oid 14：[0.06096, 0.06096, 0.035306, 0.035306] | = 默认；实例几何是同一公式按毫米算（09-18 §5），半宽 61.06 = **60.96** + 0.1 ✓ |
| 0201 `/JSite329` | 模板 | oid 77：Left/Right 0.1143、Top 0.02032；oid 513：Right 0.0254 | oid 13 → 77、oid 490 → 513，皆无值 | — |
| 0201 `/JSite396` | 实例（Manifold） | oid 13：0.1143 / 0.1143 / 0.02032 | oid 66：**[0.057909, 0.114300, 0.035590]** | 宽 57.909 + 114.3 = **172.209** ✓；高 2 × 35.590 = **71.18** ✓；弧 r′ = **35.590035** ✓（09-18 §5 从几何读出的 Left′ / Right / Top′ 三个数全中） |
| 0201 `/JSite396` | 实例（` Line2`） | oid 117：Right 0.0254 | oid 118：[0.025400] | = 默认，实例几何 = 模板 ✓ |
| 工艺 `/JSite7559` | 模板 | oid 73：四个 0.0127 | oid 66 → 73，无值 | — |
| 工艺 `/JSite6963` | 实例（Black Box） | oid 27：四个 0.0127 | oid 13：**[0.012700, 0.113927, 0.012700, 0.078067]** | 宽 0.0127 + 0.113927 = **0.126627** ✓；高 0.0127 + 0.078067 = **0.090767** ✓（原点在左下角，`Left` / `Bottom` 留默认、`Right` / `Top` 被拉） |

四个实例四个对上（含两个「没拉过」的对照：值 = 默认）；A01 不在本仓夹具里，未验。值的单位是**米**（与 `Double Value` 同），
顺序与 `JSymbolInformation` 的变量顺序一致（Manifold：Left, Right, Top；Black Box：Left, Right, Bottom, Top）。

## 4. 记录形状（语料归纳，未反汇编）

两种形状，同一个类型码：

```text
公共头：ed 00 | u32 负载长度（不含这 6 字节） | u32 oid | 10 字节 0
实例形（Imagineer Document）：
  +20  u32   一个 oid（0201 Manifold 398、` Line2` 598、D06 204、工艺 7535；不是放置的 graphic oid，也不是 site 号——未坐实）
  +24  u16   变量个数 n（3 / 1 / 4 / 4）
  +26  01 01 | u32 1 | u32 0x0c | "Sheets"(UTF-16, 12 字节) | u32 | u32
  然后 n 组：01 01 00 00 00 | f64 值      ← 13 字节一组
  负载长度 = 50 + 13n（89 / 63 / 102 / 102 ✓）
模板形（Server Document）：
  +20  u32 0   +24  u16 0
  +26  01 01 | u32 2 | u32 4 | "SI"(UTF-16) | u32 = 自己那份 JSymbolInformation 的 oid（其 parent_ref 反过来 = 本记录 oid）| u32 0x0c | "Sheets" …
  负载恒 62 字节，无值
```

`+26` 起那个 `01 01 | u32 变体号` 像是 `JFlavorHolder` 持久化的两个分支：`1` = 带值的实例 flavour，`2` = 只指向
`SymbolInformation` 的模板 flavour。变体 1 里 "Sheets" 之后的两个 `u32`（Manifold 0x64 / 0x40，` Line2` 0x86 / …，D06 0x23 / …）
没对上 sheet oid，留 raw。

## 5. 对 pid-parse / OCS 的含义

- 09-18 §7 说「放置实例身上没有属于它的尺寸值，面板只能给库默认或实际外框」——**前半句不再成立**：实例的实际参数就在
  `JFlavorHolder` 里。解码它（变体 1，`n` 组 `f64`，按 `JSymbolInformation` 变量顺序配名）就能给 `PidSymbolDefinition`
  一份 **`instance_variables`**（或给 `PidSymbolVariable` 加 `instance_value`），OCS 面板那行「驱动尺寸（库默认）」旁边就能
  多一行「驱动尺寸（本图实例）」：Manifold `Left 57.91 · Right 114.30 · Top 35.59 mm`，不再只有 `extent=` 的外框。
- 公式常数按英寸、值按米（09-18 §3）不变；把 `JFlavorHolder` 的值代进 `Standard Relation` 应当直接得到实例几何
  （Manifold 的 `0E$1` 无常数，`Top' = r'` 已验；D06 那条「按毫米算」的怪事也许正是 flavour 值 + 图纸单位的产物，值一样、单位不同）。
- 建议下一单：`streams/jsite.rs` 收 `0x00ED`（两变体都框住、只读变体 1 的值）、DTO 加字段（`serde(default)`，golden 只多不改）、
  棘轮钉四对（含两对「= 默认」的对照）、OCS 面板一行。半个工作日。

## 6. 复现

```powershell
cd pid-parse
python tools/psm_type_clsid.py 0xED          # 0x00ED -> symbol.dex JFlavorHolder
cargo run --example probe_jflavorholder_carries_instance_parameters
```

探针按 `sheet_record_starts` 框记录，逐存储把 `0x00ED` 与 `0x00BD` 并排打印：实例形打出 `count` 与值列表，模板形打出 `+40` 指向的
`JSymbolInformation`。§3 的表就是它的输出。

## 7. 还开着的

- 实例形 `+20` 那个 oid 与 "Sheets" 后两个 `u32` 指什么（放置侧的 `JFlavorManager`？sheet？）。
- 模板形为何也叫 flavour holder 而不带值：大概是「flavour = 一组参数值」的空壳，占位等实例化。反汇编 `symbol.dex` 的
  `JFlavorHolder::Load` 可以一次坐实两个变体的每个字段。
- A01 Horizontal Drum 实例（159.459 × 51.917）未验（夹具不在本仓）。
