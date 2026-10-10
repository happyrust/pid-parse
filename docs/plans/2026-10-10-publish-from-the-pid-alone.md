# 只凭 `.pid` 导出 publish（`_Data.xml` / `_Meta.xml`）· 开发计划（2026-10-10 开单，同日 Plannotator 批准）

> 用户 2026-10-10 06:46（zhimo 会话 opus-5-5-7）：「项目路径为D:\work\plant-code\cad\pid-parse 继续实现导出publish交服务的功能。这使得测试案例在项目文件夹D:\work\cad\PID自研软件验证」；
> 06:54：「先做只凭 .pid 的取证探针：5 张图逐张统计能复原参考 _Data.xml 的多少，写一页分析」。
> 2026-10-10（会话 opus-5-5-19 接手）：「使用plannator制定开发计划」。开单时只做计划，**没改任何代码**，只新建了本文件。
> **2026-10-10 08:36 Plannotator 批准**（`{"decision":"approved"}`，无批注）；用户随后：「已在 Plannotator 里批注 / 批准了，读结果接着做」。用户随后：「4 件事都按推荐项，直接开始 P0」——Q1–Q4 全选 (a)；G1 照旧在 P1 之后由用户看数字定。
> 基线：pid-parse `e0cbd0a`（`codex/phase32c-bundle-closeout`）。工作树上别的会话的 4 M + 1 ??（`docs/analysis/2026-09-20-…`、`docs/analysis/2026-09-21-parsing-pipeline-audit.md`、`docs/plans/2026-09-21-style-link-…`、`examples/probe_jflavorholder_…`、`examples/probe_text_anchor_by_kind.rs`）本单**一个字节不碰**。
> 未跟踪的 `examples/probe_publish_from_the_pid_alone.rs`（约 980 行，10-10 06:58 建）是上一会话按 06:54 的指示写的取证探针；记录里没有它的运行结果，本单 P0 接手。

## 一句话

`pid_publish_xml` 今天只会从工厂库（`Export.mdf` / Backup Store）导出，而 `PID自研软件验证` 的 5 张图里 4 张在手头的库（SQPlant，Oracle `exp`）里根本没有。
本计划先量清「只凭 `.pid` 能复原参考 `_Data.xml` 的多少」（P0–P1，只读，出一页分析），在门 **G1** 由你按数字定导出器做到哪一档；
再新增 `.pid` → `PublishDrawing` 装载器、复用现有 `xml_writer`，让 `pid_publish_xml --pid` 出 `_Data.xml` / `_Meta.xml`，用 `--diff-against` 对 5 份参考逐张打分并上棘轮（P2–P4）。
「交服务」交到哪（一个目录，还是某个服务接口）等你答 Q1，再做 P5。

## 1. 现状

**用例**：5 张图，Plant `P01`，参考由 `\\Glpid-7\pidsq\PID` 发布。按文件时间，参考 XML 都晚于 `.pid` 的最后一次保存（四张晚 16–18 分钟，I井场晚 2 小时），两者应是同一状态。

| 图 | `.pid` | 发布对象 | `<Rel>` | 在 SQPlant 备份里 |
|---|---:|---:|---:|---|
| 一级闪蒸分离器A | 1.1 MB | 570 | 767 | 否 |
| 一级闪蒸分离器B | 0.9 MB | 470 | 627 | 否 |
| 换热器CD及外输阀组 | 1.7 MB | 1 319 | 1 800 | 否 |
| 低温分离器A | 1.2 MB | 679 | 884 | 否 |
| I井场 注采阀组 | 4.0 MB | 3 093 | 4 107 | 是（DocUID 在 dump 里） |
| 合计 | | **6 131** | **8 185** | |

- 类构成：`PIDRepresentation` 1 823（30 %）、`PIDSignalPort` 1 725（28 %）、`PIDPipingPort` 718（12 %），其余是 `PIDProcessPoint` / `PIDPipingConnector` / `PIDPipeline` / `PIDPipingComponent` / `PIDPipingBranchPoint` / `PIDInstrument` 等。**七成对象是表示和端口。**
- 5 份参考都不带 `ItemTag`，是 DWG 口味（`IObject` 用 `Name`），对应 `--style dwg`。
- `_Meta.xml` 每份约 1.8 KB：`Container` 头（DocUID、DocName、Plant、SoftwareVersion、SchemaVersion、ToolID…）、`DocumentVersion` / `DocumentRevision` / `File` 三个对象和三条 `Rel`。

**管线**：`PublishDrawing` 照库表的形状（`T_Drawing` / `T_ModelItem` / `T_Representation` / `T_Relationship`，加代码表和 A01 / DWG 口味）；`xml_writer` 按类写接口；`src/publish/diff.rs` 已有 `diff_publish_xml` / `coverage_against_reference` / `diff_rel_defuids`；
CLI 是 `pid_publish_xml <mdf> --drawing UID --out … --meta-out … --diff-against … --style …`。**换一个装载器，写出和比对都能复用。**

**已有证据**：
- 上一会话粗查一级闪蒸分离器A：参考里 338 个 UID 有 298 个以 ASCII 出现在 `.pid`（`PSMcluster0` 246、`Unclustered Dynamic Attributes` 64）；标签的 SmartText 里存着 `<F>TagSequenceNo=0112101A</F>` 这种「属性=值」；
  但 110 个名称 / 位号只搜到 27 个，管线号这类拼接串基本没有，`ConstructionStatus` 等属性的来源未查。口径记录里没写，P0 用探针重新量。
- 09-30 `docs/analysis/2026-09-30-item-tags-without-data-xml.md`：凭图形 → 文字的三条关联复原位号 0 / 16，S 登记不做。那页量的是**关联**，这次量的是**属性集与标签缓存**，路线不同。
- 10-08 D1 / D2（`docs/analysis/2026-10-08-a01-representation-uid-is-the-join.md`）：表示 UID → 顶层 `FreeFormAttrSet 0x0089` → 空间映射标签 190 → 记录，A01 3 / 4、0202 39 / 39，`PidSemanticIndex::load_beside` 已用上。表示的 `GraphicOID` 可以靠它给。

## 2. 范围

**做**：P0 探针接手并跑通；P1 一页分析；G1 门；G1 选做时 P2 装载器、P3 CLI 与比对、P4 测试与全量门；P5 交付（等 Q1）。
**不做**：Backup Store 第二版（Oracle `exp` 行解码，只覆盖 I井场，见 Q3）；往 `.pid` 写回；语料以外的图；SmartPlant 自己那一侧的 publish 协议。

## 3. 要你拍板的

| # | 问题 | 选项 | 推荐 |
|---|---|---|---|
| Q1 | 「交服务」指什么 | (a) 把 `_Data.xml` / `_Meta.xml` 导出到一个目录交出去；(b) 提交给某个服务（请给服务名和接口：HTTP / 共享目录 / SPF …）；(c) 先不定，P5 前再说 | 先按 **(a)** 做；接口一给，P5 加一层薄适配 |
| Q2 | G1 之后导出器做到哪一档 | (a) 只发在 `.pid` 里**找得到出处**的对象 / 属性 / 关系，缺的不发、不猜，另出覆盖报告；(b) 再加**按规则推出**的（如端口、拼接的管线号），每条规则先要有对照证据；(c) 覆盖低于门槛（比如对象 UID 或关系两端 < 80 %）就不做导出器，改走库路 | **(a)**，(b) 逐条另开小单；门槛数字看 P1 再定 |
| Q3 | 库路（Backup Store 第二版）要不要并行 | (a) 不并行，G1 后再看；(b) 另开会话并行，给 I井场一份从库导出的精确对照 | **(a)**：它只覆盖 1 / 5，也不影响 P0–P1 |
| Q4 | 代码放哪、CLI 什么样 | (a) `src/publish/pid_load.rs`，随 `publish` 留在 `backup` 特性下；`pid_publish_xml` 加 `--pid <file.pid>` 模式，与 `<mdf>` 互斥；(b) 另起二进制，并把 `xml_writer` 从 `backup` 拆出来（wasm 可用） | **(a)**：改动最小，`--diff-against` 直接复用；网页版要用时再拆 |

## 4. 步骤

### P0 探针接手（只读，`src` 不动）

- 读通 `examples/probe_publish_from_the_pid_alone.rs` 模块注释里写死的四种读法，**不改口径**：
  - **Raw**：值的 UTF-8 / UTF-16LE / GBK 字节出现在某条流里（上界，说不出属于哪个对象）；
  - **Fields**：逐流扫 `u16 tag, u16 len` 框起的属性集字段（`0x8010` 字符串、`0x0003` 整数、`0x000B` 布尔、`0x0000` 集头），外加 `ItemValue` 与 `<F>name=value</F>` 两种标签缓存；
  - **Named**：`P&IDAttributes` 的 `DrawingNo` / `DrawingID` / `ModelID` 指名某个 UID；
  - **GraphicOID**：10-08 D2 的 UID 连。
- 只补两处输出：① `_Meta.xml` 也打分（DocUID、DocName、三个对象和三条 `Rel` 的 UID 在不在 `.pid` 里）；② 属性表再按「类 × 属性」出一张（今天只按 `interface.attribute` 合计），P2 要按类实现。
- 默认特性与 `--no-default-features` 各 build 一次（探针不用 `backup` 那一半），clippy `-D warnings` 两种特性都过。
- 5 张图逐张跑，整段输出留作分析页附录。

### P1 一页分析 `docs/analysis/2026-10-10-publish-from-the-pid-alone.md`

- 逐图 + 合计四张表：对象（按类：被指名 / UID 原样在）；属性（按类 × 属性：标签缓存 / 字段 / 仅原始字节 / 没有）；关系（按 `DefUID`：两端都被指名 / 一端）；表示（UID 连上 / 落在发布的 `GraphicOID`）；外加 `_Meta.xml` 一行。
- 结论分三档，每档列出类和属性：**能精确给**（值在某个字段里、对象能定位）／**可能按规则推**（只列候选和缺的证据，本单不定）／**`.pid` 里没有**（只在库里）。
- 写明与 09-30 那页的关系；`CHANGELOG.md` 与 `task_plan.md` 各加一段；探针 + 分析页一个提交，只 `git add` 这四个路径（`CHANGELOG.md` / `task_plan.md` 若当时有别人未提交的改动，用 `git add -p` 只挑自己的段）。

### G1 门（你拍板）

看 P1 的三档数字回答 Q2。选 (c) 就到此为止，库路另开计划。

### P2 `.pid` → `PublishDrawing` 装载器（G1 选做才开工）

- 把探针里的属性集扫描升格为 `src/parsers/` 下的正式解码器：每个值带出处（流、偏移）；合成字节单测覆盖规范输入、每一条拒绝和不 panic；登记进 `tests/parser_panic_safety.rs`。
- `src/publish/pid_load.rs` 新增 `load_drawing_from_pid(path) -> PublishDrawing`：
  - drawing：DocUID / 名称取自 `P&IDAttributes`（`DrawingID` / `DrawingNo`，以 P1 实测为准）；
  - objects：`ModelID` + `ModelItemType` → `uid` / `item_type_name`，属性取同一属性集与标签缓存；
  - representations：表示 UID 与 `GraphicOID` 走 `PidSemanticIndex` 的 UID 连；
  - relationships：按 P1 里 `<Rel>` 两端的实测出处给，第一档没有的不造。
- 代码表文本今天来自库；`.pid` 里若只有索引值，按 Q2 (a) 不发对应的描述属性，登记进覆盖报告。

### P3 CLI 与比对

- `pid_publish_xml --pid <file.pid> [--out F] [--meta-out F] [--diff-against REF] [--style a01|dwg]`，与 `<mdf>` 模式互斥；`--diff-against` 复用 `diff_publish_xml` / `coverage_against_reference`。
- 5 张图逐张跑：发出的每个对象、属性、关系都要和参考**逐值相同**——错一个是缺陷，不算覆盖率问题；覆盖数写进分析页「导出器实测」一节。

### P4 测试与全量门

- `tests/publish_from_pid.rs`：5 张图（目录不在就 soft-skip，沿用探针的 `PID_PARSE_SELF_CHECK_DIR`），钉每图「发出数 / 参考数 / 错值 0」，覆盖只许升。
- 全量：`cargo build` / `cargo test --locked --workspace --all-targets`，clippy `-D warnings`（默认与 `--no-default-features`），`cargo fmt --all -- --check`，missing-docs 棘轮，wasm32 lib 检查。

### P5 交付（等 Q1）

- Q1 选 (a)：`pid_publish_xml --pid … --out-dir DIR`，按 `<DocName>_Data.xml` / `<DocName>_Meta.xml` 命名，旁边放一份覆盖报告（JSON）。
- Q1 选 (b)：接口定了再加适配层，另开小单。

## 5. 验收

- P0–P1：`cargo run --example probe_publish_from_the_pid_alone` 在 5 张图上复现分析页里的每个数；分析页给出三档结论。
- P2–P4：5 张图都出合法的 `_Data.xml` / `_Meta.xml`；`--diff-against` 下**错值 0**；覆盖数进棘轮；既有测试与 golden 不变。

## 6. 风险

- **覆盖有天花板**：发布时 SmartPlant 从库里取的值（代码表文本、拼接的管线号、`ConstructionStatus` 一类）`.pid` 里可能根本没有。G1 就是为这个设的。
- **端口与表示占七成**：它们若是发布时按符号定义现生成的，UID 不在 `.pid`，就对不上参考的 UID，只能按结构比或认缺；P1 给实数。
- **参考与 `.pid` 是否同一状态**：按文件时间是，但要留意 A01 那种「发布后又保存过」的情形（`GraphicOID` 大面积落空就是信号）。
- **共享工作树**：别的会话正在改 `style_link` 等；本单只碰新文件、`src/publish/`、`src/bin/pid_publish_xml.rs` 和 `CHANGELOG.md` / `task_plan.md` 的追加段，提交只 `git add` 明确路径；cargo 构建目录可能被别的会话占着锁，等就是了。

## 7. 执行记录

### P0（2026-10-10，会话 opus-5-5-19）

- 接手上一会话的探针，四种读法不动；加 `_Meta.xml` 逐项定位和「类 × 属性」表；修了 `_Meta.xml` 定位的一处误报（有些 `ModelID` 字段存 `0`，短值不再当名字查）。
- 默认特性与 `--no-default-features` 下 clippy `-D warnings` 零告警，rustfmt 干净；debug 构建跑 5 张图约 40 s，输出 524 行。

### P1（2026-10-10）

- 分析页 `docs/analysis/2026-10-10-publish-from-the-pid-alone.md`：对象 UID 2 948 / 6 131（48.1 %，6 类 3 155 个整类不在），表示 1 823 / 1 823，关系两端都在 3 491 / 8 185 而关系自己的 UID 0 条，属性值在标签缓存 1 296 / 11 671（11.1 %）；三档结论见该页。
- 探针、分析页、本计划、`CHANGELOG.md`、`task_plan.md` 一个提交（`054fc4e`）。

### G1（2026-10-10，用户拍板）

- 「G1：做 P2，下游只要设备 / 仪表 / 管件清单和图上的表示」。P2 的验收按这个范围收窄：设备、仪表、管件及其表示、图纸，其余有出处的对象照发但不作验收。

### P2 第一步：归属（2026-10-10）

- 探针 `examples/probe_label_values_reach_their_item.rs`，分析 `docs/analysis/2026-10-10-a-label-value-reaches-its-item-through-its-text-box.md`：值行 `+12` → `igTextBox` `+4` → `DependencyObject` `+22` → 被标注物项的 `P&IDAttributes` 行（`+16` 图形交叉核），3 954 / 3 978 挂上、两端打架 0、对照属性错挂 0；归属后能给的发布值 822 个。
- 仪表一对三：有 `SymbolAttr` 的 219 / 287 按 `Type` / `Class` 一一对应，68 个没有。
- 名称规则（仪表 236 / 236、设备 13 / 13，只在参考 XML 上核）要用户拍板；顺手更正 P1 页：端口 / 工艺点 / 分支点 2 983 个的 UID 是父 UID + 后缀。
- 下一项：用户定名称规则与公称直径换算用不用，再写 `src/publish/pid_load.rs`。
