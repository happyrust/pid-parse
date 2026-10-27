# A2 图框模板：缺的是哪几份、拿到之后怎么挂上（2026-10-27 核查）

`docs/analysis/2026-10-23-the-border-template-draws-the-sheet-furniture.md` 定下「模板按
**描边范围 = 页幅**认」这条规则，当时记的是「A2 的图纸在这套语料里没有对应模板，所以不画
家具」。本轮把这句话查到底：**磁盘上确实一份 A2 模板都没有**，但**图纸自己把它们的名字写在
链路记录里** —— 所以这不是「不知道缺什么」，而是「知道缺什么、只要拿到就能挂上」。

核查由 `probe_which_template_the_a2_sheets_link`（本轮新增）做：它逐张打印声明的页幅、
图框记录的三态与范围，以及 `igOLELinked` 那只 `JSite` 的 `\001Ole` moniker 列表。

## 零、当日稍后：第 1、2、3 项不必索要，备份包里就有

`D:\work\cad\pid-test-data` 是**完整备份包**，它的 `RefData~4~682.zip` 里就有那三份：

```text
unzip -n RefData~4~682.zip CPE-SQ-A2.igr CPECCHB-SQ-A1.igr CPECCHB-SQ-A2.igr \
    -d "D:\work\cad\PID自研软件验证\Ref\Template Files"
```

（`-n` 不覆盖已存在的；同包另有 `CPECCHBA2.igr` 与 `A0-W - New.igr` 一类，见 §2）解出来之后实测：

- 备份挂模板数 **31 → 47**（正是 §2 记的那 7 + 9 + 2 张，＝47）；
- 但 20 张 A1 页当场被 `CPECCHB-SQ-A1.igr` 顶掉 —— 两份 A1 模板的墨迹都是 `841.0000000000001`
  与 `841.0000000003785`，而页面自报 `841.0000500894253`，**3.8e-10 mm 的噪声**决定归属，
  CCHB 那份还不放任何符号，于是那 20 张的比例波消失。
- 已修：候选先按**链接记录里的名字**过滤（`CPE-SQ~2.IGR` → 前缀 `CPE-SQ`），误差按 `1e-3 mm`
  量化后再比，磁盘名作最后一档。实测 **47 张 / 63 个放置**，模板 × 页：
  `CPE-SQ-A1` 21、`CPE-SQ-A0` 8、`CPE-SQ-A2` 7、`CPECCHB-SQ-A2` 9、`CPECCHB-SQ-A1` 2。
- 复现：`cargo run -p pid-plot --example probe_which_template_each_sheet_wore -- <备份目录>`。
- §5 只做了**前半条**（同名优先）；「找不到同族就不画」仍没做，所以 §4.1 那五张的读数没变。

下文保留当日下午的原样记录（「要拿什么」），已取得的项在 §2 表里标 ✅。

## 一、磁盘上没有 A2 模板，但名字在记录里

整棵可达目录树里只有两份 `.igr`，是同一份内容的两个副本：

```text
D:\work\cad\PID自研软件验证\Ref\Template Files\{CPE-SQ-A0.igr,CPE-SQ-A1.igr}
D:\work\plant-code\cad\pid-parse\test-file\Ref\Template Files\{CPE-SQ-A0.igr,CPE-SQ-A1.igr}
```

而图纸侧的 `\001Ole` 流写着它们指向谁。53 张整厂备份里 45 张至少链一个 `.igr`，名字落在
**四族**里；语料五张与 `test-file` 七张另有第三、第四族：

| 链的名字 | 族 / 幅面 | 用了它的图（备份） | 磁盘上有吗 |
|---|---|---:|---|
| `CPE-SQ~2.IGR` | CPE-SQ / A1（841.00 × 594.00） | 20 | ✅ `CPE-SQ-A1.igr` |
| `CPE-SQ~3.IGR` | CPE-SQ / A0（1189.00 × 841.00） | 8 | ✅ `CPE-SQ-A0.igr` |
| `CPE-SQ~1.IGR` | CPE-SQ / A2（594.00 × 420.00） | **7** | ❌ **缺** |
| `\CPECCHB-SQ-A2.igr`（与 `CPECCH~2.IGR` 同现） | CPECCHB / A2（594.33 × 420.57） | **9** | ❌ **缺** |
| `Ref\Template Files\CPECCHB-SQ-A1.igr`（与 `CPECCH~3.IGR` 同现） | CPECCHB / A1（841.15 × 594.40） | 2（`O井场`、`P井场`） | ❌ 缺（现由 CPE-SQ 的 A1 顶替，见 §4.1） |
| 没有 `.igr` 链 | — | 4（`A井场`、`N井场`、`1200td`、`600td`） | — |
| 没有页幅 | — | 4（`D井场`、`换热器CD`、`凝析油…B气相出口改造`、`Q井场`） | — |

`~1` / `~2` / `~3` 是 **DOS 8.3 的碰撞序号**（同目录同名文件按建入次序编号），不是幅面代码：
同一族里 `~3` 出现在 A0 页上、`~2` 出现在 A1 页上、`~1` 出现在 A2 页上，只是这族文件的建入
次序恰好是从大到小。所以**幅面的证据是页幅本身**，文件名按该族自己的命名法推：
`CPECCHB-SQ-A2.igr` 这个名字是记录里明写的长名，`CPE-SQ-A2.igr` 是照同族 `-A0` / `-A1` 补的。

## 二、要拿的东西（清单）

| # | 文件名（请求用） | 一句话说明 | 影响 |
|---|---|---|---|
| 1 | ✅ `CPE-SQ-A2.igr` | 与我们已有的 A0 / A1 同一族，A2 幅面 | 备份 **7 张**（`集注站 进出站收发球单元 A/B/C/E/G/J/L井场进站生产收球筒…`，页幅恰好 594.00 × 420.00） |
| 2 | ✅ `CPECCHB-SQ-A2.igr` | 另一族（华北分公司版），记录里长名明写 | 备份 **9 张**（`00/02` 三张改造图 + `test/U01` 的 `D01`、`D03`、`D04`、`D06`、`D07`、`TESTDWG-0002TP03-01`，页幅 594.33 × 420.57）；`test-file/D06.pid` 同一份 |
| 3 | ✅ `CPECCHB-SQ-A1.igr` | 第 2 份的 A1 兄弟 | 备份 2 张（`O井场`、`P井场`，页幅 841.15 × 594.40） |
| 4 | `XA-A1.igr`（`\SPID\XaLNG\P&ID Reference Data\Template Files\`） | 熊安项目的 A1 | `test-file/工艺管道及仪表流程-1.pid`（841.00 × 594.00） |
| 5 | `CPECCHBA2.igr`（无连字符） | 第三种拼法 | `test-file/DWG-0202GP06-01.pid`（593.75 × 419.63，仓库内两份副本） |
| 6 | `XIONGANA2.igr` | 熊安项目的 A2 | `test-file/DWG-0201GP06-01.pid`（594.31 × 420.31） |
| 7 | `CPECCH` 的 A0（记录里只有短名 `CPECCH~3.IGR`） | 待定 | `Q井场` 链它，但这张图**没有声明页幅**，先不动（见 §4.3） |

第 1、2 份落定，`53` 张里的挂模板数从 **31 → 47**。第 3 份**不动这个计数**：`O井场`、`P井场` 现在就在画家具（穿的是 CPE-SQ 的 A1），它改的是那两张「画对了没有」—— 取得后确实换成了 CCHB 自己的 A1，那两张的族盲替换因此消掉（见 §4.1）。

第 1–3 项已从 `RefData~4~682.zip` 取得（§零）；第 4–7 项（`XA-A1.igr`、`CPECCHBA2.igr`、
`XIONGANA2.igr`、`CPECCH` 的 A0）磁盘上仍没有，也因此仍在族盲替换，或先不动。

## 三、挂上的判据：现成的规则，一行代码都不用改

`pid-plot/src/model.rs` 的 `load_border_template` 逐份解析模板目录下的 `.igr`，用
`ink_extent_mm` 量它的描边范围，取 **`max(|Δw|, |Δh|) ≤ 2 mm`**（`TEMPLATE_EXTENT_TOLERANCE_MM`）
里误差最小的那一份，**不做任何缩放**。所以拿到文件后要满足的只有一件事：**它的框就是它自己的
范围**。

- 第 1 份：7 张的页幅恰好 `594.00 × 420.00`；同族的 `CPE-SQ-A1.igr` 画的是 `(0,0)–(841,594)`
  且与 B 的页幅误差 0.00 mm。A2 同法出图就是 **0.00 mm** → 直接命中。
- 第 2 份：9 张的页幅是 `594.33 × 420.57`；同族的 A1 兄弟在 `O井场` / `P井场` 上是
  `841.15 × 594.40`（同样带 `+0.15 / +0.40` 那半格）→ 只要 A2 那份同法出图，误差 **0.00 mm**。

放置位置是模板搜索目录里的第一个：**`<语料>/Ref/Template Files/`**（备份与语料用同一个），
仓库内的夹具放 `test-file/Ref/Template Files/`。两份都放进去之后：

```text
cargo run -p pid-plot --example probe_page_defects -- <备份目录>   # template 计数 31 → 47
```

**明确不做的**：把 `CPE-SQ-A1.igr` 按 594/841 缩放成 A2。图框与标题栏的线宽、格子尺寸、
字高都是作者按纸面毫米定的，整体缩放会把它们一并改小；`2026-10-23` 那轮已经写明宁可「不画家
具，也不从别一档缩放上猜」。同理，用 A1 的家具挪坐标自己拼一份 A2 模板也是猜 —— 拼出来的
东西没有参考出图可对，不该进产品。

## 四、核查顺带查出来的三件事

### 4.1 判据是**族盲**的：5 张现在穿着别族的家具

页幅落在 2 mm 窗口里就会被认成那张纸的模板，哪怕图纸链的是别的族：

| 图 | 页幅 | 链的是 | 实际画的是 | 证据 |
|---|---|---|---|---|
| ~~`O井场`、`P井场`~~ | 841.15 × 594.40 | `CPECCHB-SQ-A1.igr` | ~~`CPE-SQ-A1.igr`（误差 0.40）~~ → **已修**：链名优先，现在穿自己点的 `CPECCHB-SQ-A1.igr` | `probe_which_template_each_sheet_wore` |
| `集注站 凝析油稳定塔、泵及换热器(600td)` | 842.54 × 595.54 | 无 `.igr` 链 | `CPE-SQ-A1.igr`（误差 1.54） | 同上 |
| `A井场` | 1189.31 × 841.31 | 一个非 `.igr` 的 OLE 对象 | `CPE-SQ-A0.igr`（误差 0.31） | 同上 |
| `test-file/工艺管道及仪表流程-1.pid` | 841.00 × 594.00 | `XA-A1.igr` | `CPE-SQ-A1.igr`（误差 **0.00**） | 同上（该图 34 条 `SimSun` 正是模板的 34 条标签） |

模板的 34 条标签全是 `SimSun`，所以「这张纸有没有家具」在 `probe_page_defects` 的字体列上
直接可读：7 张 CPE-SQ A2 那张（`A井场进站生产收球筒`）字表里没有 `SimSun`、也没有
`furniture=true` 的重复项 —— **它确实一条家具都没画**。

这 5 张现在都没有参考出图，判定不了「该不该穿别族的家具」。这是**既有的口径**，不是本轮的
回归；只是现在有名有姓，能选了。

### 4.2 `A01` 的 Embedded 帧里没有内容：仓库内没有 A2 夹具

`test-file/export-test/publish-data/A01/A01.pid` 的帧态是 `Embedded / LocallyLinked`，
Embedded 那一条声明 `594.00 × 420.00`（正是 A2）—— 按 `igOLEEmbedded` 的定义「被框的对象
存在本图里」，它本该自带一份 A2 家具。查了：

- `probe_ole_links`：6 个 `JSite`，**没有一个**带 `\001Ole` 流或链；
- `pid_inspect --coverage`：顶层流只有 `JSite*` / `PSM*` / `StyleCluster` / `TaggedTxt*` /
  `DocVersion*` / 两张 `SummaryInformation`，没有 `\001Ole` / `\001CompObj` 一类。

所以那份家具的字节不在文件里（帧只声明了幅面）。**结论：仓库内没有一份 A2 家具可以当夹具**，
A2 这条路要落地必须先有 §2 的文件，或者拿一张 A2 的参考出图反推。

### 4.3 四张没有页幅声明的图

`D井场`、`换热器CD`、`凝析油一级闪蒸分离器B气相出口改造`、`Q井场` 的
`page_dimensions_mm` 是 `None`，模板这一步整块跳过。其中 `Q井场` 的两条帧记录互相矛盾：

- 一条 `Linked 841.15 × 594.40`（A1 幅面），
- 一条 `Embedded 689.60 × 554.30`，
- 链的是 `CPECCH~3.IGR`（按族里 `~3` 用在 A0 页上推，是 A0）。

幅面、链名、帧范围三个数彼此打架，**先不动**：这张图要先有人能说清它画多大。

## 五、可选的口径改动（要不要做由人定）

`load_border_template` 现在只按范围认文件。改成**先按链接名找、找不到再按范围**，可以做到：

- 长名（`CPECCHB-SQ-A2.igr`、`CPECHBA2.igr`、`XA-A1.igr`、`XIONGANA2.igr`）直接当键；
- 只有短名时抽出**族前缀**（`CPE-SQ` / `CPECCH` / `XA` / `XIONGAN`），把范围匹配限制在该族内；
- 该族在磁盘上没有对应幅面的文件时，**不画**，而不是拿别族的顶。

**已做的是前两条的一半**：候选仍按范围筛（`≤ 2 mm`），但同尺寸里**先取图纸点名的那一份**
（`linked_template_prefix` + `template_named_by`），误差再按 `1e-3 mm` 量化，磁盘名作末档。
这一半是**必须**的，不是可选项：不做的话，§2 的第 1–3 项一放进目录就会把 20 张 A1 页的模板
换成别族（`3.8e-10 mm` 的噪声，见 §零），那是**确定的回归**。

**没做的是第三条**：该族没有对应幅面时仍按范围顶替，所以 §4.1 里 `600td`（无链名）、`A井场`
（非 `.igr` 链接）、`test-file/工艺管道及仪表流程-1.pid`（`XA-A1.igr` 缺席）三张读数不变。
它们没有参考出图，所以这次不做等于把「可能是对的错」留着；要动就等一张 A2/A1 参考出图到位，
那时才有判定「该画什么」的依据。

## 六、拿到之后怎么验

```text
# 1) 放进 Ref/Template Files/ 后，先量模板自己
cargo run --example probe_igr_template -- "<语料>/Ref/Template Files"
#    同族应长得像 A0/A1：≈173 图元（Line 104 / Polyline 17 / Arc 18 / Text 34），
#    范围恰是 (0,0)–(594,420)

# 2) 谁挂上了、穿的哪一份、放了什么：31 张 → 47 张 / 63 个放置
cargo run -p pid-plot --example probe_which_template_each_sheet_wore -- <备份目录>

# 3) 重钉三条闸门
cargo test -p pid-plot --test export_plant_backup -- --nocapture
cargo test -p pid-plot --test border_template     -- --nocapture
cargo test -p pid-plot --test export_every_fixture -- --nocapture
```

三条闸门里要改的钉子是明确的：

- `export_plant_backup` 现在钉 53 张的模板计数与一堆下界读数；涨到 47 之后按新数重钉，
  并确认没有别的数字（图元、文字、重复项）被家具带偏 —— 家具会新增 ≈ 173 图元 + 34 标签/张。
- `border_template` 现在有两支：A1（`B` 夹具）与 A0（`I井场`）逐字对照模板，外加一支
  「没有匹配模板的 A2（`D06`）不画家具」。第 2 份文件到位后**最后那一支要反过来**：
  `D06` 应画出 CCHB 的家具，且它的两条框线与 34 条标签要逐条对得上（那时才有第一份 A2 证据）。
  查清了：`D06.pid` 链的正是 `\CPECCHB-SQ-A2.igr`，所以那一支的前提本来就是「**语料**没有这份
  文件」而不是「这张纸没有框」—— 把这份 `.igr` 放进 `test-file/Ref/Template Files` 它就会开始画，
  该支要一并改写（本轮没往仓库加这 233 KiB 的二进制夹具，所以该支仍是绿的）。
- 五张参考图（`plot_compare`）里没有 A2，读数不受影响；`write_five_drawings` 同理。

## 七、复现

```text
cargo run --example probe_which_template_the_a2_sheets_link -- <a.pid|dir> ...
cargo run --example probe_ole_links -- <a.pid>
cargo run --bin pid_inspect -- <a.pid> --coverage
find "<语料>" "<备份>" -iname "*.igr"
```

`probe_which_template_the_a2_sheets_link` 的输出每行是
`幅面页幅 帧态与范围 igr=[链到的名字] 文件名`，末行给三类计数；本页所有「谁链了什么」都是它
一次跑完 53 张的读数。
