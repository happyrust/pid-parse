# 备份包自带参考数据：出图侧直接读 `RefData~*.zip`（2026-10-27 实现）

上一轮把 A2 模板从备份包里**手工解压**出来（`unzip -n RefData~4~682.zip …`，见
`docs/plans/2026-10-27-a2-border-templates-what-to-obtain-and-how-they-attach.md` §零）。
这一轮把那一步去掉：`pid-plot` 自己读压缩包里的 `.igr` 模板与 `.sym` 符号体。顺带查到两件事
—— 名字带 BOM（不改就读不到任何条目），以及清单里第 6 项那份模板**在**仓库里却**没有框**。

## 一、为什么此前必须手工解压

SmartPlant 的备份包把参考数据放成 `RefData~SCHEMA~ID.zip`，与 `Export.dmp` 同层：

| 归档 | 内容 | 谁需要 |
|---|---|---|
| `RefData~4~682.zip` | 11 份 `.igr`（`A0-W.igr`、`CPE-SQ-A2.igr`、`CPECCHBA2.igr`、`XIONGANA2.igr` …） | 图纸的图框、标题栏、会签栏 |
| `RefData~4~681.zip` | 644/616 份 `.sym`，树形与共享盘同构（`Design/…`、`Piping/…`、`Equipment/…`、`Instrumentation/…`、`Assemblies/…`、`Toolbars/…`） | 符号体 |
| `~684` `.xlsm`、`~804` `SPPIDDataMap.xml` + `CatalogIndex.mdb`、`~809` 报表 XML | 与出图无关 | 发布侧 |

而出图侧的语料发现只认目录：`border_template_dirs` 找 `Ref/Template Files`，
`symbol_library` 找名字含 `symbol` 的 `Ref/*` 目录。所以「导出备份包」在动手之前先得
`unzip`，而解出来的东西一旦留在语料里，就再也分不清「这张纸的家具来自语料还是备份」。
`backup::zip_index` 只列中央目录（很早就有了），从不解压——发布侧只需要知道包里有什么。

## 二、两个坑都在名字上

**条目名带 UTF-8 BOM。** `RefData~4~682.zip` 里每个条目名都是 `\u{feff}CPECCHBA2.igr`
（目录条目也是，`\u{feff}Assemblies/`）。于是 `Path::file_name()` 拿到的是
`\u{feff}CPECCHBA2.igr`，与 `Path::ends_with("CPECCHBA2.igr")`、`file_stem()` 全都对不上。
`member_name` 负责归一：去 BOM、`\` 统一成 `/`、去掉尾分隔符、压掉空组件。

**名字可能是 GBK。** 这是既有 `decode_zip_name` 的职责（从 `backup::datamap` 移到
`refdata_archive`，两处共用一份），因此取成员一律**按索引**遍历、与归一化名比较，
**不用 `ZipArchive::by_name`**：它按 `zip` crate 自己的解码匹配，遇到 GBK 名字会拒。

## 三、接入点

- **新模块 `src/refdata_archive.rs`，新特性 `refdata-archive`**（`zip` 依赖本身）。
  `refdata_archives(dir)` → 目录里的 `RefData*.zip`；`archive_members(archive, "igr")` →
  按扩展名列出成员元数据（不解压）；`read_archive_member(archive, name)` → 真取字节。
  `backup-exp` 改为经由这个特性启用 `dep:zip`，`pid-parse` 的
  `--no-default-features` / wasm32 门禁仍不含归档读端；`pid-plot` 只骑这一个特性，
  所以「画一张图」不链接 publish 管线、rusqlite 或仓库里的 GPL MDF 解析器。
- **符号库：归档即根。** `SymbolLibrary::with_archives(…)` 把归档列在**所有目录根之后**，
  于是解压与否不改变任何一张图的读数（解出来的目录仍然赢 —— 有单测钉住：目录里那份读得出、
  归档里同名的读不出，结果必须是目录那份）。按名索引（标题栏那种无盘符的绝对路径）也把归档
  成员一起建进去，`name_miss` 与 `relative_by_name` 的「唯一」判断因此跨目录与归档同样成立。
  `read_symbol_geometry_from_bytes`（`PidPackage::from_bytes`，不落临时文件）是同一份 `.sym`
  解码的字节入口，`read_symbol_geometry` 变成它的一层外壳。
- **模板：归档成员与磁盘文件共用一套判据。** 成员以 `<zip 路径>\<成员名>` 作 label
  （`ArchiveFile::label`），于是 `ends_with`、`file_stem`、链名前缀、误差量化全部照常工作。
  目录源排在归档源之前，理由与符号库相同。

## 四、实测

仓库里本来就带着一个整包夹具：`test-file/backup-test/DWG-0202GP06-01_p/`，它的
`~682.zip` 有 11 份 `.igr`、`~681.zip` 有 616 份 `.sym`（其余四个归档与本轮无关）。
新闸门 `pid-plot/tests/refdata_archive_corpus.rs` 的读数：

| 图纸 | 页幅 | 穿的模板 | 来自 | 墨迹 | 图元 |
|---|---|---|---|---|---|
| `test-file/DWG-0202GP06-01.pid` | 593.75 × 419.63 | `CPECCHBA2.igr` | `~682.zip` 成员 | 594.00 × 420.00 | 101 |
| `test-file/D06.pid` | 594.33 × 420.57 | `CPECCHBA2.igr` | 同上 | 594.00 × 420.00 | 101 |
| `test-file/DWG-0201GP06-01.pid` | 594.31 × 420.31 | `A2-W - New.igr` | 同上 | 594.00 × 420.00 | 101 |

- 前两张**点名**的分别是 `CPECCHBA2.igr` 与短名 `CPECCH~2.IGR`（前缀 `CPECCH`，
  `CPECCHBA2` 以它开头）—— 同尺寸四份 A2（`A2-W - New.igr` / `A2-W.igr` / `CPECCHBA2.igr` /
  `qsmcqtaz13A2.igr`，墨迹全是 594.00 × 420.00）里，「链名优先」的判据在这一包上同样生效。
- 第三张见 §五。
- **对照**：同一批图换回 `test-file`（那底下 `Ref/Template Files` 只有 A0/A1，且无可达归档）时
  **一张都不挂** —— 所以上表的家具确实是压缩包供的，不是磁盘上哪份副本。
- **符号**：根全部置空、只给 `~681.zip`，`DWG-0202GP06-01` 的符号仍然解析出来
  （同一支测试用 `lay_out_with_symbol_roots` 明说来源，绕开「顺带发现的目录根」）。

## 五、第 6 项是相反的结果：文件在，框不在

`test-file/DWG-0201GP06-01.pid` 的链路写着 `XIONGANA2.igr`（两处：短名一处、UNC 一处），
而 `XIONGANA2.igr` **就在** `~682.zip` 里。它仍然选不上，因为它读出来没有框：

```text
cargo run --example probe_igr_template -- <解出来的 XIONGANA2.igr>
  page None m  entities 6  symbol definitions 2
  kinds {"Other": 2, "Symbol": 1, "Text": 3}
  ink bbox (451.51, 12.91) .. (545.01, 48.91) mm     ← 只有会签栏那三行字

cargo run --bin pid_inspect -- <解出来的 XIONGANA2.igr> --coverage
  Sheet streams: 1
  [ID] Sheet6 -> sheet_streams (653 B)
  Sheet provenance: total=1 declared=1 orphan=0 with_endpoints=0 empty_declared=1
```

6 个实体是 2 `Other` / 1 `Symbol` / 3 `Text`，那三条字是「核准」「校对」「储运工艺」；
`Sheet6` 是 **653 字节的空声明**。文件自己的摘要说明它是什么：

```text
Application: SmartSketch Application      Template: TransAcad.igr
ACAD_GLOBALS.FileName = C:\Users\sp3d\Desktop\xionganA2.dwg
File Version: SmartSketch(R) 6.*/RAD 4.2.
Created: 2025-12-25T08:12:27Z              Author: CPE / LastAuthor: sp3d
```

即一张 `.dwg` 经 SmartSketch 转出来的图，不是 SmartPlant 的图框模板：框线既不在 `Sheet6`
也不在任何我们画得出的记录族里。所以这张图穿的是**没点名**的 `A2-W - New.igr`（墨迹同样
594.00 × 420.00，误差 0.31，四份同尺寸里按 label 排序取第一个）。

结论有两面，都要留着：**「拿到 `XIONGANA2.igr`」这件事已经有答案**（文件在仓库的夹具里，
出图侧也读得到），但**它满足不了这张图**，原因在文件自身，不在语料缺件。清单 §2 第 6 项
因此从「要索要」改成「在手但不可用」。

## 六、边界与仍未做的

- **只读 `.igr` 与 `.sym`**：`~684`（`.xlsm`）、`~804`（`SPPIDDataMap.xml` / `CatalogIndex.mdb`）、
  `~809`（报表 XML）与出图无关，仍是发布侧的事。不带 `.zip` 后缀的那几个 `RefData` 文件没读，
  它们不是 ZIP：`~680` 是 332 KB 规则表 CSV、`~703` 是 XML、`~709` 也是 XML，而 **`~683`
  是一张 `Sheet6`/`PSMcluster` 齐全的图纸模板（CFB）**（`2026-10-21-the-plant-backup-exports-every-sheet.md`）——
  如果哪张图的页幅与它相配，它才是下一个要问的对象；本轮没查它的幅面。
- **字体仍只从目录找**（`font_dirs`）：这套备份里没有字体归档，真有的话要单开一条。
- **`XA-A1.igr`（清单第 4 项）与 `CPECCH` 的 A0（第 7 项）仍不在手**：`test-file/工艺管道及
  仪表流程-1.pid` 因此继续穿别族的 `CPE-SQ-A1.igr`（误差 0.00）；`Q井场` 那张图**没有声明页幅**，
  模板这一步整块跳过。
- **§五 那条「A2 没有匹配模板就不画」的用例没改**：它的语料是项目语料（无可达归档），
  `D06` 在那里仍然不挂。归档可达时 `D06` 会开始画 —— 这正是清单 §6 预告过的那一支，
  现在由新闸门 `refdata_archive_corpus` 覆盖，而不动旧那一支。

## 七、复现

```text
# 归档读端：列成员、取成员、BOM 与分隔符归一、错误路径（全合成夹具，不需要备份包）
cargo test -p pid-parse --lib refdata_archive
# 符号库把归档当根：只给归档也解析得出来；目录同名时目录赢
cargo test -p pid-parse --lib symbol_library

# 归档即参考数据：三张图 + 对照 + 只给归档的符号（仓库夹具）
cargo test -p pid-plot --test refdata_archive_corpus -- --nocapture

# 探针：先打印语料里的归档与成员数，再逐张标出模板来自归档还是磁盘
PID_PARSE_SELF_CHECK_DIR=test-file/backup-test/DWG-0202GP06-01_p \
  cargo run -p pid-plot --example probe_which_template_each_sheet_wore -- \
  test-file/DWG-0202GP06-01.pid test-file/DWG-0201GP06-01.pid test-file/D06.pid
```
