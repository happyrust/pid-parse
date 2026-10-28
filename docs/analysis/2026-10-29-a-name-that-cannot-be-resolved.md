# §2.10 的「名字找不到」不是顶替的理由：四份「缺模板」里有三份在盘上（2026-10-29 量）

`#41` §2.10 留着一条判据问句：页面链了一份盘上没有的 `.igr` 时，是按页幅**穿别族的家具**，还是
**不画**？旧记录把它推给参考出图（「把一个可能是对的错，换成确定的不画」）。这一页用 §2.2 那套
方法（把框、链名、盘上的文件、穿上的模板、标题栏的单位名并排量）量了 53 张备份与 `test-file`
夹具，结论是**不用参考出图也能定**，而且旧前提有一半是错的：

1. **53 张里这条顶替一次也没发生。** 49 张穿上模板的页，链名词干与实际穿上的文件 **49/49 一致**；
   4 张没穿的全部**没有声明页幅**（§2.13），不是被拒。50 张带链、3 张无链，无链的 3 张也都没有
   页幅 —— 范围顶替在真实备份里从未被需要。
2. **四份「缺模板」有三份在盘上。** `CPECCHBA2.igr` 与 `XIONGANA2.igr` 就在画的包
   `RefData~4~682.zip` 里；`CPECCH~3.IGR`（`Q井场`）的词干命中 `CPECCHB-SQ-A1.igr`，没穿上是因为
   那张页没有页幅（§2.13）。真正「名字在盘上找不到」的只有 `XA-A1.igr` 一份。
3. **顶替会换掉图纸的作者单位。** `CPE-SQ` 族写的是「中国石油集团工程设计有限责任公司华北分公司」
   / `CPENC-DD11008`；`CCHB` 族写的是「中国石油工程建设有限公司」/ 工程设计证书 `A111017147`。
   穿错族不是排版差异，是在图纸标题栏里印上另一家单位与另一个证书号。
4. **同一张图，三种语料三种家具。** `DWG-0202GP06-01` 在 `test-file` 语料下穿
   `CPECCHB-SQ-A2.igr`，在五图语料下穿 `CPE-SQ-A2.igr`，在它自己的包目录下穿
   `CPECCHBA2.igr`。顶替读的不是图，是这次把哪个目录当语料。

所以判据：**不保留**。页面点了名，就只穿它点的那份；名字解析不到时不画家具（并记下缺的是哪份），
无链页才走 2 mm 范围。理由不是「宁可少画」，是名字是唯一能说明**标题栏属于谁**的证据，而页幅只
说明**纸多大**。

## 一、53 张：顶替一次也没发生

读数来自 `probe_frame_vs_template`（逐张打印页幅、链、实际穿的模板、正文墨），2026-10-29：

| 项 | 数 |
|---|---:|
| 页 | 53 |
| 带链 | 50（4 张 `.PID` + 46 张 `.IGR`） |
| 穿上模板 | 49 |
| 链名词干 = 所穿文件名 | **49 / 49** |
| 无声明页幅（不穿） | 4 |
| 靠范围穿上别族 | **0** |

没穿的 4 张是 `D井场`、`Q井场`、`换热器CD`、`一级闪蒸分离器B气相出口` —— `page_dims=None`，框记录
要么没有、要么与页幅无关（`Q井场` 的 `igSmartFrame2d` 是 841.15 × 594.40，但页幅仍是 0.00）。
`Q井场` 是唯一「有名却无家具」的页：链 `CPECCH~3.IGR`，词干 `CPECCH` 本来就能命中
`CPECCHB-SQ-A1.igr`，**不是缺文件**，是 §2.13 的页幅问题。

## 二、三张夹具页：两名在盘上，一名是碎片

`test-file` 的 7 张里，6 张穿上模板，其中 2 张名字与文件一致（`D06`、`集注站…B`），另外 3 张
（`DWG-0202` 有一份副本，共 4 行）是旧记录里的「族盲」：

| 图 | 页幅 | 链 | 名字在盘上？ | `test-file` 语料穿的 | 自己的包当语料穿的 |
|---|---|---|---:|---|---|
| `工艺管道及仪表流程-1` | 841.00 × 594.00 | `XA-A1.igr` | **没有**（全盘无此文件） | `CPE-SQ-A1.igr` | —（无包） |
| `DWG-0202GP06-01` | 593.75 × 419.63 | `CPECCHBA2.igr` | **在** `RefData~4~682.zip`（594.00 × 420.00） | `CPECCHB-SQ-A2.igr` | **`CPECCHBA2.igr`**（差 0.37 mm） |
| `DWG-0201GP06-01` | 594.31 × 420.31 | `XIONGANA2.igr` | **在**同包，但 `.igr` 只导出 6 个实体、93.5 × 36 mm | `CPECCHB-SQ-A2.igr` | 同（差 ~500 mm，仍拒） |

三行的细节：

- **`CPECCHBA2.igr` 是一份完整的 A2**（墨迹 594.00 × 420.00，17 条字、78 个实体），标题栏写的是
  「中国石油工程建设有限公司华北分公司」与证书 `A111017147`。`DWG-0202` 的 `Template` 元数据
  只额外写着 `CPECCHBA2-new.pid`，与链同族。它的包目录当语料时按名直取就是这份，说明**名字与
  文件都是好的，缺的是搜索根**：`border_template_sources` 只看语料自身与父目录的
  `RefData~*.zip`，而这份包在 `test-file/backup-test/DWG-0202GP06-01_p/` 下。
- **`XIONGANA2.igr` 不是一个页模板的完整导出**。它自己的 `.pid` 声明
  `594.00008 × 420.00002`、`frame state=Embedded`；导出的 `.igr` 只有 3 条字（「核准」「储运工艺」
  「校对」）与 2 个 `Other`、1 个 `Symbol`，墨迹 `(451.51, 12.91) .. (545.01, 48.91)` —— 那正是
  A2 页右下角标题栏的位置，按 1:1 放上去会落在对的地方，只是没有外框。按墨迹范围它离 594 × 420
  差 ~500 mm，所以连命名窗口也拒它。
- **`XA-A1.igr` 确实不在盘上**：全仓与两份测试数据的 `RefData~*.zip` 里都没有 `XA` 成员。
  `工艺管道及仪表流程-1` 的框 841.00008 × 594.00004，`CPE-SQ-A1.igr` 的墨迹正好 841 × 594，
  所以「按范围」会稳定选中它 —— 稳定，但没有一条证据说这张熊安 LNG 的图穿过 CPE 的标题栏。

## 三、为什么「按范围顶替」不算读图

1. **页幅证明的是纸，不是标题栏。** A1 模板全在 841 × 594，A2 全在 594 × 420；范围能分尺寸，
   分不了族。选中「最近的一份别族模板」在几何上总是成立，在文档上往往是假的。
2. **它不是稳定的。** 同一张 `DWG-0202`，`test-file` → `CPECCHB-SQ-A2`、五图语料 →
   `CPE-SQ-A2`、自己的包 → `CPECCHBA2`。一份出图不应随 `--corpus` 改变标题栏上的单位名。
3. **它顶替掉的正是「这页是谁画的」。** `CPE-SQ` 34 条标签、`CCHB` 17/24 条标签，两族的单位名与
   证书号不同（见抬头）。图框与标题栏是图纸的署名部分，不是装饰。
4. **真实语料没有一例支持它。** 53 张 49/49 靠名字；4 张无家具全是没页幅。唯一的三个顶替案例
   都在夹具里，其中两个的文件其实在盘上。

## 四、判据（建议）

| 页面的证据 | 穿什么 |
|---|---|
| 有链名，文件可解析（盘上或 `RefData~*.zip` 成员），且在命名窗口（10 mm）内 | 穿它（现状） |
| 有链名，解析不到，或解析到的文件过不了窗口 | **不穿**，并记下缺的名字（供报告） |
| 无链名 | 保留 2 mm 范围（没有更好的证据） |

两个前置（不是本条判据的一部分，但决定「缺」有多少）：

- **修搜索根**：一张图旁的 `<图名词干>_p/` 包（或包内 `RefData~*.zip`）应算候选。修好之后
  `DWG-0202` 按名穿 `CPECCHBA2.igr`，不再落在顶替里。
- **单列「名字命中但 `.igr` 只是标题栏碎片」**（`XIONGANA2.igr`）：它的墨迹在页坐标里、位置正确，
  但比页小 500 mm。画 1:1 会得到一个有标题栏、没有外框的页 —— 这需要单独判据，不在本条里顺手做。

## 五、实施会动哪些数

- 53 张备份：**不变**（49 穿 / 4 无页幅；顶替本来就没发生）。
- `test-file`：`工艺管道及仪表流程-1` 与 `DWG-0201GP06-01` 不再穿别族家具（分别少一套 34 格 /
  24 格）；`DWG-0202GP06-01` 修搜索根后按名穿 `CPECCHBA2.igr`（101 图元），不修则同样不穿。
- 闸门：`export_every_fixture` 只要求每页有墨、有字、缺符号为空（`工艺管道…-1` 的三个 `XA …`
  是它自己的库，删家具不动这条）；`export_plant_backup`、`border_template`、
  `refdata_archive_corpus` 的语料都显式给出，不受影响。`pid-plot --lib` 需补两条单测：
  有名页在候选里没有名字命中时返回 `None`；无链页仍按范围命中。

## 六、复现

```text
cd <repo>
PID_PARSE_SELF_CHECK_DIR=/d/work/plant-code/cad/pid-test-data/backup-plant-data \
  cargo run -q -p pid-plot --example probe_frame_vs_template -- \
  /d/work/plant-code/cad/pid-test-data/backup-plant-data
PID_PARSE_SELF_CHECK_DIR=test-file \
  cargo run -q -p pid-plot --example probe_frame_vs_template -- test-file
PID_PARSE_SELF_CHECK_DIR=test-file/backup-test/DWG-0202GP06-01_p \
  cargo run -q -p pid-plot --example probe_frame_vs_template -- \
  test-file/DWG-0202GP06-01.pid
unzip -p test-file/backup-test/DWG-0202GP06-01_p/RefData~4~682.zip XIONGANA2.igr > /tmp/XIONGANA2.igr
cargo run -q -p pid-parse --example probe_igr_template -- /tmp/XIONGANA2.igr
unzip -p test-file/backup-test/DWG-0202GP06-01_p/RefData~4~682.zip XIONGANA2.pid > /tmp/XIONGANA2.pid
cargo run -q -p pid-parse --example probe_page_dims -- /tmp/XIONGANA2.pid
```

稳定探针：`probe_frame_vs_template`（页幅 / 链 / 穿的模板 / 正文墨）、`probe_igr_template`
（模板墨迹与标签）、`probe_page_dims`（模板 `.pid` 的页幅与框状态）、`probe_ole_links`（链）。
