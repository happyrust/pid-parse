# 闪蒸A、低温A 的参考图不画图框：`.pid` 里没有区分它们的字段

日期：2026-11-07

## 结论

`集注站 凝析油闪蒸单元 一级闪蒸分离器A`（下称闪蒸A）和 `集注站 烃水露点控制单元（一） 低温分离器A`
（低温A）的原厂参考图整页没有图框、标题栏线条和模板文字，只有 `.pid` 自己的标题栏标签；
`reference_plots_acceptance` 的整页读数因此多出 3478.898 / 3478.694 mm（我方画了两张图都链接的
`CPE-SQ-A1.igr`）。同样链接这份模板的闪蒸B、空冷器A/B/C 的参考图都带图框。

把图框可能挂靠的每个位置在两组之间逐字节比过：**29 个字段里没有一个能区分两组**。验收语料里的
`.pid` 和 7/19 备份包里的同名 `.pid` 结论相同。没有可供 `pid-plot` 遵守的 `.pid` 字段，所以
`pid-plot` 不改，CT-1 的整页 pin 照旧记录这一差异。

差异在 `.pid` 之外。同一个 A1 链接（字节完全相同）在 2026-10-12 11:53 出闪蒸B 时解析成功，3 分钟后
11:56 出闪蒸A、11:59:26 出低温A 时都没有画出；而 11:59:16 同一批出的 I井场（A0 链接）画出了。链接写的是
建图那台机器的本地路径 `C:\PID_SQ~1\SQPlant\Ref\TEMPLA~1\CPE-SQ~2.IGR`（备份包的工程根是
`\\MM-128\PID_SQProject\SQPlant`），能否解析取决于出图那一刻的环境。另一种不能排除的可能是：出图时
工程里的这两张图是 7/19 备份之后又改过的版本（只删掉或藏起了图框），而我们手里没有那一版。两种
解释都在 `.pid` 之外。

## 两组

| 组 | 图 | 参考图 |
|---|---|---|
| 不带框 | 闪蒸A、低温A | 无图框、无模板文字 |
| 带框（A1，同一链接） | 闪蒸B、空冷器A、空冷器B、空冷器C | 有 |
| 带框（A0，对照） | I井场、J K井场、L M井场 | 有 |

换热器CD 没有 `JSite23`、没有链接，参考图也没有图框，`model.rs` 早已按此不画，不在本次比较里。

## 查过的字段

"不区分"指两张不带框的图取值不一致，或它们的取值也出现在某张带框的图上。

| 位置 | 内容 | 结果 |
|---|---|---|
| `/Sheet6` 页框 `igSmartFrame2d`（`0x003D`，244 B，oid 22） | 载荷 `+8` 图层 13；`+14` 内容标志 `0x5C808011`；`+26` 链接标志 `0x20E90040`（Linked）；`+36/+44` 原点 0；`+76/+84` 841.00005 × 594.00004 mm；`+156` = 23（与 `JSite23`、`0x004C` 站点记录的 oid 相同）；`+196/+204` 平移 (2.96e-8, 1.72e-9) m；`+212` 缩放 1 | 6 张 A1 **逐字节相同**；A0 只在 `+76/+84`（宽、高）、`+132`（宽的第二份）、`+148`（长宽比）、`+204`（平移 y）这几个 `f64` 上不同 |
| 框所在图层（载荷 `+8` = 13）的 `JSheetLayer`（`0x0081`，58 B） | `DrawingBorder`，编号 2，`+8` = 0，对象数 1，尾字 0 | 全部相同 |
| `0x0057 Top ViewFilterSet` | 显示、可选位图第 2 位（`DrawingBorder`）都是 1；位图 3..6 为 `ffff ffff ff ff`；唯一的覆盖项是认领状态图层（闪蒸A、闪蒸B 为 `ClaimedOnlyByOthers` 灰 `0x949494`，低温A、空冷A 为 `NotClaimed` `0x3F3F3F`） | 位图全部相同；覆盖项不区分 |
| `0x0060 Top ViewFilterSet`（`viewfil.dex` 的第二个类） | 28 B | 全部相同 |
| `JSite23` 存储 | 只有 `\x01Ole`（319 B）和 `JProperties`（`OLES@\0\x01\0`），没有 `\x01CompObj`、`\x03ObjInfo`、缓存呈现流 | 全部 A1 逐字节相同 |
| `JSite23/\x01Ole` 内容 | `+0` 四字节 `c2 5c cb 46`（A0 为 `c2 5c d6 46`，含义未定）；文件名字（CLSID `00000303-…-46`）的 ANSI 绝对路径 `C:\PID_SQ~1\SQPlant\Ref\TEMPLA~1\CPE-SQ~2.IGR` 及 Unicode `C:\PID_SQProject\SQPlant\Ref\Template Files\CPE-SQ-A1.igr`；`+0xE0` 起相对名字 `CPE-SQ~2.IGR` / `CPE-SQ-A1.igr` | 同上 |
| `0x004C` OLE 客户端站点（`olesite.dll`） | `17 00 00 00` + 8 个零（站点 23） | 全部相同 |
| `0x0004` OLE 站点管理器 | 12 B | 全部相同 |
| 属主为框（DA 记录尾部 `field_x`，载荷 `+12` = 22）的动态属性记录，记录自身 id 屏蔽 | `DrawingBorder`、`FromTemplate` 等 | 闪蒸A ≡ 空冷C ≡ I井场（170 B），低温A ≡ 空冷A（204 B）：不区分 |
| `0x0036 JViewSet`、`0x0037 BackDrop`、`0x0086 JSheets`、`0x0088 JSheetLayerGroup` | | 全部相同 |
| `0x0002 INGR Persist Preference DEX` | 只差 `BsplineExtensionMode` 之后的补零长度 | 闪蒸A ≡ 空冷A，低温A ≡ 空冷B ≡ I井场：不区分 |
| `0x0042 JSheetLayerManager` | 图层登记表与编号占用位图 | 低温A 与空冷A 的位图相同：不区分 |
| `0x0076 SheetView` | `+12/+16` 在 7/6 那批（闪蒸A、低温A、闪蒸B、换热器CD）为 0/1，其余为 2/3；`+101..+267` 是视图变换的 `f64` | 不区分 |
| `0x0114 JSheet` | 框的 oid 排第一，后接四个 `i16`（闪蒸A −4/−4/13782/9735，低温A −12/−12/13791/9744，闪蒸B −12/−12/13790/9744） | 不区分 |
| 顶层流 | `AppObject`（闪蒸A ≡ 闪蒸B ≡ 空冷A/B/C）、`DocVersion2/3` 与 `\x05SummaryInformation`（闪蒸A、低温A、闪蒸B、换热器CD 是 7/6 17:18 由 `liuzongshuai_hb` 用 SPPID 09.00.00.0155 重存的，其余是 6 月由 `Administrator` 存的）、`\x05DocumentSummaryInformation`、`PSMroots`、`PSMclustertable`、`PSMsegmenttable`、`JSitesList`、`JTaggedTxtStgList`、`Dynamic Attributes Metadata` | 只跟保存批次或内容规模走：不区分 |
| `StyleCluster` | 几何记录的样式引用在载荷 `+14`，而 `igSmartFrame2d` 的 `+14` 是原生读取器确认的内容标志字；框记录没有可比的样式条目 | — |
| 厂家发布的 `_Data.xml` / `_Meta.xml` | `PIDDrawing` 没有模板或图框属性；`_Meta.xml` 只差 UID 和名称 | 不区分 |

根 `PSMcluster0` 的记录链从字符串表（`SiteObjects`、`PreferenceSet`、`Sheets`、空串）之后开始，
一直铺到流末尾，中间没有空隙。偏移 117 处看起来像 `0x0073` 记录的 10 个字节其实是 `Sheets` 的末字符
`73 00` 加下一个表项；空冷器C 若从偏移 32 起走，会吞掉开头几条真记录、仍然走到末尾。探针因此先走完
字符串表再铺链。

## 参考图一侧

带框的 PDF 内容流一开头就裁到整页（A1 上是 `q n 0 1683.36 2384.16 -1683.84 re W n`），先画模板外框，
再进入会签栏的裁剪块，共 6 个 `re W n`（A2 为 2 个），然后才是图纸本身。闪蒸A、低温A（以及换热器CD）的
流第一笔就是图纸自己的 `n 0.96 w 1 j`，整条流里没有一个 `W n`。也就是说，模板这一层在出图时整个没画，
不是被遮住或被裁掉。

出图时间（PDF `CreationDate`，东八区）：

| 时间 | 图 | 链接 | 图框 |
|---|---|---|---|
| 10-12 11:53:13 | 闪蒸B | `CPE-SQ~2.IGR`（A1） | 有 |
| 10-12 11:56:18 | 闪蒸A | `CPE-SQ~2.IGR`（A1） | **无** |
| 10-12 11:59:16 | I井场 | `CPE-SQ~3.IGR`（A0） | 有 |
| 10-12 11:59:22 | 换热器CD | 无 | 无 |
| 10-12 11:59:26 | 低温A | `CPE-SQ~2.IGR`（A1） | **无** |
| 10-14 16:57:21 – 16:58:31 | J K、L M、空冷器B、C、A、A2 | A0 / A1 / CCHB-A2 | 全部有 |

10-12 那批里，A0 链接在 11:59:16 能解析，A1 链接从 11:56 起不能解析，而 11:53 还能；10-14 那批 A1 链接
又都能解析。`.pid` 一侧的链接字节在这些图之间完全相同。

两套副本的版本：验收语料里 7 月那 4 张（闪蒸A、低温A、闪蒸B、换热器CD）是 7/6 17:18 重存的，对应
`_Data.xml` 17:34–17:36 的发布；备份包（`Manifest.txt` `DateCreated 07/19/2026 20:13:24`）里同名文件
最后一次保存分别在 7/5、7/9、7/12、7/5。两套副本的图框字段都相同。备份早于出图，所以它不能证明出图时
工程里是哪一版。

## 影响

没有代码改动。11 张图整页与带掩膜的读数、53 张备份图的导出都不变；`border_template`、
`export_plant_backup` 不受影响。

## 若要清零这 3478 mm

需要出图一侧的证据，`.pid` 里拿不到：

- 在能访问 `CPE-SQ-A1.igr` 的环境里用同样的方式重出闪蒸A、低温A。若重出的图带框，替换这两张参考图，
  整页 extra 随之归零；
- 或者拿到出图时工程里这两张图的 `.pid`。若那一版确实删掉或藏起了图框，把它放进语料，本文的比较会
  直接指出差异所在。

## 复现

```text
cargo run --example probe_does_the_pid_say_the_border_prints -- \
    --bare   "<corpus>/集注站 凝析油闪蒸单元 一级闪蒸分离器A工艺及自控流程图.pid" \
             "<corpus>/集注站 烃水露点控制单元（一） 低温分离器A工艺及自控流程图.pid" \
    --framed "<corpus>/集注站 凝析油闪蒸单元 一级闪蒸分离器B工艺及自控流程图.pid" \
             "<corpus>/集注站 烃水露点控制单元（一） 空冷器A集预冷分离器A工艺及自控流程图.pid" \
             "<corpus>/集注站 烃水露点控制单元（二） 空冷器B集预冷分离器B工艺及自控流程图.pid" \
             "<corpus>/集注站 烃水露点控制单元（三） 空冷器C集预冷分离器C工艺及自控流程图.pid" \
             "<corpus>/I井场 注采阀组工艺及自控流程图.pid" \
             "<corpus>/J K井场 注采阀组工艺及自控流程图.pid" \
             "<corpus>/L M井场 注采阀组工艺及自控流程图.pid"
```

`<corpus>` 是 `D:\work\cad\pid-test-data\验收数据`。末行读数为
`29 fields over 2 bare and 7 framed sheets: 0 separate the groups`；把六张 A1 换成
`D:\work\plant-code\cad\pid-test-data\backup-plant-data\00\00` 下的同名文件，读数为
`29 fields over 2 bare and 4 framed sheets: 0 separate the groups`。类型名来自
`tools/psm_type_clsid.py`（`sppid-dlls\bin\radsrvitem.dll`，SHA-256 `C47C0DBE…2139`）。
