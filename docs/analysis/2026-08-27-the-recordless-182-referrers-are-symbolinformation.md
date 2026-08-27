# 那 191 个「无记录」的 182 引用者是 `SymbolInformation`

> 日期：2026-08-27
> 范围：`pid-parse`
> 结论类型：**corpus（四图 11 个存储的 space map × 记录链 × `PSMroots` 三方 join，无抽样）**
> 前置：`2026-08-27-the-spacemap-is-an-incoming-reference-index.md` §5.1
> （本文回答它留下的问题；那节的否证——「不是 `id 2` 那种流对象」——继续成立）
> 棘轮：`tests/parse_real_files.rs::psm_space_map_recordless_referrers_only_sit_on_0x00c7_entries`、
> `tests/parse_real_files.rs::psm_roots_symbol_information_is_the_only_root_without_a_record`

## 0. 上一轮留下的问题

入边表定向之后剩了一处说不清：519 个 tag-182 成员里有 191 个的 `value`
**在自己存储的记录链里找不到任何记录**。上一轮否掉了「它们是 `/JSitesList`
那种流对象」的猜测，只留下一句描述——188 个扁在 `DWG-0202` 一张图、全是活
id、都没有自己的条目、只带 182 这一个 tag——和一条方向指示：**从这些成员所在
条目的记录家族反查**。

本文按那条指示走完了。答案：**它们是 `SymbolInformation` 对象**——不是新的一
类东西，是符号信息层没落盘，而空间表把指向它的入边留了下来。

先说自证：本轮用一套独立写的字节走法重新数了一遍，得到「`DWG-0202` 188 个 +
`D06` 3 个 tag-182 无记录，`DWG-0201` 只有那 1 条已知的 tag-249 悬空边，工艺图
0 个」——和已提交的测量一个数不差，所以下面的推理和之前的棘轮站在同一套读数上。

## 1. 它们全部长在 `0x00C7` 上

**191/191** 的宿主条目都是 `0x00C7` 记录（`DWG-0202` 的 188/188、`D06` 的
3/3），零例外；而且这类成员**只出现在 `JSite` 存储里，顶层一个没有**。

顺着 `0x00C7` 往上，是一整套结构，四张图形状一致：

| 事实 | 计数 |
|---|---|
| 全语料 `0x00C7` 记录（188 / 7 / 4 / 4），全为 24 字节 | 203 |
| 其中有 space map 条目的 | **203 / 203** |
| 其中 `parent_ref`（payload+4）指向一条 `0x00EA` 记录的 | **203 / 203** |
| `0x00C7` 条目上的成员，tag 全部是 182 | 215 个成员，1 种 tag |
| 这 215 个里引用者有记录的 | 24（12 条 `0x00BD` + 12 条 `0x006F`）|
| 引用者无记录的 | **191** |

`0x00EA`（全语料 76 条）就是这一组的容器，字段直读得出：

```text
+0   u32  oid
+4   u32  parent_ref（这一层为 0）
+13  u32  子数
+21  子数 × u64  子对象持久 id（高 4 字节恒为 0）
     尾部 u32 = 321，四图恒定
```

自校验：`DWG-0202` 的 71 组按大小 1/2/3/4 分别是 16/17/14/24 组，于是子槽
`+21/+29/+37/+45` 上应命中 71/55/38/24 次——实测正是 71/55/38/24。**容器的成员
表和空间表的 `parent_ref` 链互相印证**，两边说的是同一组。

## 2. 健康文件里，那个位置坐的就是 `SymbolInformation`

另外三张图的每个 `0x00C7` 条目带**两个** 182 成员，两个引用者都有记录：一条
`0x00BD` 加一条 `0x006F`。而那条 `0x00BD` 引用者，**正是 `PSMroots` 里写着
`SymbolInformation` 的那个 id**：

| 图 | 存储 | 根表里的 `SymbolInformation` id | 它作为引用者的 tag |
|---|---|---|---|
| `D06` | `JSite145` | 22 | 182 |
| `DWG-0201` | `JSite329` | 77、513 | 182 |
| 工艺图 | `JSite7559` | 73 | 182 |

`DWG-0202` 里同一个位置换成了无记录 id，而 `PSMroots` **仍然把其中 5 个
（627 / 1649 / 1760 / 1796 / 1916）叫 `SymbolInformation`**——有名字、有入边、
没记录。这 5 个是实名证据；其余 66 个靠位置同构（同为某个 `0x00EA` 组唯一的
182 入边，id 混在同一段分配区里）。

把根表整个横过来看，这个身份更硬——全语料 96 条 `PSMroots` 记录：

| 根名 | 有记录 | 无记录 | 有记录时的家族 |
|---|---|---|---|
| `DocStore` | 11 | 0 | `0x0075` |
| `Dynamic Attributes Set Table` | 11 | 0 | `0x006D` |
| `StyleLibrarian` | 11 | 0 | `0x005A` |
| `TopVFSet` | 11 | 0 | `0x0060` |
| `Server Document` | 4 | 0 | `0x004A` |
| `_SupportOnlyList` | 4 | 0 | `0x0067` |
| `Imagineer Document` | 3 | 0 | `0x004A` |
| **`SymbolInformation`** | **25** | **16** | **`0x00BD`（25/25）** |

**`SymbolInformation` 是唯一一个会缺记录的根名**，而且有记录时家族恒为
`0x00BD`。顺带一条早该记下的事：`PSMroots` 的 `id` 字段不是「不透明类型
标记」（旧 rustdoc 的说法），**它是本存储 id 空间里的持久 id**——96 条里 80 条
直接解析到同存储的记录，剩下 16 条正是这批 `SymbolInformation`。

## 3. 为什么偏偏是 `DWG-0202`

`JSite793` 是**唯一一个有 `0x00C7` 却一条 `0x006F` 都没有**的存储：另外三个站点
是 7↔5、4↔4、4↔4，它是 188↔0。`0x00BD` 也不成比例——188 条 `0x00C7` 只配 11 条
`0x00BD`（`DWG-0201` 是 4 条配 17 条）。它的 id 分配同样见了底：

- `PSMsegmenttable`（`'stab' + u32 槽数 + 每段一字节在用标志`）：它是 6 槽用
  4 段，别家 4 槽用 3 段——标志位和实际存在的 `PSMspacemap` 段流一一对上；
- `PSMclustertable` 里每个 cluster 记着**它跨哪几段**（`u32 段数 + 段数×u32
  段号`）：它的 `PSMcluster0` 是 `[0, 3]`，别家都是 `[0]`；
- 段 0 的 `m_iNext` 顶在 **8192 上限**、自由表只剩 79——这一段发满了才滚到段 3。

**一处口径订正**：上一轮说这些 value「全是活 id（`< m_iNext` 且不在自由表）」。
在这个存储里这句话近乎空话——段 0 发出去 8192 个 index，只有 495 个有记录、79
个在自由表。那里的「活」不构成证据，本文的身份判定不依赖它。

## 4. 记录去哪了（线索，未证）

仓库里早有一条对得上的 IDA 旧证：`OLECRT.dll::sub_100017C0` 在**外部嵌入 OLE
存储**里打开一个 **`SymbolInformationCluster`**
（`2026-06-13-phase31-olecrt-storage-entrypoints.md`，已收进
`2026-06-19-ida-evidence-baseline.md` 的历史证据表）。也就是说符号信息本来就有
第二个家，不一定落在本存储的 `PSMcluster0` 里。这解释得通「id 活着、名字在根表
里、入边在空间表里，唯独记录不在这个存储的 cluster 里」，但本轮没有验证，只作
线索记下。

## 5. 对既有结论的影响

- 入边方向、13 个 tag 的家族表、所有既有计数**一个没变**。
- §3.2 那句「184 这类 payload 里找不到的边是只活在表里的应用层登记边」要分开
  看：184 的那 306 条是**没有 payload 对应**（引用者记录在、只是不写这条引用），
  本文这 191 条是**引用者记录根本不在**，两回事。
- tag 182 至少盖三个记录家族：`0x0067`（`_SupportOnlyList`，顶层）、`0x00BD`
  （`SymbolInformation`，站点内）、`0x006F`。所以 §3.3 那张表里的「182 =
  `_SupportOnlyList`」只是这个类的一个代表名，不是全部。

## 6. 已落地与下一步

**已落地**：本文 + 两条棘轮
（`psm_space_map_recordless_referrers_only_sit_on_0x00c7_entries`、
`psm_roots_symbol_information_is_the_only_root_without_a_record`）+ guide §3.2/§3.3
的相应段落 + `PsmRootEntry::id` 的 rustdoc 订正。**解码行为零变化。**

**下一步**：

- 把 `0x00BD` / `0x006F` / `0x00C7` / `0x00EA` 这一族解出来命名。`0x00BD` 带
  UTF-16 名字（语料里见到 `Left`），`0x00C7` 是 24 字节叶子（一个 f64 + `1` +
  `15`），形状上像「符号 + 连接点」，但没有独立证据，本文不给名字。
- 验 §4 那条线索：`SymbolInformationCluster` 是否就是这些记录的去处——要么在
  `JSite<N>/\001Ole` 的嵌入存储里找，要么回 IDA 看 `sub_100017C0` 的读写两侧。
- `PSMsegmenttable` 的标志字节现在有了语义（段是否在用，4/4 存储一致），
  `PSMclustertable` 的 `段数 + 段号表` 同理；两者目前都还标着 `Probed`，要升
  `Decoded` 得单独一轮（会动 byte-audit 快照）。

## 7. 复现

```powershell
cd pid-parse
cargo test --test parse_real_files psm_space_map_recordless_referrers_only_sit_on_0x00c7_entries
cargo test --test parse_real_files psm_roots_symbol_information_is_the_only_root_without_a_record
```
