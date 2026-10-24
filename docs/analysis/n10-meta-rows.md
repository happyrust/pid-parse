# N10 `_Meta.xml` 行与参考对照：T_DRAWING / T_DRAWINGVERSION / T_REVISION 与 5 份参考 `_Meta.xml`（2026-10-14）

> 计划 `docs/plans/2026-10-13-publish-progress-and-next-steps.md` S3（Q3 (a)）。
> 只读 `D:\work\cad\pid-test-data\Export.dmp`（`EXPORT:V12.01`，74,489,856 字节）与 `D:\work\cad\PID自研软件验证\*_Meta.xml`（5 份参考）。不跑 `imp`，不连 Oracle，不改 `src/`，不改两个 `publish\` 交付目录。
> 探针：`examples/probe_oracle_exp_drawing_meta.rs`。`cargo run --locked --example probe_oracle_exp_drawing_meta` 退出 0。

## 一句话

`T_DRAWING` 53 行的 `REVISION`、`VERSION`、`LASTPUBLISHTIME`、`SP_LASTEFPUBLISHTIME` 全是 `0/53`（全空）；`T_REVISION` 是 `0` 行空表；`T_DRAWINGVERSION` 的 `15` 行全部是删图前自动留底的快照（`SP_VERSIONPURPOSE = 3`、`SP_COMMENTS = "Created for Delete"`），它们的 `SP_DRAWINGID` 在现存 `T_DRAWING` 53 行里命中 `0/15`。5 份参考 `_Meta.xml` 的 3 个对象 + 3 条关系共 30 个 UID（含 I井场的 6 个）在 `Export.dmp` 全表与原字节、以及 5 个 `.pid` 原字节里全部是 **0 命中**；参考里的 `DocVersionDate`（I井场 `2026/06/22`、集注站 `2026/07/06`）与 `FilePath`（`\\Glpid-7\pidsq\PID`）同样不在库和 `.pid` 里，而是发布当时那台机器的时间与共享目录。提交 `4866b81` 已用 `derive_meta_uid` 写出这 3 个对象 + 3 条关系（`DocRevision="0"`、`DocVersion="1"`、`MajorRev_ForRevise="0"`，`DocVersionDate` 与 `FilePath` 留空）。

---

## 1. `Export.dmp` 里的图纸与版本相关表

探针先钉 `APPLICATIONS` 第 0 行 `(1, "SmartPlant PID", "SPPID")`，再扫描全部 154 个 `INSERT` 中表名含 `DRAWING` / `VERSION` / `REVISION` / `PUBLISH` / `DOCUMENT` 及 `T_PLANT` / `T_PLANTGROUP` 的表：

| 表名 | 行数 | 列数 | 非空列数 | 说明 |
|---|---:|---:|---:|---|
| `T_DRAWING` | 53 | 33 | 20 | 现存 53 张图纸主表 |
| `T_DRAWINGVERSION` | 15 | 12 | 10 | 已删除图纸的快照留底（`SP_STORAGE` 为 LONG 内嵌 zip） |
| `T_REVISION` | 0 | 11 | 0 | 修订表，空表 |
| `T_DRAWINGSITE` | 53 | 8 | 7 | 53 张图的站点归属（`SP_WSSITEID` 全为 `A7A2C458…`） |
| `T_DRAWINGPROJECT` | 53 | 8 | 7 | `SP_DATEINCLUDED` 与 `T_DRAWING.DATECREATED` 同值 |
| `T_GLOBALDRAWING` | 53 | 4 | 4 | `SP_ID`、`NAME`、`SP_DRAWINGID`、`DRAWINGNUMBER` |
| `T_DRAWINGSUBSCRIBER` | 0 | 4 | 0 | 空表 |
| `T_PLANT` | 1 | 6 | 1 | 仅 `SP_ID` 非空，证书号 / 设计院 / 项目号全空 |
| `T_PLANTGROUP` | 6 | 16 | 11 | `SQPlant` 树（3 个叶子单元对应 53 张图） |
| `VERSION` / `VERSION#2` | 1 / 6 | 4 | 4 | Schema 版本字典表，非图纸版本 |
| `VERSIONINFO` / `VERSIONINFO#2` | 4 / 4 | 3 | 3 | Schema 组件版本字典表 |

---

## 2. `T_DRAWING`（53 行，33 列）版本与时间字段实测

将 Oracle 内部 7 字节 `DATE`（类型 12：`世纪+100, 年+100, 月, 日, 时+1, 分+1, 秒+1`）解码后，53 行与 `_Meta.xml` 相关的列统计如下：

| 列号 | 列名 | 类型 | 非空行数 |  distinct | 实测值 / 样本 |
|---:|---|---|---:|---:|---|
| 0 | `SP_ID` | CHAR | 53 / 53 | 53 | 图纸 `DocUID`（I井场为 `5633AF4121EE4F4D9FE78E90722CC050`） |
| 1 | `DESCRIPTION` | CHAR | 0 / 53 | 0 | 全 NULL |
| 2 | `UPDATECOUNT` | NUM | 53 / 53 | 11 | `1` .. `51`（I井场为 `1`） |
| 3 | `NAME` | CHAR | 53 / 53 | 53 | 图名（I井场为 `I井场 注采阀组工艺及自控流程图`） |
| 4 | `TITLE` | CHAR | 0 / 53 | 0 | 全 NULL |
| 5 | `REVISION` | CHAR | **0 / 53** | 0 | **全 NULL** |
| 6 | `VERSION` | CHAR | **0 / 53** | 0 | **全 NULL** |
| 12 | `DATECREATED` | DATE | 53 / 53 | 53 | `2026/05/09 16:12:06` .. `2026/07/12`（I井场为 `2026/06/17 23:05:20`） |
| 13 | `PATH` | CHAR | 53 / 53 | 53 | 植物库相对路径（I井场为 `\00\00\I井场 注采阀组工艺及自控流程图.pid`） |
| 15 | `DRAWINGNUMBER` | CHAR | 53 / 53 | 53 | 图号（I井场为 `DWG-0000集05-08`） |
| 16 | `TEMPLATE` | CHAR | 53 / 53 | 6 | `CPE-SQ-A0.pid` 等 6 种模板 |
| 17 | `LASTPUBLISHTIME` | DATE | **0 / 53** | 0 | **全 NULL** |
| 18 | `LASTARCHIVETIME` | DATE | **0 / 53** | 0 | **全 NULL** |
| 19 | `SP_LASTEFPUBLISHTIME` | DATE | **0 / 53** | 0 | **全 NULL** |
| 29–32 | `DESIGNPHASE` / `PROJECTNUMBER` / `DRAWINGDATE` / `DATEOFDRAWING` | CHAR / DATE | **0 / 53** | 0 | **全 NULL** |

- 53 行里没有任何一行带非空的 `VERSION`、`REVISION` 或 `LASTPUBLISHTIME` / `SP_LASTEFPUBLISHTIME`。

---

## 3. `T_DRAWINGVERSION`（15 行，12 列）与 `T_REVISION`（0 行）

### 3.1 `T_DRAWINGVERSION` 的 15 行全部是已删除图纸的留底快照

12 列中 `SP_PROJECTNAME`（`0/15`）与 `SP_REVISIONID`（`0/15`）全空；其余 10 列（`SP_ID`、`ACTOR`、`NAME`、`SP_DRAWINGID`、`DATECREATED`、`DRAWINGNUMBER`、`SP_ACCESSMODE`、`SP_COMMENTS`、`SP_STORAGE`、`SP_VERSIONPURPOSE`）全部 `15/15` 非空。

全部 15 行的 `SP_ACCESSMODE = 1`、`SP_VERSIONPURPOSE = 3`、`SP_COMMENTS = "Created for Delete"`，且 **`SP_DRAWINGID` 在现存 `T_DRAWING` 53 行中命中 `0 / 15`**：

| 行 | `SP_ID` | `SP_DRAWINGID` | `NAME` | `DRAWINGNUMBER` | `DATECREATED` | 在 `T_DRAWING` |
|---:|---|---|---|---|---|---|
| 0 | `29207301…` | `4304F559…` | `DWG-0000TP01-03 P井场注采阀组工艺及自控流程图` | `00003` | `2026/05/22 15:22:04` | 否 |
| 1 | `45E7E01C…` | `2DAFBD9B…` | `DWG-0000TP01-02 0井场注采阀组工艺及自控流程图` | `00002` | `2026/05/22 15:28:52` | 否 |
| 2 | `ECB9DA33…` | `CAA6018F…` | `D03` | `003` | `2026/05/13 16:29:12` | 否 |
| 3 | `F16BF54B…` | `302A0B8D…` | `集注站 烃水露点控制单元（二） 空冷器B集预冷分离器B工艺及自控流程图` | `DWG-0000集05-22` | `2026/06/23 17:37:44` | 否 |
| 4 | `8203087C…` | `8E730FC5…` | `集注站 烃水露点控制单元（三） 空冷器C集预冷分离器C工艺及自控流程图` | `DEG-0000集05-23` | `2026/06/23 17:37:46` | 否 |
| 5 | `30D51CB7…` | `80F2A2F0…` | `集注站 烃水露点控制单元（一） 空冷器A集预冷分离器A工艺及自控流程图` | `DWG-0000集05-21` | `2026/06/23 17:38:29` | 否 |
| 6 | `45E1EDB4…` | `A284369F…` | `集注站 低压气及轻烃处理单元 轻烃分离器二工艺及自控流程图` | `DWG-0000集05-51` | `2026/07/12 22:02:38` | 否 |
| 7 | `7901883C…` | `CAAAAD9A…` | `Copy of DWG-0002TP03-01` | `Copy of 001` | `2026/05/14 12:20:07` | 否 |
| 8 | `CB07E210…` | `2203D74A…` | `006` | `D06` | `2026/05/14 12:27:46` | 否 |
| 9 | `E6A79F84…` | `6DF98E56…` | `DWG-0002TP03-02` | `02002` | `2026/05/19 15:01:16` | 否 |
| 10 | `AAA49F87…` | `64124D9C…` | `DWG-0002TP03-02` | `02002` | `2026/05/19 15:10:56` | 否 |
| 11 | `CAF22080…` | `804C7573…` | `Copy of DWG-0002TP03-01` | `Copy of 02001` | `2026/05/19 17:58:47` | 否 |
| 12 | `2BC2C5AA…` | `E876A5B1…` | `I井场 注采阀组工艺及自控流程图` | `DWG-0000集05-08` | `2026/06/17 23:04:16` | 否（旧图删除留底，64 秒后新建现存 `5633AF41…`） |
| 13 | `871AFC18…` | `4D1BE887…` | `D01` | `001` | `2026/05/09 16:17:29` | 否 |
| 14 | `F68EF2CB…` | `2ABF2D1D…` | `D02` | `002` | `2026/05/09 16:17:42` | 否 |

### 3.2 `T_REVISION`（0 行，11 列）

列为 `SP_ID`、`DESCRIPTION`、`APPROVEDBY`、`SP_DRAWINGID`、`SP_CREATEDBY`、`SP_CHECKBY`、`SP_REVISIONDATE`、`SP_MAJORREVISIONNUMBER`、`SP_MINORREVISIONNUMBER`、`SP_SEQUENCEINGROUP`、`REVIEWBY`，0 行。

---

## 4. 5 份参考 `_Meta.xml` 的结构与 UID 构成

5 份参考 `_Meta.xml` 的结构完全固定：1 个 `<Container CompSchema="DocVersioningComponent" ... ToolID="SMARTPLANTPID" ToolSignature="AAAD">` 下含 3 个对象与 3 条 `<Rel>`：

| 块 | 固定属性 / 关系连接 | I井场参考值 | 4 张集注站参考值 |
|---|---|---|---|
| `DocumentVersion` | `IObject.Name = "{DocName} Version"`<br>`IDocumentVersion DocRevision="0" DocVersion="1"`<br>`<IFileComposition/>` | `UID="596C9B84A82C4C0000AF8FE79EF00EF"`（31 位）<br>`DocVersionDate="2026/06/22"` | 32 位十六进制 UID<br>`DocVersionDate="2026/07/06"` |
| `Rel (VersionedDoc)` | `UID1 = DocUID` → `UID2 = DocumentVersion.UID` | `UID="4C9F5894FCF1433AB4BF4E1F48D21B28"` | 32 位十六进制 UID |
| `DocumentRevision` | `IObject.Name = "{DocName} Revision"`<br>`IDocumentRevision MajorRev_ForRevise="0" MinorRev_ForRevise=""` | `UID="7B285A2D0AB64DD9B998926CF802F8E7"` | 32 位十六进制 UID |
| `Rel (RevisedDocument)` | `UID1 = DocumentRevision.UID` → `UID2 = DocUID` | `UID="70602B9820874930B512D75084B56295"` | 32 位十六进制 UID |
| `File` | `IObject.Name = "{DocName}.pid" Description=""`<br>`IFile FilePath="\\Glpid-7\pidsq\PID"` | `UID="AAAD5EA18AC7-67-44C5-AEAF33D4C22EB56"`（36 位） | `AAAD` + 带连字符短 GUID（37–39 位） |
| `Rel (FileComposition)` | `UID1 = File.UID` → `UID2 = DocumentVersion.UID` | `UID="AAAD703C37A-A805-4D01-9BC6AB10CBAC12F1"`（38 位） | `AAAD` + 带连字符短 GUID（39 位） |

### 4.1 UID 构成与格式化特征

1. **前 4 个 UID（`DocumentVersion`、`VersionedDoc`、`DocumentRevision`、`RevisedDocument`）**：
   - 不带连字符的大写十六进制 v4 随机 GUID（第 13 位全为 `4`）。
   - I井场的 `DocumentVersion.UID`（`596C9B84A82C4C0000AF8FE79EF00EF`）只有 **31 位**：适配器在格式化某个 `< 0x10` 的字节时掉了前导 `0`（`%X` 而非 `%02X`）。
2. **后 2 个 UID（`File`、`FileComposition`）**：
   - 前缀固定是 `Container.ToolSignature`（`AAAD`），后面拼带连字符的 v4 GUID：`AAAD{Data1}-{Data2}-{Data3}-{Data4}`。
   - 同样存在掉前导 `0` 的格式化：例如 I井场 `File.UID` 的 `Data2` 是 `67`（2 位，缺 2 个前导 `0`）且 `Data4` 是 15 位（缺 1 个前导 `0`），总长 36 位；分离器 B `File.UID` 为 `AAAD4202DB2-3D7-4B69-A562CA25A7FAD29B`（`Data1` 7 位、`Data2` 3 位，总长 37 位）。
3. **库与 `.pid` 全文检索结果**：
   - 将 5 张图共 30 个 UID（含去 `AAAD` 前缀、对缺位补齐 `0` 还原出的 32 位十六进制候选、ASCII / UTF-16BE / UTF-16LE、16 字节大端与 Windows 混合端原始字节）在 `Export.dmp` 的 154 个表单元格与 74.5 MB 原字节、以及 5 个 `.pid` 原字节中检索：**命中全部为 `0`**（对照组：各图 `DocUID` 在 `.pid` 与 `Export.dmp` 中均正常命中）。
   - 检索 `Glpid`、`pidsq`、`2026/06/22`：在 `Export.dmp` 与 `I井场.pid` 中同样 **全部为 `0`**。

---

## 5. 与当前实现（`4866b81`）的对照

提交 `4866b81` 已将 `write_pid_meta_xml` 改调 `write_meta_xml`：
- 已写出 `DocumentVersion`、`DocumentRevision`、`File` 及 `VersionedDoc`、`RevisedDocument`、`FileComposition` 三条关系，标签层级、`Name`、`DocRevision="0"`、`DocVersion="1"`、`MajorRev_ForRevise="0"`、`MinorRev_ForRevise=""` 与参考完全一致。
- 6 个 UID 由 `derive_meta_uid(drawing_uid, role)`（UUID v5，32 位大写十六进制）确定性生成。
- `DocVersionDate` 与 `FilePath` 当前输出为空串 `""`。
- **Q5–Q7 批复（2026-10-14）**：见 `docs/plans/2026-10-13-publish-progress-and-next-steps.md` §3.1，保持 `4866b81` 现行口径不变，N10 收口。

## 复现

```powershell
cargo run --locked --example probe_oracle_exp_drawing_meta
```
