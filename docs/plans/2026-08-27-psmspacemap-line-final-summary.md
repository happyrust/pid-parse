# `PSMspacemap` 这条线 — 阶段总结

> 日期：2026-08-27
> 范围：`pid-parse`，分支 `codex/phase32c-bundle-closeout`
> 区间：`ac49a92`..`cec05e0`（15 个 psm commit + 1 个 zip 归档），
> 24 个文件、+7296 行
> 起点：`/PSMspacemap` 是一批没人读过的流；终点：一族四个类的解码器进了库

## 1. 一句话

从「这堆 `PSMspacemap` 流是什么」开始，走到「**它是入边索引**」，再顺着一条找不到
记录的入边，挖出并解码了 `SmartPlant` 的**符号参数化链**——
`JSymbolInformation` → `Double Value Object` → `Variables Object` →
`Standard Relation` → `JDim`——四个类名全部坐实，四种记录字节全解，进解码器、进模型、
进棘轮。

## 2. 结果

| | 前 | 后 |
|---|---|---|
| 集成测试 | 114 | **121** |
| 单元测试 | 1055 | **1071** |
| 有解码器的 PSM 家族 | 8 typed + 2 audit | **+4**（本族，audit-only）|
| type code 表里有名字的码 | — | **+4**（`0x006F`/`0x00BD`/`0x00C7`/`0x00EA`）|
| 新 probe | — | 2 |
| 新分析文档 | — | 2（另订正 2 篇同日文档）|

五道门（build / test / clippy / fmt / missing-docs）全程绿。**解码行为的变化只有一处
——多了四个新家族的解码**；既有几何输出一个字节没动。

## 3. 走法

### 3.1 读表（`ac49a92`）

`Segment::Load` / `Segment::Save` 给出帧：流名是 `segment << 13` 算出来的，持久 id 就是
`(segment << 13) | index`。四图 38 个 member 流走到零剩余、条目数与头里的数相等。

### 3.2 两次自我推翻

这条线的价值有一半在这里。**每一次都是语料把假设否掉的**：

1. **「tag 是 type code」→ 否。**（`0e8e578`）`PSMroots` 说 `id 20 = TopVFSet`，而 20
   作为引用者恒带 tag 184；RAD 注册表说 `TopVFSet` 的码是 87/96。13 个 tag 当 type
   code 查，**13 个 GUID 一个都不在注册表里**。
2. **「成员是出边」→ 方向整个反了。**（`7a30700`）IDA 给出一条重建路径，读法是
   「tag 181 = 本仓的 `parent_ref`」。语料 **0/84、0/519**，槽位次序也对不上。改用
   表↔记录链 join 逐字节双向扫，结论倒过来：**value 是引用者，tag 是引用者的类，
   成员是入边**。
3. 外加两处小订正：`id 2` 不是 `/JSitesList` 流而是真对象（`859f6c3`）；
   `SymbolInformationCluster` 那条线索在本语料里是死的（`93f0606`）。

**教训**：反汇编读对了字节，不等于那条代码路径写过你手里的文件。`sub_56495440` 是
重建/压缩例程、无直接调用方——正常保存的文件当然不带它的签名（`6f8e099` 的 IDA 复核
把这一条讲透了）。

### 3.3 顺着一条断边挖到底

方向定了之后，剩下一个说不清的：519 个 tag-182 成员里 **191 个的 value 找不到任何
记录**。四轮把它挖穿：

- **`9491c6a`**：191/191 全部长在 `0x00C7` 条目上；健康文件里同一位置坐的是
  `PSMroots` 点名的 `SymbolInformation`；全语料 96 条根记录里它是**唯一会缺记录的
  名字**（16/41）。
- **`93f0606`**：`SymbolInformationCluster` 否证；缺的不是这个类，是它**长形那一层**。
- **`fa06d4d`**：长形字节解开，203 条 `0x00C7` **恰好二分**——12 条被活着的长形列着、
  191 条带无记录引用者，**重叠 0、遗漏 0**。
- **`c3e72da`**：四个类名一次拿到，`0x00BD` 与 `PSMroots` 的字符串**两条独立路对上**。

### 3.4 落库（`4bade08` / `d7eb003` / `cec05e0`）

四个 `PsmRecordDecoder`、四个模型 DTO、挂在 `JSite::symbol_information` 上、16 项单测、
八个入口进 panic 语料、跨图棘轮、probe。

## 4. 最终字节布局

**空间表条目**（`PSMspacemap/0x%.8x`，见 guide §3.1/§3.2）：

```text
u32 head（低 13 位 index，bit 17 必须置位）; u16 在用槽数 ; u16 槽容量
容量 × { u32 引用者持久 id ; u16 引用者的类 }      —— 入边
```

**符号信息 / 表达式族**（只在 `JSite<N>/PSMcluster0`）：

```text
0x00BD JSymbolInformation   44 字节头（+14 == 0x0010 才有下文）
                            u32 变量数 + 每变量 { 1 ; 1 ; f64 值 ;
                            u16 字符数 ; UTF-16 名 ; u32 值对象 oid }
0x00C7 Double Value         24 字节：oid ; parent ; 0 ; f64 值 ; 1 ; 15
0x00EA Variables            +13 u32 成员数 ; +17 起 每成员 { 0 ; u32 id } ; u32 321
0x006F Standard Relation    常量 GUID ; +38 JBExpression CLSID ; +58 值类型 CLSID ;
                            u32+ASCII 签名 `%>i%<i` ; 每个 `%` 一个
                            { u64 槽标记 ; u32 oid ; GUID } ; u32+UTF-16 公式
```

四种记录**全部按声明长度精确收尾**，`probe_symbol_information_family_shape` 另证
每条被接受的记录都正好落在链记录的起止上，零例外。

## 5. 关键洞察

**5.1 这张表是入边索引，而且不是全量引用图。** 1401 条记录的 `parent_ref` 非零却
一条都不进表。它只登记 13 种引用，既不能当对象清单，也不能当引用全图。

**5.2 tag 是引用者的类，一个对象只有一种 tag。** 2415 个引用者零分歧。182 至少盖三
个记录家族（`0x0067` / `0x00BD` / `0x006F`），所以「一个 tag 一个类名」是近似。

**5.3 id 空间按存储分。** 顶层和每个 `JSite` 各自编号，跨存储解析 id 会造出假命中。

**5.4 「活 id 没有记录」是常态。** 各存储发出去的 index 里几百到上千个没有记录
（`JSite793` 是 11120）。所以**「不在自由表」推不出「没被删过」**——这条一度是
「这些对象是不是被删了」的核心论据，实测它什么也证明不了。

**5.5 表达式子系统的对象和图元混在同一条记录链里。** `0x00C7`/`0x00EA` 来自
`exprdex.dll`。**按 type code 分家族时别默认「一条记录就是一个图元」。**

**5.6 一个符号的参数是活的。** `Left`/`Right`/`Bottom`/`Top` 不是常数，是变量，经
`JBExpression`（`0E$1+0.01` 这种）驱动尺寸对象。这是本条线对渲染侧最有价值的发现。

## 6. 落地清单

**代码**：`parsers::sheet_records` 四个解码器 + DTO + 八个封装；
`model::{JSiteSymbolInformation, Decoded*Record}`；`streams::jsite` 接线。

**棘轮**（`tests/parse_real_files.rs`，7 条新增 + 1 条改名）：

| 测试 | 锁住什么 |
|---|---|
| `psm_space_map_every_referrer_carries_one_tag` | 2415 引用者 / 675 多条目，零分歧 |
| `psm_space_map_members_are_incoming_edges` | 7 个 tag 的入边方向 + 那条悬空 249 |
| `psm_space_map_181_edges_match_igsymbol_jsite_ref` | 80/80 与解码器的 `jsite_ref` 对帐 |
| `psm_space_map_recordless_referrers_only_sit_on_0x00c7_entries` | 191 全在 `0x00C7` 上 |
| `psm_roots_symbol_information_is_the_only_root_without_a_record` | 96 条根、16 缺、家族恒定 |
| `symbol_information_long_form_lists_the_0x00c7_it_refers_to` | 二分 (203, 12, 191, 0) |
| `standard_relation_binds_a_double_value_to_a_dimension` | 13 条关系字节精确 + 操作数家族 |
| `symbol_information_family_decodes_across_fixtures` | 解码器数出 203/76/45/13 + 文档 surface |

**文档**：guide §3.1/§3.2/§3.3/§4/§7；分析文档
`2026-08-27-the-spacemap-is-an-incoming-reference-index.md`、
`2026-08-27-the-recordless-182-referrers-are-symbolinformation.md`；
另两篇同日文档加订正框。

## 7. 还悬着的

1. **写侧**：正常路的 append-member 方法不在 `radsrvitem` 里（`radsrvitem` 只把
   `(id, 成员块)` 原样搬进搬出），在跨 DLL 的 COM 虚调之后。
2. **(a) 删除 还是 (b) 没落盘**：`DWG-0202` 那 191 条倾向 (b)（两半各自独立落盘、
   两边都能缺；每条 `0x00C7` 只带**一个** phantom 而不是两个），但没判死。
3. **三处未认**：公式串开头的 `0E`；`0x006F` 的 `+12` 常量 GUID 和操作数槽的
   `0145EEC0-…`（都不在 `jutil.dll` 注册表里，多半是接口 IID）。
4. **几何含义**：`JSymbolInformation` 头上两个 `f64`（如 0.1016 / 0.17145）和每个变量
   那个 `f64` 还没和符号实际几何对上——`Left`/`Right` 共用一个值、`Bottom`/`Top` 共用
   另一个，但都不等于两个尺寸的一半。
5. **184 那 306 条**只活在表里的应用层登记边，没查。

## 8. 复现

```powershell
cd pid-parse
cargo run --example probe_psmspacemap_tag181_is_the_parent_ref
cargo run --example probe_symbol_information_family_shape
cargo test --test parse_real_files psm_space_map
cargo test --test parse_real_files symbol_information
cargo test --test parse_real_files standard_relation
python tools/psm_type_clsid.py 0xBD 0xC7 0xEA 0x6F
```
