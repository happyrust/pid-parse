# `0x0115 JDim` 是一条有帧的记录，块长随尺寸种类走（2026-09-14）

> 承接 `2026-08-04-annotation-families-risk.md`（认出 `sub_564BA320` 那条五级 reader，判
> 「全语料 0 命中、不建议写解码器」）与 `2026-08-27-the-recordless-182-referrers-are-symbolinformation.md`
> （`0x006F` Standard Relation 的出参 13/13 是 JDim、spacemap tag-188 的 46 条成员里 42 条
> 落在 `0x0115` payload 里）。计划
> `OpenCADStudio/docs/plans/2026-09-07-jdim-driving-dimensions-and-layer-panel.md` 的 J1 项
> 要求：先把 18 条逐字段摆开，再拿原生读取器坐实，最后写清**哪些字段坐实、哪些留 raw**。
>
> probe：`examples/probe_jdim_bytes.rs`；
> 原生侧：`tools/idalib_radsrv_igdimension.py`（radsrvitem 的 igDimension 链）、
> `tools/idalib_imagdex_jdim.py`（imagdex 侧的 RTTI 走查，**没找到 DoIO**，见 §6）；
> 类名解析：`tools/psm_type_clsid.py`。
>
> **等级：帧是原生读法背书，块内是 corpus。** 记录的外框（34 字节前缀 + `main_len` 块区 +
> 条件尾字）不是语料拟合出来的，是 `radsrvitem.dll!sub_564BB990` 自己的算术；块内各字段的
> 含义仍只有语料证据，**没有解码器，没有 DTO**——本文只立字节账，不进 `src`。

## 1. 语料：18 条，全在符号定义缓存里

| 图 | 存储 | 条数 | 图层 | payload 长度 |
|---|---|---:|---|---|
| D06 | `/JSite145/PSMcluster0` | 5 | 35 `Dimension`（#5）| 198 / 308 / 276 / 198 / 166 |
| DWG-0201 | `/JSite329/PSMcluster0` | 5 | 44 `Dimension`（#4，3 条）、507 `Dimension`（#4，2 条）| 194 / 308 / 198 / 194 / 194 |
| DWG-0202 | — | 0 | — | — |
| 工艺管道及仪表流程-1 | `/JSite7559/PSMcluster0` | 4 | 102 `Dimension`（#4）| 288 / 198 / 198 / 288 |
| A01（导出件）| `/JSite39/PSMcluster0` | 4 | 150 `Dimension`（#4）| 194 / 198 / 308 / 194 |

六种长度：166×1、194×5、198×6、276×1、288×2、308×3。`DWG-0202` 一条都没有与它没有参数化符号一致。
每条的 `parent_ref` 是所在定义 sheet，`sheet_layer_ref` 是该存储里名为 `Dimension` 的图层——
而那个图层在视图过滤集里**实测为关**（`2026-09-14-viewfilterset-carries-the-layer-display-state.md` §3，
`Dimension` 11/11 关）。**文件自己说这批记录不上屏**，与计划 D2「默认不画」一致。

## 2. 帧：`payload = 34 + main_len + tail`，18/18

```text
+0   u32 oid ; +4 u32 parent_ref ; +8 u32 sheet_layer_ref ;
+12  u16 sub_type 0 ; +14 u16 尺寸种类 ; +16 u16 0
+18  u16 版本 2 ; +20 u16 0|1 ; +22 u32 ; +26 u16 标志字 ; +28 u16 0 ; +30 u32 main_len
+34  块区，长度由尺寸种类决定（本语料 48 字节）
…    其后各段，直到 34 + main_len
+34+main_len  u32 —— 只在标志字含 0x0100 时存在
```

18 条全部在最后一字节处精确收尾。**这不是拟合**：`radsrvitem.dll!sub_564BB990` 以**记录头**
为基址（头一句 `*(WORD*)a2 == 277`，所以它的每个偏移比 payload 小 6），尾字取自

```c
*(_DWORD *)(*(_DWORD *)(a2 + 36) + a2 + 40)   // = payload + 34 + main_len
```

且只在 `*(DWORD*)(a2+32) & 0x100`（= payload `+26` 的标志字）时读。语料里 `0x100` 与 `0x200`
从不单独出现，探针最初只能按 `0x0300` 判；**读取器把这两位分开了**，探针随之改判 `0x100`，仍 18/18。

标志字取值与尾字：无尾的是 `0x0041`（1 条）/ `0x0051`（4 条），带尾的是 `0x0341`（1）/ `0x0351`（11）/ `0x0361`（1）。
`0x0041` 是 09-07 那份清单没有的新值（A01）。

## 3. 头部逐字段

| 偏移 | 读法 | 证据 | 状态 |
|---|---|---|---|
| `+0` / `+4` / `+8` | oid / parent_ref / sheet_layer_ref | 七个图元族共用的 18 字节信封 | 坐实（既有） |
| `+12` u16 | `sub_type`，18/18 为 0 | 信封 | 坐实（既有） |
| `+14` u16 | **尺寸种类**，18/18 为 1 | 原生：第二个子读取器按它在 **8 个值**间分派，每种一个块读取器 | **坐实** |
| `+16` u16 | 18/18 为 0；原生：`==1` 时第三、四个子读取器才走一段 64 字节数组 | 原生 | 坐实（语料恒 0） |
| `+18` u16 | 版本 2（18/18） | 原生第四个子读取器按它分支 | 坐实 |
| `+20` u16 | 0（16 条）/ 1（工艺两条 288）；原生把它过 `sub_564B82A0` 重映射表后存 `this+24` | 原生 | 半坐实：是个码，含义未解 |
| `+22` u32 | 6（16 条）/ 49（0201 两条）；原生原样存 `this+756`，不解释 | 原生 | **留 raw** |
| `+26` u16 | 标志字。`&0x0F`、`(>>4)&3`、`0x0100`（有尾字）等约十六个位 | 原生逐位展开 | 帧相关的 `0x0100` 坐实，其余留 raw |
| `+28` u16 | 18/18 为 0（与 `+26` 合成原生那一个 dword 读） | 原生 | 坐实 |
| `+30` u32 | `main_len` | 原生用它算尾字地址 | **坐实** |

## 4. 块区：`+34` 起，块长随尺寸种类

`sub_56446B50` 给块长，这也是原生侧怎么跳过它去找后面几段的：

| 尺寸种类 | 块长 |
|---|---|
| 1 / 8 | **48**，块首 dword 含 `0x2000` 时 80 |
| 2 | 48，`0x100` 时 64 |
| 3 / 4 / 5 | 48 |
| 6 | 48，`0x400` 时 64 |
| 7 | 52 |

语料 18 条全是种类 1 且块首无 `0x2000`，所以块 = `+34..+81` 共 48 字节——**这正解释了
「为什么下一段从 `+82` 开始」**，也解释了第一轮探针的困惑「一条读法盖不住 102 / 70 / 82 三种块长」：
块长本来就随种类走，而语料**八种只练到一种**。

块内 48 字节（种类 1）：

| 偏移 | 值 | 读法 |
|---|---|---|
| `+34` f64 | 恒 ≈ `3.05e-5` m（2⁻¹⁵，低位有微小变化） | 疑公差 / 精度，**留 raw** |
| `+42` f64 | **尺寸值**：3.81 / 10.16 / 12.7 / 20.32 / 25.4 / 35.56 / 63.5 / 114.3 mm | 18/18 都是英寸整倍（0.15″…4.5″），**坐实为长度** |
| `+50` f64 | 恒 ≈ `3.05e-5` m | 同 `+34`，留 raw |
| `+58` f64 | 恒 ≈ `4.88e-4` m（2⁻¹¹） | 留 raw |
| `+66` / `+74` f64 | 18/18 为 0 | 留 raw |

`+42` 与 09-07 上午那条证据对得上：`Parametric Manifold` 缓存本体 35.59 mm（库默认 20.32），
而 D06 的两条 JDim 恰是 35.56 mm。

## 5. `+82` 之后：引用槽

```text
+82  u16 段数（短三形 1，长三形 2） ; +84 f64（0.0 十条 / 1.0 八条）
+92  段一 …（其后每段的起点随前一段长度走：+202 是双块记录的段二）
```

把「引用槽」定义成「u32 命中同存储某对象、且其后跟一个非零标记字」，语料给出的信号很干净：

| 偏移 | 命中 | 指向 | 标记字 |
|---|---:|---|---|
| `+92` | **18/18** | `0x0018 Line Object` 或 `0x005E Point Object` | `0x00CB` ↔ 线 20/20、`0x00F0` ↔ 点 4/4 |
| `+140` | 14/18 | `0x0058 JDimGroup` / `0x0085 Vertical Constraint` / `0x0114 JSheet` / `0x0089 FreeFormAttrSet` | `0x0067` / `0x008C`，**不跟类走** |
| `+202` | 6/18（正是那 6 条双块记录） | 同 `+92` | 同 `+92` |
| `+250` | 3/18 | 同 `+140` | 同 `+140` |
| `+280` | 2/18 | `0x0018 Line Object` | 与尾字重叠 |

类代码由 `tools/psm_type_clsid.py` 解出（radsrvitem 的 `type_code → CLSID` 表 + jutil 注册表）：

| 代码 | CLSID | 模块 | 名字 |
|---|---|---|---|
| `0x0058` | `8EC51800-…-46` | imagdex.dex | **JDimGroup Object** |
| `0x0085` | `FAFFE580-0259-11CE-AD7A-0800365FFA01` | imagdex.dex | Vertical Constraint |
| `0x0089` | `7D708DB0-5CB5-11CE-8EC7-080036EDF901` | jengine.dll | FreeFormAttrSet |
| `0x0114` | `3D4773E0-3782-11CE-956A-08003601DFE5` | docext.dex | JSheet Object |
| `0x0067`（标记字）| `AF0351B1-E21E-11CE-B6B9-08003601DCD0` | jengine.dll | Assoc subsystem element list |
| `0x008C`（标记字）| `C1430B8D-CE5D-11CE-9472-0800361C0602` | imagdex.dex | Groups Collection Object |
| `0x00CB`（标记字）| `A16AFDC0-AEC5-11CD-8BA4-08003624FC01` | — | **不在 RAD 注册表里** |
| `0x00F0`（标记字）| `57CDB650-CBE7-11CD-8BA4-08003624FC01` | — | **不在 RAD 注册表里** |

于是读法是：**`+92` 是这条尺寸量的那个几何 + 它的关联类**（标记字与几何类一一对应，
`Assoc subsystem element list` 这个名字也指向同一读法：这是关联而不是裸指针），
**`+140` 是这条尺寸归属的组 / 约束 / 属性集**。08-27 tag-188 那 42 条命中由此有了落点。

> **更正（2026-09-18，`2026-09-15-tag-188-members-land-in-jdim-reference-slots.md`）**：与空间图逐条 join 之后，
> `+92` / `+202` 的读法成立（40/40 落槽、槽预言类零反例）；**`+140` 不是引用**——四个存储里取值只有 48 / 16
> 两个常数、解到的类各不相同、A01 里解不到记录，上表 14/18 是小整数碰巧撞上 oid；**尾字是所属 `JDimGroup` 的
> oid**（13/13 解到组、组成员表回指 6/6）；`+280` 是主区末尾不带标记字的裸引用，其后的 u16 就是尾字。

每段里的三对 f64（段一为 `+108/+116`、`+124/+132`、`+146/+154`）在同一条记录里几乎相同，
是符号本地坐标；段尾的 `±1.0`（`+162`/`+170`）像轴向，工艺两条 288 的 `+272` 是 `π/2`。
**这一段都留 raw**：段长在语料里就有 102 / 70 / 82 三种，一条读法还没盖住。

## 6. imagdex 侧的 DoIO 没找到（否定结论）

08-31 给四个曲线族用的路子是：`ExportedVTableJ<Fam>2dPersistImp` 访问器 → vtable slot 3 →
`jengine_1076` 拿版本 → DoIO worker 用 `jengine_1075(stream, size, &member)` 逐字段读。
**这条路在 JDim 身上不通用**（`tools/idalib_imagdex_jdim.py`）：

- imagdex 确有类 `.?AVJDim@@` 与 `.?AVJDimGroup@@`（与 `psm_type_clsid` 把 `0x0115` / `0x0058`
  解到 `JDim Object` / `JDimGroup Object` 一致）——**身份坐实**。
- 但两个类各只到一张 vtable（8 槽 / 7 槽），**没有一槽**走到 `jengine_1075` / `jengine_1076`；
  模块里也没有 `IJPersistImp@JDim@@` 这种子对象（`JStyleBase` / `JGroupEngine` 等都有）。
- 它为它们准备的是 `tagDimPersistData` / `tagDimGrpPersistData` / `tagAnnotPersistData`，
  但那是**内存载荷结构不是读取器**：唯一的虚函数是删除析构，构造器（`sub_1032F87D`）清零
  七百多字节，远多于记录的 166…308。
- `jengine_1075` 有 2161 个调用点，但这份 i64 几乎没存函数边界，每个点都要先回溯 `55 8B EC`
  序言把函数造出来；在 JDim 代码区试过 6 个，落到的是 JGroupEngine 的持久化。

所以**字段读序仍缺一个权威来源**。帧由 radsrvitem 背书（§2），块内没有。

## 7. 坐实 / 留 raw

**坐实（可以据此写代码）**：记录帧 `34 + main_len + tail(0x100)`；`+30 main_len`；
`+14` 是尺寸种类（8 种）；`+34` 是块首、块长由种类决定（种类 1 = 48）；`+42` 是尺寸值（米）；
`+92` 指向被量的几何（Line / Point）且标记字标明是哪类；`+140` 指向组 / 约束 / 属性集。

**留 raw（不进 DTO）**：`+22`（原生自己不解释）；`+26` 除 `0x0100` 外的约十六个位；
`+20` 的重映射码；`+34` / `+50` / `+58` 三个疑似公差的 f64；`+82` 之后每段的完整文法
（三对点、轴向、`π/2`、段长 102 / 70 / 82 的差别）；两个标记字 `0x00CB` / `0x00F0` 的类名。

按 D3 的纪律，这一栏不写进 DTO——要么等 imagdex 的 DoIO，要么等一张能把八种尺寸种类都覆盖到的语料。

> **更正（2026-09-18）**：坐实栏里的「`+140` 指向组 / 约束 / 属性集」**撤下**（不是引用，见
> `2026-09-15-tag-188-members-land-in-jdim-reference-slots.md` §7）；尾字补进坐实栏：**所属 `JDimGroup` 的 oid**
> （语料互证等级，同文 §6）。

## 8. 还开着的

- **八种尺寸种类只练到一种。** 语料外的 `.pid` 若含链式 / 堆叠 / 角度尺寸（imagdex 里有
  `JDimChainLinear` / `JDimStackLinear` / `JDimStackAngular` / `JDimCoordinateGroup` 等类），
  块长会变成 52 / 64 / 80，本文的块内读法一条都不适用。
- `rad_class_name()`（`src/parsers/undecoded_census.rs`）的表还没补 §5 那几个名字，
  探针与丢弃告警里它们仍显示 `?`。
- `0x0010` 那 638 条与 `0x0115` 同 GUID 的子记录（Phase 33 的老账）本轮没碰。

## 9. 复现

```powershell
cd pid-parse
cargo run --example probe_jdim_bytes
python tools/idalib_radsrv_igdimension.py
python tools/idalib_imagdex_jdim.py
python tools/psm_type_clsid.py 0x0115 0x0058 0x0085 0x0089 0x0114 0x0067 0x008C 0x00CB 0x00F0
```
