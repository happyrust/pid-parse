# 没有 `_Data.xml` 的位号：文件内三条关联复原 0 / 16，S 登记不做（2026-09-30）

> 承接 OpenCADStudio 计划 `docs/plans/2026-09-29-pid-import-next-round.md` S1（任务 7；09-28 单 P-D7 已批：只取证，位号总复原率 ≥ 80 % 才提议实施单）；口径照其 spec design D6。
> 问题：没有 `_Data.xml` 时（字节路 / 网页版就没有），只凭 `.pid` 里的关联能复原多少位号与类？
> 探针 `examples/probe_item_tags_without_data_xml.rs`（只读，`src` 不动，不需要 `backup` 特性）；答案键的两跳见 `2026-08-07-graphic-oid-is-the-semantic-join.md`。

## 一句话

今天 `label=` / `class` 只来自 publish 副本旁的 `_Data.xml`（`GraphicOID` 两跳）。只凭文件内的三条关联——`igTextBox.parent_ref`、`DependencyObject` 编组、放置的符号库路径 → 类——
0202 上 16 个带标签的物项**一个位号也复原不出（0 / 16 = 0.0 %）**，类对 2 / 16。文字其实画在图上（4 / 16 原样是某个文本框的全文），只是这些关联够不着它。低于 80 %，**S 登记不做**。

## 口径（打分前写死在探针的模块注释里）

- **答案键**：默认（全量）解析 publish 副本，`PidSemanticIndex::load_beside` 读 `_Data.xml`，六个图形族的每条记录过 `PidSemanticIndex::resolve`（两跳），得 `(GraphicOID, label, class)`。
  label 取 `PidSemanticObject::label()`——`ItemTag`，没有就 `Name`，即 OCS 显示的 `label=`。0202 的 `_Data.xml` 一个 `ItemTag` 属性都没有（DWG 口味的导出只写 `Name`），
  A01 带 `ItemTag` 的 3 条全部两跳断，严格只认 `ItemTag` 分母是 0、无从打分；严格数并列打印。
- **分母**：label 非空、两跳落到至少一条文件内图形记录的物项；落不到的记「两跳断」，不进分母。锚点 = 落点记录 + `GraphicOID` 本身指的 `DependencyObject`。
- **路 a**：`parent_ref` 是锚点之一的文本框为候选。**路 b**：组展开后含文本框、且组本身或其非文本成员是锚点，组内文本为候选。两路都是恰一个不同文本才算预测，多个记「多候选」。
- **路 b 的放宽**（打分前定）：成员 = 尾部引用（`+18` 起 4 字节对齐、指同 sheet 解码池的 oid，同 `PidSemanticIndex` 的读法）∪ `parent_ref` 指该组的记录 ∪ 子组的成员（一层）。
  理由：打分前的结构普查里，只认尾部引用时 84 组中含文本框的 24 组只有 2 组同时含线、没有一组含符号；而文本框的 `parent_ref` 56 / 56 指 `DependencyObject`，10 组的成员里还有组。
  这两种也是文件内的编组关系，一并算进，0 分就不能归咎于读法太窄。放宽是超集：放宽后候选全空，D6 字面的读法（「同一组里有文本框与图形」）同样是 0 / 16。放宽后 29 组含文本。
- **路 c**：物项的 `igSymbol2d` 放置（直接落点优先，否则经依赖那一跳）→ `jsite_ref` → `JSite<id>` 的 `symbol_path`（否则 `local_symbol_path`）→ 下表首条命中；都不中不给类。
  规则按库目录名与答案键的类词表写出，没看分数，事后不改。

| # | 条件（`Symbols` 以下的路径段，不分大小写） | → class |
|---|---|---|
| 1 / 2 | 某级目录 `nozzles` / `vessels` | PIDNozzle / PIDProcessVessel |
| 3 | 首级目录 `equipment` | PIDEquipment |
| 4 / 5 | 某级目录 `system functions` / 首级目录 `instrumentation` | PIDControlSystemFunction / PIDInstrument |
| 6 | 某级目录 `piping opc's` | PIDOPC |
| 7–9 | 某级目录 `valves` / `fittings` / `piping components` | PIDPipingComponent |
| 10 | 文件名含 `note` | PIDNote |
| 11 / 12 | 某级目录 `labels` / `annotation` | （不给类） |
| 13 | 文件名为 `drawing description` | PIDDrawing |

- **组合与归一**：label = 路 a，否则路 b；class = 路 c。去首尾空白、内部连续空白压成一个、大小写敏感。门槛看合计的位号总复原率。

## 语料与答案键

| 图（publish 副本） | 发布物项 | 带 label | 其中 `ItemTag` | 落进文件 = 分母 | 两跳断 |
|---|---:|---:|---:|---:|---:|
| DWG-0202GP06-01 | 39 | 16 | 0 | 16 | 0 |
| A01 | 4 | 3 | 3 | 0 | 3 |
| 合计 | 43 | 19 | 3 | **16** | 3 |

- 0202：10 条 PIDPipingConnector 经依赖落在 `igLineString2d` 上；4 条 PIDBranchPoint、2 条 PIDControlSystemFunction 直接落在 `igSymbol2d` 上。
- A01：3 条（`V 010121A`、两条 `PH- 0102102-…`）落不到任何文件内图形记录（同 08-07 §4 的覆盖缺口），只报不计。合计因此就是 0202。

## 结果

分图：A01 分母 0，各路 n/a；0202 各数即合计，列在下面。

| 路 | 覆盖 | 准确 | 看到的 |
|---|---:|---:|---|
| a `igTextBox.parent_ref` | 0 / 16 | — | 0202 `/Sheet6` 56 个文本框的 `parent_ref` 全指 `DependencyObject`，没有一个是物项的锚点 |
| b `DependencyObject`（放宽后） | 0 / 16 | — | 0202 84 组、29 组含文本，没有一组连到物项的锚点 |
| c 符号路径 → 类 | 6 / 16 | 2 / 6 | 2 个 PIDControlSystemFunction（`Instrumentation\System Functions\D C S\…`）对；4 个 PIDBranchPoint 判成 PIDOPC；10 条 run 没有路径 |
| **位号总复原率**（a，否则 b） | **0 / 16 = 0.0 %** | | P-D7 门槛 80 % |
| 类复原率（c，只报） | 2 / 16 = 12.5 % | | |

| 答案键 class | 分母 | 落点 | a 覆盖 | b 覆盖 | c 覆盖 | c 对 | 位号对 | 类对 |
|---|---:|---|---:|---:|---:|---:|---:|---:|
| PIDPipingConnector | 10 | 经依赖 `igLineString2d` | 0 | 0 | 0 | 0 | 0 | 0 |
| PIDBranchPoint | 4 | 直接 `igSymbol2d` | 0 | 0 | 4 | 0 | 0 | 0 |
| PIDControlSystemFunction | 2 | 直接 `igSymbol2d` | 0 | 0 | 2 | 2 | 0 | 2 |

## 失败类别

| 类别 | 计数 | 一例 |
|---|---:|---|
| label：无框指向 | 16 | g=103 PIDPipingConnector `A3jqz0401-OD-50 mm-1.6AR12-WE-50mm`：a、b 都没有候选 |
| label：位号拆成多框 / 多候选 / 文字不是位号 / 别的错字 | 0 | — |
| class：没有符号路径 | 10 | 同上 g=103：run 落在 `igLineString2d` 上，没有放置 |
| class：类错 | 4 | g=1257 PIDBranchPoint `1`：放的是 `Piping\Piping OPC's\Off-Drawing.sym` → PIDOPC |
| class：映射缺 / 多类 | 0 | — |
| 两跳断（不进分母） | 3 | A01 g=24601 PIDProcessVessel `V 010121A` |

## 诊断（不计分）

- 答案在 a ∪ b 的候选里：0 / 16。候选集本来就是空的，换更好的挑法也救不回来。
- 答案画在文件里（不看关联）：4 / 16 原样是某个文本框的全文，2 / 16 只以片段出现（g=612 `LIA-060201` ← `LIA` + `060201`），其余 10 / 16 没有一个 ≥ 3 字的文本框是答案的一段。

## 结论：< 80 %，S 登记不做

1. 三条文件内路都够不到带标签的物项：文本框挂在 `DependencyObject` 上，这些组与物项的锚点不相连（a、b 覆盖与候选上限都是 0 / 16）。
2. 管道 run 的 label 是属性拼成的串，按段读是管线号、介质、管径、等级……（`A3jqz0101-OD-100 mm-1.6AR12-WE-50mm`；10 条 run 只有 6 个不同的串，一个串 5 条 run 共用）；run 没有符号放置，路 c 也给不了类。
3. 分支点用 OPC 符号画，路径 → 类判成 PIDOPC（4 / 4 错）。把规则改成 → PIDBranchPoint 就是从答案键学规则，D6 不许（映射打分前定）；即便改了，路 c 的类复原率上限也只是覆盖 6 / 16 = 37.5 %。
4. 样本小：只有 0202 一张图的 16 个物项，A01 一个都不贡献。证据薄，但 0 / 16 没有歧义——不是差一点，是没有一条通路。

不提议实施单。OCS 照旧：`label=` / `class` 只在路径路读到 `_Data.xml` 时给，字节路不给。

## 未测的线索（后来者若再试；本次都没量）

- **标签符号放置**：0202 放了 4 个 `Design\Annotation\Labels\Item Note & Label.sym`（按规则 10 判 PIDNote；不是分母里任何物项的落点）。标签文字可能在放置点名的缓存本体里
  （`2026-09-07-placement-tail-names-the-cached-definition.md`），不在 sheet 的 `igTextBox` 上，路 a、b 只看文本框、看不到它；放置连不连得到物项、本体里的字是不是位号，**未量**。
- **关系记录**：本仓解的 `0x006F Standard Relation` 只见于 `JSite` 的参数化链（变量 → JDim，`JSiteSymbolInformation::relations`）；它或别的关系类记录会不会把标签接到物项，**未查**。
- **由属性拼 run 的 label**：管线号 / 介质 / 管径 / 等级在工厂数据库里（`backup` 那一半，没有备份语料）；`.pid` 自己的 `Unclustered Dynamic Attributes` 带不带这些值**未查**
  （08-07 §4 见过 A01 的四个发布 oid 各在该流出现一次）。

## 复现

```powershell
cargo run --example probe_item_tags_without_data_xml
```

- 库是 pid-parse `26d1887`（探针随其后的提交入库，`src` 不动）；语料 `test-file/export-test/publish-data/{DWG-0202GP06-01,A01}/`，没有检出的图打印 `skip`。
- 两次实跑（2026-09-30）去掉 cargo 自己的行后 131 行逐行相同；末行 `threshold (P-D7): combined tag recovery rate 0.0 % is below 80 %`。
