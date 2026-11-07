# 凝析油一级闪蒸分离器 B 气相出口改造图：`.pid` 里没有图框（2026-11-07）

## 结论

备份图 `集注站 凝析油一级闪蒸分离器B气相出口改造工艺及自控流程图`（`backup-plant-data\00\02`，下称 B）
导出时不带图框，原因在 `.pid` 本身，`pid-plot` 的解析和模板选择没有出错。这张图里没有图框对象：没有页框
`igSmartFrame2d`，没有带 `\x01Ole` 的 JSite，没有 OLE 客户端站点，承载图框的图层对象数为 0。姊妹图
`…分离器A气相出口改造…`（下称 A）这几处都有，并链接 `CPECCHB-SQ-A2.igr`。

厂家数据库 `T_DRAWING.TEMPLATE` 对 B 仍写 `CPECCHB-SQ-A2.pid`。这一列只记录建图时选用的图纸模板，
不决定出图时有没有图框：换热器CD 同样写着 `CPE-SQ-A1.pid`，`.pid` 里同样没有图框，而它的原厂参考图
整页没有图框。

`pid-plot` 不改。`export_plant_backup` 中 53 张图有 3 张不带模板，这一点不变。

## 逐项对比

| 位置 | A（带框） | B |
|---|---|---|
| `/Sheet6` 中的 `0x003D igSmartFrame2d`（滑窗扫描，载荷 156…4096 B） | 10 个信封，第一个在 `+8`：载荷 246 B，oid 22，`+8` 图层 14，`+14` 内容标志 `0x5C808011`，`+26` 链接标志 `0x20E90040`（Linked），`+76/+84` 为 594.328 × 420.566 mm，`+156` 站点 23。解码器读出 1 个页框 | 0 个（流长 89 211 B）。解码器读出 0 个 |
| 根存储图层的对象数 | `Notes`（oid 14，#3）为 1，`DrawingBorder`（oid 13，#2）为 0 | `Notes` 为 0，`DrawingBorder` 为 0 |
| 带 `\x01Ole` 的 JSite | `JSite23`：`\x01Ole` 335 B，ANSI 路径 `C:\PID_SQ~1\SQPlant\Ref\TEMPLA~1\CPECCH~2.IGR`，Unicode 路径 `C:\PID_SQProject\SQPlant\Ref\Template Files\CPECCHB-SQ-A2.igr` | 无。33 个 JSite 存储全部是符号缓存或嵌套子文档 |
| `JSitesList` | 152 B，36 项，包含 23 | 136 B，32 项，没有一项是 OLE 站点 |
| 根 `PSMcluster0` 的 `0x004C` OLE 客户端站点 | 站点 23 | 无（546 条记录一直铺到流末尾） |
| `0x0004` OLE 站点管理器 | `02000000 01000000 00000000` | 相同 |
| `\x05SummaryInformation` 模板（PROPID 7） | `normal.pid` | `normal.pid` |
| `TaggedTxtData/Drawing` 的 `Template` | 没有这个流（`TaggedTxtData` 下只有一个空的 `Revision`） | 同 A |
| `Export.dmp` 中的 `T_DRAWING.TEMPLATE` | `CPECCHB-SQ-A2.pid` | `CPECCHB-SQ-A2.pid` |

同为 CCHB A2 的 `轻烃分离器I` 和 `低压气压缩机入口改造` 与 A 一样：页框在 `/Sheet6 +8`，`JSite23` 链接
`CPECCHB-SQ-A2.igr`，有站点 23，`Notes` 图层 1 个对象。

## `pid-plot` 的选择过程

`lay_out_with_symbol_roots` 从两处取页面：页框（`decoded_page_frame`），或 `TaggedTxtData` 里的模板名；
链接名取自各 JSite `\x01Ole` 流中的 ASCII 串。B 两处都没有，所以 `page_mm` 为 `None`，`linked_prefixes`
为空，`furniture_page` 为 `None`，不会调用 `load_border_template`。页面由 `resolved_page_mm` 量出，为 A2，
这就是 `export_plant_backup` 中 `DERIVED_PAGES` 计入的那一张。按文件名找模板也无从做起：B 里没有任何
`.igr` 或 `.pid` 路径串，ASCII 和 UTF-16 都查过。

## 为什么不能按数据库那一列补图框

`T_DRAWING.TEMPLATE` 是建图时选的图纸模板（一个 `.pid`）。图框只是该模板里的一个对象，建图之后可以删除。
`换热器CD` 的 `TEMPLATE` 是 `CPE-SQ-A1.pid`，`.pid` 里没有页框也没有 OLE 站点；它的原厂参考图（验收数据，
Amyuni 出图）整页没有图框，可见 SmartPlant 出图时没有按这一列补图框。`D井场`（`CPE-SQ-A0.pid`）的情况
相同。如果 `pid-plot` 按这一列给 B 补上 CCHB A2 图框，画出来的就是 SmartPlant 不会画的东西。

## 53 张备份图

用 `probe_does_the_sheet_link_a_border` 把 53 张都跑了一遍：50 张带链接图框，其中 46 张链接 `.igr`，
4 张链接 `.pid` 图纸模板（凝析油稳定塔 1200td、600td，A井场，N井场）；3 张没有图框（`D井场`、`换热器CD`、B）。
这与 `export_plant_backup` 的 `TEMPLATED_SHEETS = 50` 一致。

## 与 CT-2 的关系

CT-2 查的是闪蒸A、低温A：`.pid` 里有图框对象，但参考图不画（`2026-11-07-the-two-bare-a1-plots-are-not-in-the-pid.md`）。
本条的情况不同，B 根本没有图框对象。`T_DRAWING.TEMPLATE` 也区分不了 CT-2 的两组：闪蒸A、低温A、闪蒸B、
空冷器A/B/C 都写 `CPE-SQ-A1.pid`。

## 复现

```text
cargo run --example probe_does_the_sheet_link_a_border -- \
    --dump "D:\work\plant-code\cad\pid-test-data\Export.dmp" \
    "<backup>\00\02\集注站 凝析油一级闪蒸分离器A气相出口改造工艺及自控流程图.pid" \
    "<backup>\00\02\集注站 凝析油一级闪蒸分离器B气相出口改造工艺及自控流程图.pid" \
    "<backup>\00\00\集注站 烃水露点控制单元（二） 换热器CD及外输阀组工艺及自控流程图.pid"
```

`<backup>` 是 `D:\work\plant-code\cad\pid-test-data\backup-plant-data`。
