# P&ID 文字保真收口：先量对齐，再接字体

> 日期：2026-08-13
> 状态：**待批注**。本文是计划，不是结论。
> 关联仓：`pid-parse`（解码）+ `OpenCADStudio`（消费/渲染）

## 为什么是这两件

文字这条线上，插入点、字高、旋转、颜色都已接线并有 ratchet 覆盖。**记录级的解码内容
基本见底**——剩余 refusal 13 条里 12 条是判定正确的退化折线。所以下一步的价值不在
「多解出东西」，而在「已经画出来的东西摆得对不对、长得对不对」。

两个候选：

| | 内容 | 证据状态 | 对图面的影响 |
|---|---|---|---|
| **F** | 文字对齐 / justification | **完全未取证** | 位置错 → 标签整体偏移半个词宽 |
| **A** | 字体名 | **native-reader 已备齐** | 字形错 → 但位置对 |

**先做 F 的取证，再决定顺序。** 理由：位置错比字形错难看得多——一个字体不对但位置
对的标签仍然可读、可信；一个偏移半个词宽的标签会压到管线上。而 F 现在**一点证据都
没有**，可能是真问题，也可能根本不存在（全都左对齐）。在没量之前就投入 A，等于用
"手头有证据"代替"哪个更重要"来排序。

这条纪律在本项目已经付过两次学费：A01 那 18 条我用错偏移去看、断言"背后没有字"，
后来被自己推翻；旋转角我分别量两个槽位、判"都不是角度"，漏了它们是一个向量。
**先确认自己在正确的位置上看，再下结论。**

## 第 1 步 · F 的取证（决定后续顺序）

`igTextBox` 通过 `index` 命名一条 `JStyleTextPara`（段落样式），我们**只读了它的
`+38`（字符样式指针），其余字段一概没读**。对齐是段落属性，如果它存在，最可能在这里。

用刚刚在 `JStyleTextChar` 上验证成功的手法，而不是从语料统计猜：

1. `style.dll` 里有 `JStyleTextPara::IJStyleTextParaImp` 字符串（`0x100a20a4`）与
   对应 vtable。**从接口的 get/put 访问器反查对象槽位**，再找哪个 DoIO 调用者喂这些
   槽位——上一轮正是这样从"反推失败"翻盘找到 `sub_10030A20` 的。
2. 拿到 `JStyleTextPara` 的序列化读序后，**整条记录的字段一次性全部到手**，对齐在不
   在里面立刻有答案，不需要逐个字段猜。
3. 配一个 probe 把该字段在全语料的取值分布量出来。

**分支判据：**
- 若读序里有对齐字段**且语料取值有变化**（不是恒为左对齐）→ **F 优先于 A**，因为位置
  错影响更大。接线时 `acadrust::Text` 已有 `horizontal_alignment` / `vertical_alignment`
  / `alignment_point` 三个字段可用，OCS 目前一个都没设。
- 若无对齐字段，或取值恒定 → **F 就此登记为阴性结论收口**，转做 A。
- 顺带：这一趟会把 `+20` style-tail tag（待办 C）和形状 3 的 A/B doubles（待办 B）
  的线索一并带出来，它们同属"读序能一次交代完"的范畴。

时间盒：与上一轮同级。取证无果则登记 Coverage Gap，不无限投入。

## 第 2 步 · A 字体名接线（F 让路或 F 做完之后）

证据已是 native-reader（`payload +68` u16 长度 + `+70` UTF-16 正文，`payload == 70 + 2*count`
全语料 381/381），**不需要再取证，是纯工程**。

### 已核实的落地事实

- `acadrust::tables::TextStyle` 字段：`name`、`height`（**0 = 可变**）、`width_factor`、
  `oblique_angle`、`font_file`（SHX）、`big_font_file`（亚洲）、`true_type_font`（TTF 家族名）。
- OCS 的 `src/entities/text_support.rs:23` **优先取 `true_type_font`，为空才回退 `font_file`**。
- OCS 有 `scene::text::sysfont` 枚举系统已装字体，按名匹配。
- `acadrust::Text` 有 `style: String` 按名引用 TextStyle。

### 方案

- **每个不同字体名建一条 TextStyle**，池化，沿用本导入器已有的 `PID-DASH-<n>` 先例
  （`register_dash_linetypes`）。命名用 **`PID-<字体名>`** 而不是 `PID-FONT-<n>`：字体名
  本身有意义，`PID-Arial` 比 `PID-FONT-3` 可读，且与 `PID-` 前缀的图层约定一致，不会撞
  图纸自带样式。需要对名字做符号表合法化（去非法字符、截断、重名加序号）。
- **`true_type_font` 填 SmartPlant 报的名字**，因为 OCS 优先认它并去系统字体里找。
  `font_file` 留空。
- **`height` 必须填 0（可变）**。字高已经逐实体设好，样式里再填一个非零高度会与之
  冲突。这是最容易踩的坑。
- **未解析/缺失的字体名保留现状**（用文档默认样式），与字高、颜色的兜底口径一致。

### 12 条乱码字体名怎么办

`宋体` 的 GB2312 字节（`CB CE CC E5`）被厂商逐字节拓宽成 UTF-16，读作 `ËÎÌå`。
**建议原样保留，不做 GB2312 还原。** 理由：还原是猜测，而本项目的既定纪律是"拒收
而非猜测"；保留原样会让字体匹配失败并回退到默认字体，效果与还原失败一致，但不会
在文档里写入一个我们编出来的名字。检测特征（全部字符落在 U+0080..U+00FF）写进注释
备查即可。**这一条我不确定，是最想听你意见的地方。**

### 验收

- pid-parse：`style_link` 把字体名带上现有两跳链（与颜色同法，**不新建索引表**）；
  ratchet 增字体名分布断言。
- OCS：`tests/pid_import.rs` 断言 TextStyle 表里有预期条目、且 Text 实体按名引用；
  `pid_probe` 实测字体分布。
- 两仓全套 + clippy。

## 登记不做（本轮）

- D：12 条退化折线（正确拒收）、1 条未归因 `DependencyObject`、1 条 `0x0020` Rectangle
  （语料仅 1 条，无 fixture 支撑写解码器）。
- E：三个扫描型家族改链式认领——结构性改进，屏幕上无变化，要动 7100 行核心。
- OCS 侧 `CONTEXT.md`（多次登记未做，术语目前散在 analysis 文档里够用）。

## 未决事项（需要你拍板）

1. **F 与 A 的顺序**——本计划主张先量 F 再定，但如果你认为字体更急，可以直接做 A。
2. **乱码字体名**的处置（保留 / 还原 / 回退默认）。
3. **工作树两处旧内容**仍未处理：`pid-parse` 的 `src/parsers/sheet_records.rs`（会撤销
   你对冗余测试的合并）与 OCS 的 `docs/plans/2026-08-12-*.md`（含已不成立的进度说法）。
   建议 `git checkout` 采纳 HEAD，等你点头。
4. **三批未提交改动**：文字颜色接线、角度单位修复、本轮原生取证与文档。

## 附：本计划未能用上 Oracle

按要求尝试了 oracle MCP 求第二意见，两个引擎都被环境挡住：API 缺 `OPENAI_API_KEY`；
浏览器模式需要先在 Oracle 私有 Chrome 配置里手动登录一次 ChatGPT。一次性设置：

```
oracle --engine browser --browser-manual-login --browser-keep-browser \
  --browser-manual-login-profile-dir "C:\Users\dpc\.oracle\browser-profile" -p "HI"
```

登录后重跑即可。上面的优先级判断与设计方案是我自己的分析，**没有经过第二意见校验**。
