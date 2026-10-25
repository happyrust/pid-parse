# `EqTypeDescription` 定在 C22 长文上：`codelist_short_text` 有反证，没有符号路径的容器一个也没有

> 日期：2026-10-25
> 范围：`pid-parse` 数据库线的 `IEquipment EqTypeDescription`（`PIDProcessVessel` / `PIDProcessEquipment` /
> `PIDMechanicalEquipment` / `PIDHeatTransferEquipment`）
> 结论类型：**维持现状** —— 口径仍是「符号路径词干 → `codelist_text`（长文）→ 原值 `EquipmentType`」，
> **不采用** `codelists.codelist_short_text`
> 前置：数据库线 #27（`75d4362`；`src/publish/sqlite_load.rs:384-411` 的选择表注册 +
> `src/publish/xml_writer.rs:472-485` 的三级回退）
> 证据：mission 目录 `contract-work\i4-evidence\`（只读脚本与 `.out` 日志，可复跑）

## TL;DR

1. **要查的是「没有符号路径的容器，参考发的是什么」。查遍盘上的夹具：没有可引用的答案。**
   P01 包导出 `T_EQUIPMENT` 402 行设备全部有表示（0 行无表示、0 行无 `FileName`）；
   DWG-0202GP06-01 导出 4 行设备 0 / 0；TEST02_p 的 MDF 里唯一那台罐
   （`C57494A1B154442C9DF0F4BA713E88EC`）带 `\Equipment\Vessels\Horizontal Drums\Horizontal Drum.sym`。
   全盘只有一行设备没有符号路径（TEST02_p 的 `B3C21360171142798D754AB2151E88BA`），
   而 7 份参考里**没有一块**是它。这一支今天**不可观测**，没有任何证据支持为它改口径。
2. **能观测到的那一半，反证 `codelist_short_text`。** 参考里能落到 C22 行、且该行长文 ≠ 短文的块有 **18** 个：
   参考值 **18 / 18 等于长文、0 / 18 等于短文**。例：`Other equipment`（C22 1759）对 `Equipment`；
   `1 diameter, 1 chamber hemispherical head vertical drum`（C22 212）对 `1D 1C, 1:1 V Drum`；
   `Other heat transfer equipment`（C22 787）对 `Heat transfer equipment`。
3. **结论：维持现状。** `codelist_short_text` 不进回退链；长文与短文相同的 5 个块观测上无法判别，
   也就没有理由偏向短文。
4. **附带发现（只记录，不改）**：有 3 个块的符号词干与 C22 长文不一致，参考发的是**长文**而不是词干
   （I井场 V-10014 / V-10019 的词干是 `1to1Parametric V Drum`；DWG-0202GP06-01 那台罐的词干是
   `Wastewater Pit`，参考是 `Open top tank`）。A01 之所以发词干，是因为 TEST02_p 那份导出里**根本没有 C22**
   （`codelists` 只有 55 / 63 / 66 / 73 / 74 / 88 / 90 / 107 / 111 / 118 / 126 / 135 / 138）。
   「参考其实是 C22 优先」这条不在 I4 允许的两种口径里（两种都把词干放第一级），而且会动到 A01 的
   `Horizontal Drum` 的来源，所以只写在这里，留给将来有独立夹具的功能。

## 1. 问题

#27 把 `EqTypeDescription` 定为三级回退（`src/publish/xml_writer.rs:472-485`）：

1. 符号路径词干（`...\Horizontal Drum.sym` → `Horizontal Drum`）；
2. `CodelistIndex` 查表（`EquipmentType` → `C22` → `codelist_text`）；
3. 原值 `EquipmentType`。

`codelists.codelist_short_text` 在夹具 schema 里早就有（`src/publish/sqlite_load.rs:783-784`），
形如 `1D 1C 2:1 H Drum`，但**从没被读过**：`load_codelist_index` 的 SELECT 只取 `codelist_text`
（`sqlite_load.rs:366-367`），`CodelistIndex` 也没有短文的入口。留下的问题是：第二级该用长文还是短文。
本文按「先取证据再定案」把它定下来。

## 2. 方法（可复现）

- **Oracle `exp` 转储**：自解析（`contract-work\i4-evidence\read_exp_tables.py`）——ASCII 的
  `INSERT INTO "T" (...)` 行 + 二进制列头，u16 **小端**计数与字段长度，列字符集 2000 时载荷是 UTF-16BE，
  NULL 记为 0xFFFE，行分隔是两个 0 字节，表尾 0xFFFF，Oracle NUMBER 按仓库
  `examples/probe_oracle_exp_rows.rs::decode_i64` 解。读 `T_EQUIPMENT`、`T_REPRESENTATION`、`CODELISTS`。
- **MDF**：走 crate 自己的 `pid_parse::publish::open_mdf_as_sqlite` 把 TEST02_p 的 `Export.mdf` 登台，
  再 dump `codelists` / `attributes` / `T_Equipment` / `T_Representation`（临时 example，用完已删）。
- **参考侧**：`scan_references.py` 读 7 份厂商 `_Data.xml`，取每个设备块的 `IObject@UID` 与
  `IEquipment@EqTypeDescription`，与（符号路径词干、C22 `CODELIST_TEXT`、C22 `CODELIST_SHORT_TEXT`）三方比对。
- **设备行普查**：`probe_all_fixtures_eqtype.py` 对盘上每一份 exp 转储 + 上面的 MDF，数 `T_EQUIPMENT` 行数、
  没有表示的、没有 `FileName` 的。

## 3. 证据

### 3.1 参考在这些容器上发的是什么

7 份参考共 25 个设备块，24 个带 `EqTypeDescription`（`集注站…换热器CD` 的 `PIDProcessEquipment` 没有该属性）：

| 参考块 | 数 | 参考 `EqTypeDescription` | 符号路径词干 | C22 长文 | C22 短文 | 判定 |
|---|---:|---|---|---|---|---|
| I井场 `PIDProcessEquipment` | 12 | `Other equipment` | `Wellhead Christmas Tree 02` / `Wellhead Control Panel` / `Chemical Injector` / `Skid Mounted Equipment` | `Other equipment`（1759） | `Equipment` | **= 长文** |
| I井场 `PIDProcessVessel` V-10014 / V-10019 | 2 | `1 diameter, 1 chamber hemispherical head vertical drum` | `1to1Parametric V Drum` | 同参考（212） | `1D 1C, 1:1 V Drum` | **= 长文** |
| I井场 `PIDMechanicalEquipment` | 2 | `Proportioning pump` | `Metering Pump` | 同参考（830） | 同参考 | 长=短，不可判别 |
| I井场 `PIDMechanicalEquipment` | 2 | `Electric motor` | `Electric Motor Type 1` | 同参考（1641） | 同参考 | 长=短，不可判别 |
| 集注站 4 份参考的 `PIDProcessVessel` | 3 | `1 diameter, 1 chamber hemispherical head vertical drum` | 本地无库 | 同参考（212） | `1D 1C, 1:1 V Drum` | **= 长文**（文本比对） |
| 集注站 换热器CD `PIDHeatTransferEquipment` | 1 | `Other heat transfer equipment` | 本地无库 | 同参考（787） | `Heat transfer equipment` | **= 长文**（文本比对） |
| DWG-0202GP06-01 `PIDProcessVessel` | 1 | `Open top tank` | `Wastewater Pit` | 同参考（309） | 同参考 | 长=短，不可判别；词干不符 |
| A01 `PIDProcessVessel` | 1 | `Horizontal Drum` | `Horizontal Drum` | `1 diameter, 1 chamber, elliptical head horizontal drum`（241） | `1D 1C 2:1 H Drum` | = 词干（该夹具无 C22，见 §3.4） |

合计：**18 个块参考 = 长文且长 ≠ 短**（12 + 2 + 3 + 1）、**5 个块长 = 短**（判不出）、
**1 个块参考 = 词干**（A01）。短文单独命中的块：**0**。

### 3.2 没有符号路径的容器：盘上只有一行，而参考里没有它

| 夹具 | 表 | 设备行 | 无表示 | 有表示但无 `FileName` |
|---|---|---:|---:|---:|
| `D:\work\cad\pid-test-data\Export.dmp`（P01 包导出，74 489 856 B） | `T_EQUIPMENT` | 402 | 0 | 0 |
| `test-file\backup-test\DWG-0202GP06-01_p\Export.dmp`（8 802 304 B） | `T_EQUIPMENT` | 4 | 0 | 0 |
| `test-file\backup-test\TEST02_p\extracted\Export.mdf` | `T_Equipment` | 2 | **1** | 0 |
| 同上两份 SQLite 镜像（`Export_full.sqlite` / `Export_v2.sqlite`） | `T_Equipment` / `T_Vessel` | 2 / 1 | 1 / 0 | 0 / 0 |
| `test-file\backup-test\TEST02_p\Export.dmp`（20 012 544 B） | —— | 没有 `T_EQUIPMENT` / `T_REPRESENTATION` 表 | —— | —— |

盘上唯一一行没有符号路径的设备是 TEST02_p 的 `B3C21360171142798D754AB2151E88BA`
（`T_Equipment`，`EquipmentType = 1759`，`TagPrefix = A`；`T_Representation` 里没有它的行）。
它在 **7 份参考里一块都没有**（A01 的参考只有一个设备块，就是那台罐 `C57494A1…`），
所以「没有符号路径的容器，参考发什么」这句话**仍然没有可引用的答案**：
既没有一台没有符号路径的 `PIDProcessVessel`，也没有一台没有符号路径、而参考又描述过的容器。

`集注站…` 那 4 份参考的对象 UID 在盘上任何一份导出的 `T_EQUIPMENT` / `T_REPRESENTATION` 里都查不到
（§3.1 里记作「本地无库」），它们的符号路径状态无法判定，因此**不能**当作「没有符号路径」的证据。

### 3.3 附带发现：三处词干 ≠ 参考值

| 块 | 词干 | 参考值 | 参考值等于 |
|---|---|---|---|
| I井场 V-10014 `E7FD246A423149B28966D9DC5DD4C017` | `1to1Parametric V Drum` | `1 diameter, 1 chamber hemispherical head vertical drum` | C22 212 长文 |
| I井场 V-10019 `F2AE6E4A4C2541AAA084459B29A82911` | `1to1Parametric V Drum` | 同上 | C22 212 长文 |
| DWG-0202GP06-01 `F1081777313D41ABB14B7B18FA210D83` | `Wastewater Pit` | `Open top tank` | C22 309 长文（= 短文） |

三处都是「参考不认词干、认 C22」。这条与 §4 的结论**不冲突**：I4 要定的是 C22 那一级用长文还是短文，
词干那一级是 #27 已定的口径，改动它要另开功能（理由见 §5 候选 B）。

### 3.4 A01 的 `Horizontal Drum` 为什么是词干

TEST02_p 的 `Export.mdf` 里，`codelists` 只有
`codelist_number` = 55 / 63 / 66 / 73 / 74 / 88 / 90 / 107 / 111 / 118 / 126 / 135 / 138，
**没有 22**（`EquipmentType` 那一张）。于是 `T_Equipment` 里那台罐的 `EquipmentType = 241`
在这份导出上查不到 C22 条目，第二级必然落空，参考与我们都落到第一级的词干
`\Equipment\Vessels\Horizontal Drums\Horizontal Drum.sym` → `Horizontal Drum`。
换句话说：A01 的 `Horizontal Drum` 是「C22 查不到时的结果」，不是「C22 输给词干」的证据。

## 4. 结论

`EqTypeDescription` 的口径**维持现状**：

> **符号路径词干 → `codelist_text`（C22 长文）→ 原值 `EquipmentType`。**
> `codelists.codelist_short_text` 不参与这条链。

理由就是 §3：能判别的 18 个块上参考一律等于长文（0 个等于短文），而「没有符号路径」的那一支
在盘上没有任何可引用的参考值（§3.2），没有观测支持为它换口径。

## 5. 被否掉的候选

- **候选 A：第二级改用 `codelist_short_text`。** 否。§3.1 的 18 个块是直接反证：
  参考值 18 / 18 等于长文、0 / 18 等于短文（`Equipment`、`1D 1C, 1:1 V Drum`、
  `Heat transfer equipment` 这些短文一次都没出现过）。
- **候选 B：把 C22 长文提到词干之前（C22 优先）。** 证据确实支持（§3.3 三处），**但不在本项改**：
  1. I4 允许的两种口径都把词干放第一级，改序是另一件事；
  2. A01 的 `Horizontal Drum` 必须继续从符号路径来（回归门禁
     `cli_default_style_matches_a01_reference_delivery_contract` 与单测
     `vessel_eq_type_prefers_symbol_path_over_codelist_to_match_a01` 都钉着这一级）；
  3. 盘上能同时命中两级且参考只认其中一级的夹具只有 I井场与 DWG-0202GP06-01，而这两份的
     数据库发布今天都还没有对应的 IEquipment 输出（I井场交付是 `.pid` 线，DWG 的 MDF 夹具缺
     `extracted\Export.mdf`），改序无法被现有门禁覆盖。
  留待将来单独的功能：那时要同时更新 `vessel_eq_type_prefers_symbol_path_over_codelist_to_match_a01`
  并给 A01 / I井场 / DWG 三条线各自的 IEquipment 输出补上钉住参考的测试。

## 6. 落地与回归

- **口径写在这里，也写在代码旁边**：`src/publish/xml_writer.rs:472-485` 的三级回退注释现在点名了
  「`codelist_text` 是唯一的选择表来源、`codelist_short_text` 有意不读」以及本文。
- **两条 pin 测试**（把口径钉住，防止将来无证据地改）：
  - `publish::xml_writer::tests::vessel_eq_type_ignores_codelist_short_text` —— 无符号路径的罐，
    C22 241 行同时带长文与短文（`1D 1C 2:1 H Drum`），断言发的是长文；
  - `publish::sqlite_load::tests::load_codelist_index_ignores_codelist_short_text` —— 只带短文的
    C22 行不注册条目（短文不是来源）。
- **行为未变**，因此 A01 的交付契约与既有夹具计数不动。改动前基线（`HEAD = 434ecd8`）：

| 命令 | 结果 |
|---|---|
| `cargo test --locked -p pid-parse --lib` | 1340 passed / 0 failed |
| `cargo test --locked -p pid-parse --test publish_sqlite_load` | 4 passed / 0 failed |
| `cargo test --locked -p pid-parse --test publish_xml_cli` | 21 passed / 0 failed |
| `cargo test --locked -p pid-parse --test publish_meta_parity` | 6 passed / 0 failed |
| `cargo test --locked -p pid-parse --test publish_from_pid` | 16 passed / 0 failed（63.94 s） |
| `cargo clippy --locked -p pid-parse --all-targets -- -D warnings` | 0 告警 |
| `cargo fmt --all -- --check` | 干净 |

  改动只加注释与上面两条单测，`--lib` 因此从 1340 变 1342，其余套件应逐字不变。
