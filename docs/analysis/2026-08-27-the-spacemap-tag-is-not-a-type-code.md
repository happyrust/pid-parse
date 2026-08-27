# `PSMspacemap` 的 tag 不是 type code：两个类名从 `PSMroots` 认出来了

> 日期：2026-08-27
> 范围：`pid-parse`
> 结论类型：**native-reader（否证）+ corpus（两个类名，四图一致）**
> 前置：`2026-08-27-psmspacemap-is-the-object-reference-graph.md`
> 含**一处推翻**：`205`/`249` 跟 `JSymbol`(206)/`Dependency Object`(250) 差一个数，
> 之前只当巧合搁着；现在有硬证据说整条路都不通，不用再回头看它。
> probe：`examples/probe_psmspacemap_what_the_tag_names.rs`、`tools/psm_type_clsid.py`、
> `tools/clsid_registry.py`

## 0. 上一轮留下的那个问题

`tag` 是被指对象的类，这条坐实了。但**哪个 tag 是哪个类**没有。最自然的猜测是
「tag 就是 PSM type code」——毕竟 §4 那张表干的正是「码 → 类」这件事。

这一轮先把这条猜测**否掉**，再从另一个方向认出两个真名。

## 1. tag 不是 type code

### 1.1 一个对象就够了

`PSMroots` 是文档里唯一一张 `id → 名字` 的表。四张图里它都写着同一条：

```text
id 20  TopVFSet
```

而这个 `20`，在顶层 space map 里被指到时**恒定带 tag 184**（四图全中）。

于是「TopVFSet 是什么类」有了两个互相独立的答案：

| 从哪来 | 答案 |
|---|---|
| RAD 类注册表按名字查 | `Top ViewFilterSet`，`viewfil.dex`，CLSID `88E2AA20-…`（同名第二个 GUID `9A6C2C50-…`）|
| 那两个 CLSID 在 type code 表里的槽位 | **87** 和 **96** |
| space map 说的 | tag **184** |

**87 / 96 ≠ 184。** 一个对象、名字由厂商自己给、类由厂商自己的注册表给，
type code 和 tag 对不上。这条就够了。

（同一套工具查 `JSL Style Librarian` 回来是 **90** = `0x005A`，正是 §4 表里已经写着的
那个码——所以不是查表方法坏了。）

### 1.2 十三个也一样，而且不像是运气

把 13 个 tag 全都当 type code 送进 `radsrvitem.dll` 的 type_code → CLSID 表，
每个都落在一个非空槽上，GUID 也都带 Intergraph 的 `080036xx` 节点，看着像真的。
但拿去问 RAD 类注册表：

- **13 个，一个都没有名字。**
- 对照组：`0x0013`/`0x0030`/`0x003D`/`0x004D`/`0x00CE`/`0x00FA` 六个本仓已解码的码，
  **六个全有名字**。
- 密度：码 0..400 里 **230 个有名字**、170 个没有。单看 tag 所在的 181..261 这段，
  81 个码里 **39 个有名字**。13 个全部落进没名字的那 42 个，约 1/4000。

而且「注册表里没有」不是单一来源的否定——这张 96 字节的注册表在 `jutil.dll`、
`i2mnuctl.ocx`、`igrresource412.dll`、`jcntrls412.ocx` 里各有一份，四份都搜过。

**所以 tag 是另一套类编号，不是 type code。** 别再拿 §4 的表去套它。

## 2. 认出来的两个

`PSMroots` 同时给了名字和 id，而 id 在顶层 space map 里带着 tag。四张图交叉之后：

| tag | `PSMroots` 里的名字 | 各图的 id | 一致性 |
|---|---|---|---|
| **184** | `TopVFSet`（类名 `Top ViewFilterSet`，`viewfil.dex`）| 20 / 20 / 20 / 20 | 4/4 |
| **182** | `_SupportOnlyList` | 25 / 25 / 25 / **26** | 4/4 |

`_SupportOnlyList` 那一行的 id **不是四图都相同**（工艺图是 26），这让一致性有了分量：
不是同一个号碰巧带同一个 tag，是同一个**名字**带同一个 tag。

## 3. 另外两个只有结构、还没有名字

顶层 map 里，每个 `JSite` 的条目形状都是 `[(2, tag 182), (X, tag ?)]`，而
`PSMroots` 说这些 `JSite` 叫什么：

```text
D06.pid          JSite145 = Server Document      -> (146,  tag 261)
D06.pid          JSite151 = Imagineer Document   -> (138,  tag 225)
DWG-0201GP06-01  JSite329 = Server Document      -> (397,  tag 261)
DWG-0201GP06-01  JSite396 = Imagineer Document   -> (224,  tag 225)
DWG-0202GP06-01  JSite793 = Server Document      -> (794,  tag 261)
工艺管道及仪表流程-1  JSite7559 = Server Document  -> (7560, tag 261)
工艺管道及仪表流程-1  JSite6963 = Imagineer Document -> (6962, tag 225)
```

`Server Document` 一律配 tag 261，`Imagineer Document` 一律配 tag 225，零例外。
全语料里 tag 261 恰好出现 **4 次**（四个 Server Document 各一），tag 225 恰好
**3 次**——而 `DWG-0202GP06-01.pid` **没有 `Imagineer Document`**，也正是唯一
没有 tag 225 的那张图。

所以 261 与 225 各自是「Server / Imagineer 文档持有的那一个对象」的类。名字还没有，
但它们不再是两个自由变量。

> 顺带把 0202 缺 `188`/`239` 的事说清楚：它们**不是**跟 `Imagineer Document` 绑的。
> 两个 tag 都只在 `Server Document` 存储里出现，而 0202 是有 Server Document 的——
> 那张图只是没有那两类对象而已。别把 §2 的解释套过去。

## 4. 为什么另外五个根没有 tag

`PSMroots` 一共七条，上面用掉两条。剩下五条不带 tag，各有各的理由，而且都说得通：

- `StyleLibrarian` id **8192** = `(1 << 13) | 0`，`Dynamic Attributes Set Table`
  id **16384** = `(2 << 13) | 0`。它们在第 1、2 段，而本语料只有第 0 段和第 3 段有
  member 流——**根 id 和 space map 的持久 id 是同一套编号**，这两条顺带把这点又证了一遍。
- `Server Document` / `Imagineer Document` 有自己的条目，但**没有任何引用指向它们**
  （上一篇 §4.3 已测）。根就是根。
- `DocStore` id 1，没人指。

## 5. 哪个存储用哪些 tag

按 `PSMroots` 给存储起的名字分组，四图 11 个 space map 存储合并统计（数的是**被指到
的对象个数**）：

| tag | `<顶层>` | `Server Document` | `Imagineer Document` |
|---|---|---|---|
| 181 | 84 | — | — |
| 182 | 65 | 101 | 9 |
| 183 | 4 | 45 | 7 |
| 184 | 8 | 45 | 7 |
| 185 | — | 169 | — |
| 188 | — | 14 | — |
| 190 | 1199 | 247 | — |
| 201 | 15 | 9 | — |
| 205 | 1 | 3 | 4 |
| 225 | 3 | — | — |
| 239 | — | 5 | — |
| 249 | 367 | — | — |
| 261 | 4 | — | — |

`182`/`183`/`184`/`205` 三种存储都用，是真正的文档级通用类；`181`/`249`/`261`/`225`
只在顶层；`185`/`188`/`239` 只在 `Server Document` 里。

## 6. 读器那条路走过了：`radsrvitem.dll` 根本不看 tag

原以为顺着读器能问出 tag 是哪套编号。走完的结果是**否定的，但它把搜索范围切掉了一大块**。

- `sub_5647A900`（本语料走的那个读器）读完头之后，把成员区当**一整块**读进对象
  （`sub_5647AFB0(stream, dest, raw_count * 6)`），**一个字节都不解释**。
- 全 DLL 搜 13 个 tag 值的立即数比较（`cmp`/`sub` 对 `0B5h`…`105h`），**零命中**。
  `radsrvitem.dll` 从来不测试一个 tag 的值。

所以 tag 对持久层是**透传数据**：存储管理器只负责把 `(id, tag)` 原样搬进搬出，
解释它的是上面的消费者，在别的模块里。**下一轮要找的不是 `radsrvitem.dll`。**

顺带把上一篇对 `sub_5647AB70` 的说法说准。它不只是「写死了两个 tag」——紧凑形式下
它先读 8 字节，拆成两个 `u32`，然后合成：

```text
[esi+22h] = 第一个 u32 ;  [esi+26h] = 0xB5 (181)
[esi+28h] = 第二个 u32 ;  [esi+2Ch] = 0xB6 (182)
```

也就是说**紧凑形式把两个引用直接放在条目头里，tag 由读器补**。181 和 182 因此是
「这种形式下每个对象都带的那两个槽」。本语料没有一条走紧凑形式，所以这只解释了
为什么这两个值是编译期常量，没有给出它们的名字——名字要靠 §2 那条路。

顺带确认两件本仓已有的读法：`[esi+4] = (segment << 13) + index`，持久 id 的构造在
两个读器里一字不差；紧凑形式的头是 `u32 ? ; u16 index ; u32 flags`，10 字节。

## 7. 下一步

- **不要**再拿 §4 的 type code 表去套 tag（§1 已否），也不要从「差一个数」这类数值
  巧合出发（`205`/`249` 那两处就是这么来的）。
- **不要**再在 `radsrvitem.dll` 里找（§6）。
- 还没走的：消费者在哪个模块。`imagdex.dex` / `symbol.dex` / `viewfil.dex` 是候选——
  `TopVFSet` 的类就出自 `viewfil.dex`，而本仓只有 `style.dll` / `j2dsrv.dll` /
  `sppid.dll` / `ugeom2d1.dll` / `radsrvitem.dll` 有 IDA 库，这几个 `.dex` 都还没拆过。
- `PSMroots` 这条名字通道只名了 7 个对象，已经榨干。

## 7. 复现

```powershell
cd pid-parse
cargo run --example probe_psmspacemap_what_the_tag_names
python tools/psm_type_clsid.py 181 182 183 184 185 188 190 201 205 225 239 249 261
python tools/clsid_registry.py --grep "ViewFilter"
```
