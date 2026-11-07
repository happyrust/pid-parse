# `工艺管道及仪表流程-1` 的模板与符号就在它自己的工厂备份里；其余 7 个缺失符号本机没有

> 日期：2026-11-07
> 范围：`test-file/Ref`（夹具参考数据）、`pid-plot`（符号查找根，`symbol_library`）
> 结论类型：**接入** `XA-A1.igr` 与 3 个 `Xa …` 符号；**登记缺失** 7 个备份图符号与
> 3 张待到的参考 PDF（附查找清单）
> 证据类型：SHA-256 逐字节比对、全盘按文件名检索、备份包条目清点

## 0. 一句话

`test-file/工艺管道及仪表流程-1.pid` 与 XaLNG 工厂备份 `PLANT_XaLNG_1_1_3_p` 里的
`180700/气化区/工艺管道及仪表流程图（一）.pid` 逐字节相同，而同一个备份的
`RefData~4~682.zip` 带着它的框链接的 `XA-A1.igr`、`RefData~4~681.zip` 带着它点名的
`Xa.sym` / `Xa chu.sym` / `Xa Item Note & Label.sym`。这四个文件现已入库，夹具戴上自己
的图框、不再缺符号。备份图缺的另外 7 个 `.sym` 在本机 C:/D:/E: 全盘和所有备份包里都不存在；
N/O/P/Q 井场、D04、收球筒的原厂参考 PDF 也还没到。

## 1. 夹具从哪来

| 文件 | 大小 | SHA-256 |
|---|---:|---|
| `test-file/工艺管道及仪表流程-1.pid` | 450,560 | `a3cceb23bc50c0b0188ed6524fe6215f9eb0ad4c19f8e235a3ce09256249fb38` |
| `PLANT_XaLNG_1_1_3_p\PlantData~2~711.zip` 条目 `180700/气化区/工艺管道及仪表流程图（一）.pid`（GBK 文件名） | 450,560 | 同上 |

备份在本机 `D:\pid\Programming Help\PLANT_XaLNG_1_1_3_p`。它是一个完整的 SmartPlant 备份包
（`Export.dmp`、`PlantData~2~711.zip`、`RefData~4~680…809`），另把参考数据展开在包根目录。

## 2. 接入了什么

| 入库位置 | 备份包条目 | 大小 | SHA-256 |
|---|---|---:|---|
| `test-file/Ref/Template Files/XA-A1.igr` | `RefData~4~682.zip` → `XA-A1.igr` | 434,176 | `637d3968e52ebda0b98b68d19cdcfe248b6eabb89607335746ba03fd93c65a28` |
| `test-file/Ref/Symbols/Design/Annotation/Labels/Xa Item Note & Label.sym` | `RefData~4~681.zip` 同路径 | 94,208 | `5552ab94a6f7f7c9ae5c5c1101c32a03fc0b705eef4afb11b22e426c8530fbd7` |
| `test-file/Ref/Symbols/Piping/Piping OPC's/Xa chu.sym` | 同上 | 45,056 | `1a94242cc04424cec80b0b059d06e1f586695e6d6de15dd4076ca07d7d6a72bf` |
| `test-file/Ref/Symbols/Piping/Piping OPC's/Xa.sym` | 同上 | 45,056 | `1d863e8df3061d6e06c3e3481785b6cc783bb58fa722646cd6480a93fc689004` |

包根目录展开的同名文件哈希相同。

- `export_every_fixture` 拿五图语料（`PID自研软件验证`）当 corpus，不看 `test-file/Ref`，
  所以 `symbol_library` 在 `test-file/symbols-full`（本地、被 `.gitignore`）之后再加一个
  入库的 `test-file/Ref/Symbols` 根。这个根只有上面三个名字，别的库都没有，排在最后，
  不会改变任何其他图纸的解析。该测试原来对这张图豁免三个缺失名字，现在收紧为每个夹具都不缺符号。
- `model.rs` 里原来拿 `XA-A1` 当「点名但没有文件」范例的三个单测，改用合成名
  `NOWHERE-A1.igr`；「缺模板时报出名字」改为把熊安图放进一个没有 `Ref`、旁边也没有备份包的
  空目录里排版，仍然报 `["XA-A1"]`。新增 `the_xa_fixture_wears_its_own_template_and_symbols`：
  以 `test-file` 为 corpus 排版，戴 `XA-A1.igr`，`missing_templates` 为空，三个 `Xa …` 名字都不缺。
- 读数：模板墨迹 841.000 × 594.000 mm、页面 841.00008 × 594.00004，偏移 (0, 0)、缩放 1，
  模板贡献 4999 个图元；本地（有 `symbols-full`）缺失符号 0。
- `XA-A1.igr` 与 `CPE-SQ-A1.igr` 墨迹都是 841 × 594，未点名的 A1 页面按尺寸挑模板时两者打平，
  按路径排序取 `CPE-SQ-A1`——与 A2 那对（`CPECCHB-SQ-A2` / `CPE-SQ-A2`）是同一种平局，
  这也是点名的页面从不借用陌生模板的原因。单测里写明了这一点。

## 3. 查过哪里

- 全盘按文件名：`rg --files --no-ignore --hidden`（跳过 `target`、`node_modules`、`.git`）
  扫 C:\、D:\、E:\ 和 Temp 盘符，模式包括 `*xa-a1*.igr`、`*pulsation damper*.sym`、
  `*wellhead christmas tree*.sym`、`*clone of tt*.sym`、`*isa 2 way generic body*.sym`、
  `*isa_logic ctrl*.sym`、`*concentric size change*.sym`、`*qiufa*.sym`、三个 `xa` 符号名，
  以及 `*N井场*.pdf`、`*O井场*.pdf`、`*P井场*.pdf`、`*Q井场*.pdf`、`*D04*.pdf`、`*收球*.pdf`。
- 两个 `pid-test-data` 根（`D:\work\plant-code\cad\pid-test-data` 与 `D:\work\cad\pid-test-data`，
  同名文件大小一致）：逐个打开 `RefData~4~681/682/684/685/804/809.zip`、`PlantData~2~711.zip`
  清点条目；无扩展名件按头字节判型——`RefData~4~680` 一致性规则 CSV（`"Begin Rules"`）、
  `RefData~4~683` SmartSketch OLE 文档（`Sheet6` 只有 8 字节、`StyleCluster` 48,032 字节，
  是样式种子而不是模板）、`RefData~4~703` xlsx、`RefData~4~709` 保温规格 XML、
  `PlantData~2~722` `SPEMDataMap.xml`。
- `test-file/backup-test` 下两个包的 `RefData~4~682.zip` 都带 `CPECCHBA2.igr`（180,224 字节）
  与 `XIONGANA2.igr`，所以 DWG-0202 的 `CPECCHBA2.igr` 一直在它自己的包里，并不缺。

## 4. 没找到的

- 7 个备份图符号（`pid-plot/tests/export_plant_backup.rs` 的 `KNOWN_LIBRARY_GAPS`）：
  `pulsation damper.sym`、裸名 `wellhead christmas tree.sym`（库里只有 `01` / `02` 两个变体）、
  `clone of tt-temperature transmitter(1).sym`，以及 D04 指向作者临时目录的
  `isa 2 way generic body.sym`、`isa_logic ctrl.sym`、`concentric size change.sym`、`qiufa.sym`。
  以上全盘与所有备份包条目 0 命中；这些放置都由图纸自带的缓存体画出，名单保持不变。
- 新参考 PDF：`验收数据` 仍只有 11 张 Amyuni 4.5.2.9 出图（PDF-1.3）。
  `pid-test-data\backup-plant-data-pdf` 下的 53 张（10-21 01:50–01:52 一批、
  `2026-10-27` 子目录一批）是 PDF-1.7、不带 Amyuni 标记，开头对象结构与 `pid-plot` 自己的导出
  相同，是我方导出物，不能当参考图。

## 5. 复现

```powershell
Get-FileHash -Algorithm SHA256 "test-file\工艺管道及仪表流程-1.pid"
# 在 PlantData~2~711.zip 里按 GBK（代码页 936）读条目名，找 450560 字节的条目并取哈希
cargo test --locked -p pid-plot --lib model::tests::the_xa_fixture_wears_its_own_template_and_symbols
cargo test --locked -p pid-plot --test export_every_fixture
```
