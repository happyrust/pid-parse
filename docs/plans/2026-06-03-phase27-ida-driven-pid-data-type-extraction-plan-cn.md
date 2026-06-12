# Phase 27：IDA 证据驱动的 PID 全数据类型提取计划

> 日期：2026-06-03  
> 目标：结合 `ida-pro-mcp` 与 `pid-parse` 现有 parser，把 `.pid` 文件中的所有可识别数据类型整理成可验证矩阵，并逐步恢复每类数据的读取函数、字段布局、parser DTO、模型映射与测试门禁。  
> 当前默认工作目录：`D:/work/plant-code/cad/pid-parse`

## 0. 起点事实

### 0.1 当前可用 IDA 实例

`ida-pro-mcp list_instances` 当前可见：

| Port | Binary | 状态 | 备注 |
|---:|---|---|---|
| 13337 | `core.dll` | reachable | AVEVA E3D core，可能与 SPPID 部分无关 |
| 13338 | `radsrvitem.dll` | reachable / selected | 当前 Phase 27 主要入口 |

`radsrvitem.dll` survey：

- 32-bit。
- Base：`0x56440000`。
- Functions：5374，总命名 346，未命名 4867。
- Strings：1739。
- Exports：
  - `GetServerItemTransceiver` at `0x56448040`
  - `GetServerItemVersion` at `0x564480d0`
- 关键字符串：`igTextBox`、`igLine2d`、`igSmartFrame2d`、`CLSID`、`XCeedRAD.dll`。

### 0.2 已恢复的首批 IDA 证据

#### Type-code → type-name 映射函数

`sub_56448F70(_WORD *a1)` 已反编译为按 `u16` type code 返回字符串名称的 jump table。

已确认映射样例：

| Type code | Name |
|---:|---|
| `0x0018` | `igLine2d` |
| `0x0020` | `igRectangle2d` |
| `0x004D` | `igTextBox` |
| `0x0059` | `igCircle2d` |
| `0x005E` | `igPoint2d` |
| `0x0061` | `igArc2d` |
| `0x007E` | `igEllipticalArc2d` |
| `0x0084` | `igLineString2d` |
| `0x00CE` | 当前反编译截断前已知是 jump table case，需继续完整导出 |
| `0x0115` | `igDimension` |
| `0x0117` | `igBalloon` |
| `0x0118` | `igLeader` |

该函数是 Phase 27 的核心事实源之一：它可以把现有 `Sheet*` PSM type code 与 SmartPlant / IGDS 类型名对齐。

#### `igTextBox` 提取候选函数

`sub_564468B0(_DWORD *this, _WORD *a2, int a3, int *a4)`：

- 入口检查 `*a2 == 77`，即 `0x004D = igTextBox`。
- 根据 `a2[12] == 1/2/3` 走不同文本 payload 获取路径：
  - `sub_56449240(a2)`
  - `sub_56447710(a2)`
  - `sub_56447730(a2)`
- 提取 UTF-16LE 文本长度与 payload。
- 调用 `sub_564459D0(..., "igTextBox", ...)` 查找/创建目标对象。
- 将提取到的文本写入 `"TEXT"` 属性。

这说明当前 parser 的 `decode_igtextboxes` 可以进一步用 IDA 证据校准字段布局与文本路径分支。

## 1. Grill-Me 决策树

### Q1：Phase 27 是继续写说明，还是开始实现 parser？

**推荐答案：先做 IDA-backed 数据类型矩阵，再按矩阵逐类实现。**

原因：目标是“最终把所有 PID 数据类型都能提取出来”。如果直接写某个 parser，会再次局部推进，无法证明“所有类型”的覆盖范围。

### Q2：什么叫“所有 PID 数据类型”？

**推荐答案：分四层定义，不强行一次性全实现。**

1. **容器/元数据类型**：CFB、Summary、TaggedTxtData、DocVersion、AppObject、JTaggedTxtStgList。
2. **索引/对象类型**：PSMroots、PSMclustertable、PSMsegmenttable、JSite、Dynamic Attributes、relationship endpoint。
3. **Sheet PSM/IGDS record 类型**：`igLine2d`、`igTextBox`、`igPoint2d`、`igSymbol2d`、SmartPlant 扩展 `GLine2d`、`JStyleOverride`、`GraphicGroup`、`0x0010` 等。
4. **未知/未命名类型**：PSMspacemap、未覆盖 type code、audit-only payload。

Phase 27 的 Done 不等于全部 parser 完成，而是建立权威矩阵与优先级；后续每类数据以独立 slice 落地。

### Q3：以 IDA 为准，还是以 fixture byte 为准？

**推荐答案：必须双证据闭环。**

- IDA 给出读写函数、字段顺序、类型名、CLSID / class identity。
- fixture byte / existing parser 给出真实 `.pid` 里出现频率、size bucket、边界和异常样本。
- 只有两者一致时才能从 `Probed` / `AuditOnly` 升级为 `Decoded`。

### Q4：当前只有 `radsrvitem.dll` 和 `core.dll` 两个 IDA 实例够不够？

**推荐答案：够启动 Phase 27-A/B，不够完成全部类型。**

已可立即做：

- 从 `radsrvitem.dll` 导出 type-code → type-name 映射。
- 从 type name 字符串反查 xrefs，定位 per-type extraction/serialization 函数。
- 先闭环 `igTextBox`、`igLine2d`、`igLineString2d`、`igPoint2d` 等标准 IGDS 类型。

后续可能需要用户再打开：

- `style.dll`
- `J2DSrv.dll`
- `sppid.dll`
- `XCeedRAD.dll`
- `smartplantpid.exe`

特别是 `0x0010` polymorphic family 与 SmartPlant 扩展类型，大概率需要跨 DLL。

### Q5：是否允许命名 `0x0010 sub_kind`？

**推荐答案：暂不允许，直到 IDA 找到真实 discriminator。**

现有 `leading_word` 仍只是 byte-position audit 字段。Phase 27 可以寻找 discriminator，但不能提前把它写成 sub-kind。

## 2. 数据类型矩阵交付物

新增：

`docs/analysis/2026-06-03-phase27-pid-data-type-matrix-cn.md`

每个类型用统一模板：

```text
Type code / stream name：
SmartPlant / IGDS type name：
来源层级：CFB / metadata / PSM table / DA / Sheet / unknown
当前 parser：
当前模型 DTO：
IDA 证据：
fixture 证据：
字段布局：
confidence：Decoded / PartiallyDecoded / AuditOnly / Probed / Unknown
缺口：
实现 slice：
```

## 3. 执行阶段

### Phase 27-A：IDA 类型码总表恢复

- [ ] 用 `sub_56448F70` 完整导出 type-code → type-name 映射。
- [ ] 用 `py_eval` 或 `analyze_function(include_asm=true)` 补齐反编译截断后的全部 case。
- [ ] 将现有 parser type code 与 IDA 名称对齐：
  - `0x0018` → `igLine2d`
  - `0x004D` → `igTextBox`
  - `0x005E` → `igPoint2d`
  - `0x0084` → `igLineString2d`
  - `0x00CE` → `igSymbol2d`（需 IDA 表确认完整 case）
  - `0x0030` → 当前 `JStyleOverride`，需与 radsrv/style 证据保持一致
  - `0x3FE6` → SmartPlant `GLine2d` 扩展，可能不在 IGDS jump table

Done 条件：

- `docs/analysis/2026-06-03-phase27-pid-data-type-matrix-cn.md` 初版包含完整 type-code 表。

### Phase 27-B：per-type IDA 读取函数定位

- [ ] 对每个 type name 字符串跑 `search_text` / `xrefs_to`。
- [ ] 建立 `type name → xref function → caller → extraction/serialization candidate` 链。
- [ ] 先用 `igTextBox` 作为样板，确认 `sub_564468B0` 的字段提取路径。
- [ ] 扩展到 `igLine2d`、`igLineString2d`、`igPoint2d`、`igSymbol2d`。

Done 条件：

- 每个已 decoded parser 至少有一个 IDA function candidate。
- 对没有 candidate 的类型，写明缺失原因。

### Phase 27-C：字段布局对照

- [ ] 对每个 candidate function 反编译，提取字段偏移、数据类型、分支条件。
- [ ] 与 `src/parsers/sheet_records.rs` 的当前 DTO 对照。
- [ ] 标记三类结果：
  - `match`：现有 parser 与 IDA 一致。
  - `mismatch`：字段解释或偏移不一致。
  - `missing`：IDA 有字段但 parser 未提取。

Done 条件：

- 形成 `IDA layout ↔ parser DTO` 对照表。

### Phase 27-D：实现优先级与 parser 切片

- [ ] 按价值排序：
  1. mismatch 修复。
  2. missing 字段补齐。
  3. audit-only → decoded promotion 候选。
  4. unknown type 新 parser。
- [ ] 每个实现切片必须走七层 decoder 模板：
  1. Probe。
  2. Layout discovery。
  3. Decoder API。
  4. Validation rules。
  5. Unit tests。
  6. Model DTO。
  7. Pipeline + normalized geometry/import view 映射。

Done 条件：

- 产出后续 Phase 28+ 的实现 backlog。

### Phase 27-E：`0x0010` / GraphicGroup / PSMspacemap 深水区

- [ ] 复用 Phase 20 IDA-first 路线，继续追 `0x0010` RAD class identity。
- [ ] 尝试从 `GraphicGroup` 相关引用找 child/reference payload 的真实语义。
- [ ] 识别 `PSMspacemap` 的 `tseg` record 结构入口。

Done 条件：

- 每个深水区至少给出一个“继续 / negative closeout / 需要新 IDA instance”的结论。

## 4. Stop-And-Challenge

任一触发必须暂停：

1. 试图只凭 IDA 伪代码实现 parser，但没有 fixture byte 验证。
2. 试图只凭 fixture pattern 命名字段，但没有 IDA / controlled diff / 多 fixture 证据。
3. 试图把 `0x0010.leading_word` 改名为 `sub_kind`。
4. 需要分析的 DLL 当前没有打开 IDA 实例。
5. 某个 type 的 IDA function 和现有 parser 明显冲突，且无法判断哪边正确。

## 5. 推荐下一步

立即执行 Phase 27-A：

1. 从 `sub_56448F70` 导出完整 type-code → type-name 表。
2. 写入 `docs/analysis/2026-06-03-phase27-pid-data-type-matrix-cn.md`。
3. 以 `igTextBox` 作为样板进入 Phase 27-B/C。
