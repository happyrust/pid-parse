# A01 的语义要按表示 UID 连：`GraphicOID` 是发布时的旧号，UID 经 `FreeFormAttrSet` 与标签 190 落到图元（2026-10-08）

> 承接 OpenCADStudio 计划 `docs/plans/2026-10-08-pid-integration-unblock-and-next-steps.md` D1（P-D28：A01 旁有 `_Data.xml` 却挂不上语义，先取证）与 `docs/plans/2026-10-08-pid-afternoon-review-uid-join-and-merge-handoff.md` D1′（P-D32 已批：开 D2，按表示 UID 连、`GraphicOID` 兜底）。
> 探针 `examples/probe_a01_representation_uid_is_the_join.rs`（只读，`src` 不动，不需要 `backup` 特性）；两跳规则见 `2026-08-07-graphic-oid-is-the-semantic-join.md`，S1 记下的「A01 带 `ItemTag` 的 3 条两跳全断」见 `2026-09-30-item-tags-without-data-xml.md`。

## 一句话

`A01_Data.xml` 发布的四个 `GraphicOID`（24601 / 24606 / 24613 / 24615）在现在的 `A01.pid` 里**都不是图元记录**，而是顶层 `/Unclustered Dynamic Attributes` 里的 `0x0089 FreeFormAttrSet`，所以 `PidSemanticIndex::resolve` 一个也连不上（0 / 4）。
稳定的是**表示 UID**：每个发布表示的 UID 以 ASCII 恰好出现在一个顶层 `FreeFormAttrSet` 里，该集在空间映射里以**标签 190** 挂在它描述的记录下。
按这条连，A01 找回 3 / 4 个对象且全对（容器 → 卧罐符号 184、管口 → 法兰管口符号 51、管线 → 管段折线 275）；对照图 0202 的 39 个对象全部落在 `GraphicOID` 自己的记录上，与今天逐条相同。
**可修，修在 `PidSemanticIndex` 的第一跳**（D2）。

## 问的是什么、怎么问

对每个发布的 `GraphicOID`，探针在文件里按五种已有读法找它（模块注释 1–5）：
① 自己的记录（任一存储里 `payload+0` = 该 oid 的链记录）；② 以它为 id 的空间映射项（谁引用它、什么标签）；③ 把它列为成员的空间映射项（它引用谁）；④ 其它链记录里以 4 字节字出现的位置；⑤ 链外的流。
再看消费者关心的：今天、加「朴素一跳」、加「UID 连接」三种规则下，`build_normalized_geometry` 投出的图元各连到哪个发布对象（⑥ / ⑨），并把落点记录与每个发布 UID 在文件里的位置摆出来（⑦ / ⑧）。0202 publish 副本同跑作对照。

## A01：四个号各在哪

| `GraphicOID` | 发布对象 | ① 自己的记录 | ③ 以标签 190 把它列为成员的项 |
|---|---|---|---|
| 24601 | `PIDProcessVessel` `V 010121A` | `/Unclustered Dynamic Attributes` 的 `0x0089 FreeFormAttrSet` | 51 = `/Sheet6` `0x00CE JSymbol`（法兰管口） |
| 24606 | `PIDNozzle` | 同上 | 649 = `/Sheet6` DependencyObject（容器符号 184 与其位号文字 646 的标注关系） |
| 24613 | `PIDPipeline`（表示 `AC9D…`） | 同上；另被 DependencyObject 1094 在 `payload+22` 引用（空间映射标签 249） | 417 = `/Sheet6` DependencyObject（kind 4，含管段折线 275） |
| 24615 | `PIDPipeline`（表示 `4B38…`） | 同上 | 47 = `/Sheet6` DependencyObject |

- ② ④：只有 24613 有引用者（上表）；⑤：四个号在 `/PSMspacemap/0x00000000` 里各出现一次（08-07 记录 §4 看到的就是这一处与属性流里那一处）。
- 「朴素一跳」= 把 `_Data.xml` 的号改写成 ③ 里那条项的 id，再走今天的两跳：5 个图元连到 4 / 4，但 24601（容器）落到法兰管口 51、24606（管口）落到容器位号文字 646——**张冠李戴**，不能用。

## UID 在文件里的位置（⑧）

| 发布 UID | 在 `.pid` 里 |
|---|---|
| 容器的表示 `CA8A…` | 属性集 24593（ASCII，`payload+157`）；同集还有 `PIDProcessVessel` 自己的 UID `C574…` |
| 管口的表示 `C33E…` | 属性集 24601（`payload+157`）；同集有 `PIDNozzle` 的 UID `7465…` |
| 管线的表示 `AC9D…` / `4B38…` | 属性集 24612 / 24613（`payload+157` / `+160`）；两集都有 `PIDPipeline` 的 UID `185E…` |
| 图纸 `PIDDrawing` 的 UID | 十个属性集，另在 `/DocumentSummaryInformation`（UTF-16） |
| 位号 `V 010121A` / `PH- 0102102-DN250 mm-B5-P-40.000 in` | 只在文字记录 646 / 1120 里（UTF-16） |
| `PIDPipingConnector`、两个端口、工艺点、15 条 `Rel` | 文件里没有 |

每个**表示** UID 恰好落在一个顶层属性集里；该集在空间映射里以标签 190 挂在一条 `/Sheet6` 记录下——这就是连接。

## 三种规则的结果

| 规则 | A01 | 0202（对照） |
|---|---|---|
| 今天（`GraphicOID` 直连 + DependencyObject 第二跳） | 0 个图元 / 0 of 4 | 41 个图元 / 39 of 39 |
| 朴素一跳 | 5 / 4 of 4，两处连错 | 41 / 39 of 39 |
| **表示 UID → 属性集 → 标签 190 记录**，再走今天的第二跳 | **3 / 3 of 4，全对** | 41 / 39 of 39；39 个对象的 UID 落点 = 自己的 `GraphicOID` |

A01 按 UID 连上的三个：

- 24601 容器 → 集 24593 → **记录 184**（`Horizontal Drums\Horizontal Drum.sym` 的放置）；
- 24606 管口 → 集 24601 → **记录 51**（`Nozzles\Flanged Nozzle.sym` 的放置）；
- 24615 管线 → 集 24613 → 记录 417（DependencyObject）→ 第二跳 → **管段折线 275**。

第四个 24613（同一条管线的另一个表示 `AC9D…`）→ 集 24612 → 记录 420（DependencyObject kind 1），没有图元经它连上；管线已由 24615 连上，不算损失。

## 为什么号对不上

探针的解释：图在发布之后又存过，发布时写下的 `GraphicOID` 在现文件里换了主人——24601 成了**管口**表示的属性集，24606 成了一个挂在容器位号标注关系上的集。号还在，意思变了。
0202 发布之后没再存，号与 UID 两条路落在同一处。`A01_Meta.xml` 的时间戳没有核（不影响结论）。

## 线索（未核）

A01 两条位号文字各有一条 kind 2 的 DependencyObject：649（`sub` 184 = 容器符号，`+22` = 属性集 24593，`+34` = 文字 646 `V 010121A`）与 1094（`sub` 275 = 管段，`+22` = 属性集 24613，`+34` = 文字 1120）。
即这两条「标注」关系同时记着被标的图元与它的属性集，位号文字也许能顺着挂上所属物项。kind 2 的 `+22` 不总是属性集（DependencyObject 47 的 `+22` 是符号 51），也没在 0202 上核同构；OCS 计划默认 D2 不带这一条，要带先补这一核。

## 结论与下一步

- 根因：`GraphicOID` 是发布时的号，图发布后再存就可能失效；表示 UID 不变。
- 修法（D2，OCS 计划 P-D32 已批）：`PidSemanticIndex` 多建「表示 UID → 记录 oid」表（顶层 `FreeFormAttrSet` 里以 ASCII 出现的 UID → 该集的标签 190 落点；一个 UID 落多个集、或一个集落多条记录时不取、计数），第一跳先按 UID、再按 `GraphicOID`，第二跳不变；
  `GraphicOID` 与 UID 指向不一致的条数交出作诊断（A01 4 条）。预期 A01 0 / 4 → 3 / 4（184 / 51 / 275），0202 39 / 39 逐条不变；四张主图旁没有 `_Data.xml`，不受影响。
- 探针不进 `src`；跑法 `cargo run --example probe_a01_representation_uid_is_the_join`（默认或 `--no-default-features` 都行），几秒。

## 验证

- 2026-10-08 两次运行（入库前后各一次）输出逐字节相同，exit 0；本页数字取自这份输出。
- `cargo clippy --example probe_a01_representation_uid_is_the_join -- -D warnings` 默认特性与 `--no-default-features` 都过（入库前只把一处 `type_complexity` 拆成 `type Member`）；`rustfmt --check` 干净。
