# Phase 27 PID 数据类型矩阵（IDA 证据初版）

> 日期：2026-06-03  
> 证据源：`radsrvitem.dll` via `ida-pro-mcp`，当前 IDA instance `127.0.0.1:13338`。  
> 目标：把 `.pid` 中可识别的数据类型按 `type code / stream name → 类型名 → 当前 parser → IDA 证据 → 缺口` 建立矩阵，作为后续逐类补齐 parser 的权威入口。

## 1. 范围声明

本文是 Phase 27-A 的初版矩阵，当前重点是 **Sheet / PSM record type code**。

容器级和 stream 级格式（CFB、SummaryInformation、TaggedTxtData、DocVersion、AppObject、PSMroots、Dynamic Attributes 等）已在 `docs/analysis/2026-06-03-pid-file-format-analysis-cn.md` 中按证据等级整理；后续 Phase 27 会把这些 stream 也纳入统一矩阵，但本文件先落地 IDA 可直接确认的 PSM/IGDS 图元类型。

## 2. IDA 起点证据

### 2.1 Binary

| 字段 | 值 |
|---|---|
| Binary | `radsrvitem.dll` |
| IDB | `D:/work/plant-code/cad/pid-parse/dlls/radsrvitem.dll.i64` |
| Arch | 32-bit |
| Base | `0x56440000` |
| Functions | 5374 |
| Strings | 1739 |
| Exports | `GetServerItemTransceiver`, `GetServerItemVersion` |

### 2.2 Type-code mapper

`sub_56448F70(_WORD *a1)`：

- 读取 `*(u16*)a1` 作为 type code。
- 对 `<= 0x0115` 的 code 走 switch jump table。
- 对 `0x0115 / 0x0117 / 0x0118` 走 if/else 返回。
- 默认返回 `byte_5665EC9C` 指向的空/默认字符串。

该函数是当前最直接的 **PSM/IGDS type code → SmartPlant/IGDS type name** 证据。

### 2.3 `igTextBox` 样板提取路径

`sub_564468B0(_DWORD *this, _WORD *a2, int a3, int *a4)`：

- 入口检查 `*a2 == 77`，即 `0x004D = igTextBox`。
- 根据 `a2[12] == 1 / 2 / 3` 选择不同 payload reader：
  - `sub_56449240(a2)`
  - `sub_56447710(a2)`
  - `sub_56447730(a2)`
- 从 payload 中读取 UTF-16LE 文本长度和文本内容。
- 调用 `sub_564459D0(..., "igTextBox", ...)`。
- 将转换后的文本写入 `"TEXT"` 属性。

这条链路可作为后续 Phase 27-B/C 的样板：先找字符串 xref，再追读取函数，再与 `decode_igtextbox_at` 字段布局对照。

## 3. IDA-confirmed PSM/IGDS Type Code 表

| Type code | IDA type name | 当前 parser 状态 | 当前 parser / DTO | Phase 27 缺口 |
|---:|---|---|---|---|
| `0x0006` | `igPointOnRelation2d` | 未覆盖 | - | 定位 per-type reader；判断是否只作为关系约束出现 |
| `0x000F` | `igParallelRelation2d` | 未覆盖 | - | 同上 |
| `0x0013` | `igBoundary2d` | 未覆盖 | - | 同上 |
| `0x0015` | `igPerpendicularRelation2d` | 未覆盖 | - | 同上 |
| `0x0017` | `_TangentRelation2d` | 未覆盖 | - | 同上；注意名称带前导 `_` |
| `0x0018` | `igLine2d` | Decoded | `decode_iglines` / `SheetIgLine2dDecoded` | 用 IDA reader 校准字段偏移与 flags |
| `0x0019` | `igKeyPointRelation2d` | 未覆盖 | - | 定位关系约束 reader |
| `0x0020` | `igRectangle2d` | 未覆盖 | - | 定位图元 reader；评估是否需要 typed decoder |
| `0x0021` | `igComplexString2d` | 未覆盖 | - | 可能与 text / rich string 相关，需优先 xref |
| `0x003D` | `igSmartFrame2d` | 未覆盖 | - | 与 SmartFrame 语义相关，需 xref |
| `0x0040` | `igConcentricRelation2d` | 未覆盖 | - | 定位关系约束 reader |
| `0x004D` | `igTextBox` | Decoded | `decode_igtextboxes` / `SheetIgTextBoxDecoded` | 已有 IDA 样板函数 `sub_564468B0`；下一步做字段对照 |
| `0x0059` | `igCircle2d` | 未覆盖 | - | 高价值几何图元；需定位 reader |
| `0x005D` | `igBSplineCurve2d` | 未覆盖 | - | 高价值几何图元；需定位 reader |
| `0x005E` | `igPoint2d` | Decoded | `decode_igpoints` / `SheetIgPoint2dDecoded` | 用 IDA reader 校准字段偏移 |
| `0x0061` | `igArc2d` | 未覆盖 | - | 注意不要与已否定的 `0x0030` 旧 PrimitiveArc 混淆 |
| `0x0063` | `igEllipse2d` | 未覆盖 | - | 高价值几何图元；需定位 reader |
| `0x0069` | `igSymmetricRelation2d` | 未覆盖 | - | 定位关系约束 reader |
| `0x006A` | `igEqualRelation2d` | 未覆盖 | - | 定位关系约束 reader |
| `0x006B` | `igColinearRelation2d` | 未覆盖 | - | 定位关系约束 reader |
| `0x0077` | `igFixRelation2d` | 未覆盖 | - | 定位关系约束 reader |
| `0x007B` | `igGroup` | 未覆盖 | - | 与 `GraphicGroup` / grouping 语义区分 |
| `0x007E` | `igEllipticalArc2d` | 未覆盖 | - | 高价值几何图元；需定位 reader |
| `0x0082` | `igHorizontalRelation2d` | 未覆盖 | - | 定位关系约束 reader |
| `0x0084` | `igLineString2d` | Decoded | `decode_iglinestrings` / `SheetIgLineString2dDecoded` | 用 IDA reader 校准点列与 flags |
| `0x0085` | `igTangentRelation2d` | 未覆盖 | - | 与 `0x0017` 的 `_TangentRelation2d` 关系需确认 |
| `0x00CE` | `igSymbol2d` | Decoded | `decode_igsymbols` / `SheetIgSymbol2dDecoded` | IDA 已确认名称；下一步追 symbol reference / transform 字段 |
| `0x0115` | `igDimension` | 未覆盖 | - | Phase 20 已确认 parent alias `0x0115` 与 `0x0010` GUID 路径有关；需谨慎 |
| `0x0117` | `igBalloon` | 未覆盖 | - | 标注图元；需定位 reader |
| `0x0118` | `igLeader` | 未覆盖 | - | 标注引线；需定位 reader |

## 4. 当前 parser 已覆盖但不在 `sub_56448F70` 表内的类型

| Type code | 当前名称 | 当前 parser 状态 | 当前 parser / DTO | 证据与缺口 |
|---:|---|---|---|---|
| `0x0010` | PSM sub-record / attribute fragment family | AuditOnly + partial attribute fragment | `decode_sub_records_0x0010` / `decode_attribute_fragments_0x0010` | Phase 20 仅确认 GUID / type-table identity；sub-kind discriminator 仍不得命名 |
| `0x0030` | `JStyleOverride` | Decoded | `decode_jstyle_overrides` / `SheetJStyleOverrideDecoded` | Phase 16 跨 IDA 确认为 RAD `JStyleOverride`，不是 `GArc2d` |
| `0x00FA` | `GraphicGroup` / `GraphicPersist` | AuditOnly | `decode_graphic_groups` / `SheetGraphicGroupDecoded` | 只保守解 header + raw variable tail；不命名 child OID list |
| `0x3FE6` | `GLine2d` | Decoded | `decode_primitive_lines` / `SheetPrimitiveLineDecoded` | SmartPlant 扩展 parametric wrapper；需确认是否由其它 DLL reader 命名 |

这些类型不应强行并入 `sub_56448F70` 的 IGDS switch 表；Phase 27 后续需要通过其它 dispatch table、CLSID、factory 或 reader 函数建立证据链。

## 5. 初始优先级

### P0：校准已 decoded 类型

1. `0x004D igTextBox`
2. `0x0018 igLine2d`
3. `0x0084 igLineString2d`
4. `0x005E igPoint2d`
5. `0x00CE igSymbol2d`

理由：这些类型已有 parser 和 fixture ratchet；IDA 对照最容易形成 `match / mismatch / missing` 结论。

### P1：补齐高价值几何图元

1. `0x0059 igCircle2d`
2. `0x0061 igArc2d`
3. `0x0063 igEllipse2d`
4. `0x007E igEllipticalArc2d`
5. `0x005D igBSplineCurve2d`
6. `0x0020 igRectangle2d`

理由：这些会直接提升 normalized geometry 和 H7CAD 可视化能力。

### P2：关系约束与标注

关系约束：`igParallelRelation2d`、`igPerpendicularRelation2d`、`igConcentricRelation2d`、`igSymmetricRelation2d`、`igEqualRelation2d`、`igColinearRelation2d`、`igHorizontalRelation2d`、`igTangentRelation2d` 等。

标注：`igDimension`、`igBalloon`、`igLeader`、`igSmartFrame2d`、`igComplexString2d`。

理由：这类数据对语义关系和标注有价值，但需要先确认它们在真实 `.pid` fixture 中的出现频率与 reader 位置。

## 6. 下一步执行清单

- [ ] Phase 27-B1：对 P0 五类逐个 `search_text(type name)` + `xrefs_to(string addr)`，建立 `type name → xref function → caller` 链。
- [ ] Phase 27-B2：以 `igTextBox` 为样板，把 `sub_564468B0` 与 `decode_igtextbox_at` 做字段级对照。
- [ ] Phase 27-B3：对 P1 高价值几何图元寻找 reader；如果找不到，记录 negative closeout 和可能需要的新 IDA DLL。
- [ ] Phase 27-C：将 `match / mismatch / missing` 写回本矩阵。

## 7. Guardrails

- 不能把 `0x0010.leading_word` 改名为 `sub_kind`。
- 不能只凭 `sub_56448F70` 名称就声明字段布局 decoded。
- 不能把 IGDS relation type 直接映射为 PID semantic relationship；需要 Dynamic Attributes / Sheet endpoint / reader 函数共同证明。
- 不能把 `0x0030` 重新解释为 arc；Phase 16 已确认它是 `JStyleOverride`。

## 8. Phase 27-B 样板：`igTextBox` IDA ↔ Rust 字段对照

### 8.1 Dispatch chain

`sub_56445F40(int this, int a2, int a3, _WORD *a4, int a5)` 是当前确认的第一层 per-record dispatch：

| Type code | IDA target | 备注 |
|---:|---|---|
| `0x003D` | `sub_564464D0` | `igSmartFrame2d` 候选路径 |
| `0x004D` | `sub_564468B0` | `igTextBox` reader / extraction path |
| `0x00FA` | `sub_56446020` | `GraphicGroup` / `GraphicPersist` 候选路径 |
| default | `sub_564462F0` | 多数普通 IGDS type 共用默认路径；内部调用 `sub_56448F70` 取 type name |

这说明 `sub_564468B0` 是 `igTextBox` 的特化路径，不是单纯名称映射函数。

### 8.2 IDA reader summary

`sub_564468B0(_DWORD *this, _WORD *a2, int a3, int *a4)`：

| IDA evidence | 含义 |
|---|---|
| `*a2 == 77` | type identity：`0x004D = igTextBox` |
| `a2[12] == 1 / 2 / 3` | 文本 payload 有三种 runtime layout |
| mode 1: `sub_56449240(a2) -> a2 + 14`，`len = *(u16*)ptr`，`text = ptr + 2` | mode 1 文本为 `len + UTF-16LE chars` |
| mode 2: `sub_56447710(a2) -> a2 + 14`，`len = *(u16*)(ptr + 8)`，`text = ptr + 10` | mode 2 前面有 8 byte prefix |
| mode 3: `sub_56447730(a2) -> a2 + 14`，`len = *(u16*)(ptr + 4)`，`text = ptr + 6` | mode 3 前面有 4 byte prefix |
| `memcpy(Block, text, 2 * len)` + UTF-16 zero terminator | 文本是 UTF-16LE char buffer |
| `sub_564459D0(..., "igTextBox", ...)` | 创建/查找 RAD object properties |
| vtable call writes `"TEXT"` | 文本属性名为 `TEXT` |
| loop over `a3` writes `"RELEATIONS"` | IDA 还会把 relation ids 写成属性；字符串拼写来自 binary 原样 |

### 8.3 与 `decode_igtextbox_at` 的对照

| 字段/行为 | Rust 当前实现 | IDA 证据 | 结论 |
|---|---|---|---|
| Type identity | `type_code == 0x004D` | `*a2 == 77` | `match` |
| 文本编码 | UTF-16LE → Rust `String` | `len` 个 UTF-16 word，经 wide-char conversion 写入 `TEXT` | `match` |
| 文本属性语义 | `SheetIgTextBoxDecoded.text` | binary property `"TEXT"` | `match` |
| Text layout mode | Rust 固定从 payload offset 30 读 `text_length`，offset 32 读 text | IDA 根据 `a2[12] == 1/2/3` 选择三种 runtime layout | `partial / needs fixture alignment` |
| `sub_type_word` | Rust 读取 payload offset 12，但不用于文本分支 | IDA 使用 `a2[12]` 控制文本 layout | `potential mismatch`；需要确认 raw disk record 与 runtime `a2` 的 offset 映射 |
| `bytes_to_follow` | Rust 用 PSM 6-byte header 约束 `68 + text_len * 2` | `sub_564468B0` 未直接读取 raw `bytes_to_follow` | `not covered by this IDA function` |
| `oid` / `parent_ref` / `index` | Rust 从 payload offset 0/4/14 读取 | `sub_564468B0` 未直接命名这些字段；object helper / manager 可能处理 | `not confirmed` |
| 3 个 trailing f64 | Rust 在 text 后读取 3 个 finite f64 | `sub_564468B0` 未读取 trailing doubles | `fixture-proven only`，不能标为 IDA-confirmed |
| relations | Rust `SheetIgTextBoxDecoded` 未暴露 `RELEATIONS` 属性 | IDA 会遍历 `a3` relation list 并写 property `"RELEATIONS"` | `missing semantic extraction candidate`；需确认 relation ids 是否已由其它 parser 覆盖 |

### 8.4 当前结论

`igTextBox` 的 **类型身份、文本编码、文本语义属性** 已得到 IDA 支持；但 Rust 当前 raw Sheet decoder 的具体 byte offsets 仍主要来自 fixture/probe，而不是 `sub_564468B0` 的直接 offset 证明。

下一步不应立刻改 parser。应先追：

1. `this+0x3c` record manager 的 vtable `+0xA4` 实现，确认 runtime record pointer 与 raw `Sheet*` bytes 的关系。
2. `sub_564462F0` 默认路径，建立其它 IGDS 类型的共用字段读取方式。
3. 当前 fixture 中 `igTextBox` 的 `sub_type_word` 分布，看是否对应 IDA 的 `a2[12] == 1/2/3` 三种 mode。

## 9. Phase 27-B：默认 IGDS 路径 negative evidence

### 9.1 `sub_564462F0` 默认路径

`sub_56445F40` 对 `0x004D` / `0x00FA` / `0x003D` 走特化函数，其余 type code 进入 `sub_564462F0`。

`sub_564462F0(_DWORD *this, _WORD *a2, int a3, int *a4)` 的行为：

1. 调用 `sub_56448F70(a2)` 获取 type name。
2. 如果 type name 为空，改用 decimal type code 字符串。
3. 调用 `sub_564459D0((int)a2, type_name, &object)` 创建/写入 RAD object properties。
4. 遍历 `a3` relation list，把 relation id 写入 `"RELEATIONS"` 属性。
5. 提交/注册 RAD object id。

未观察到：

- line endpoint 坐标读取。
- point 坐标读取。
- polyline point list 读取。
- symbol transform / reference 读取。
- circle / arc / ellipse 参数读取。

### 9.2 P0 / P1 字符串 xref 结果

当前 `radsrvitem.dll` 中以下字符串只命中 `sub_56448F70` 映射表与 `.rdata` 字符串本体：

| Type name | 结果 |
|---|---|
| `igPoint2d` | 仅 `sub_56448F70` |
| `igLineString2d` | 仅 `sub_56448F70` |
| `igSymbol2d` | 仅 `sub_56448F70` |
| `igCircle2d` | 仅 `sub_56448F70` |

`igLine2d` 也只在 mapping 表中命中；`igTextBox` 例外，额外命中 `sub_564468B0` 特化 reader。

### 9.3 结论

当前 `radsrvitem.dll` 证据足以恢复 **type identity / RAD object type name / relation property 写入路径**，但不足以恢复普通几何图元的 raw field layout。

因此 Phase 27-B 对 P0/P1 的下一步分叉为：

1. 继续追 `sub_56445F40 -> vtable offset 164`，确认 record pointer 来源与上游对象管理器。
2. 打开并选择更多相关 IDA 数据库，优先：
   - `J2DSrv.dll`
   - `style.dll`
   - `sppid.dll`
   - `XCeedRAD.dll`
   - `smartplantpid.exe`
3. 若这些 DLL 仍无 per-type reader，则把 `radsrvitem.dll` 对 P0/P1 普通几何字段定性为 negative closeout：仅提供命名证据，不提供字段布局证据。

## 10. Phase 27-B：runtime record pointer 来源追踪

### 10.1 `sub_56445F40` 的 `v10` 来源

本轮继续追 `sub_56445F40`，确认它的 `v10` 不是 raw `Sheet*` stream 指针，而是由 runtime record manager 取出的内部 record pointer：

```text
this+0x3c              runtime record manager
vtable+0xA4            record lookup method
lookup args            (manager, record_id, 0x40, &record_ptr)
record_ptr / v10       runtime record body，首 word 为 type code
```

在 `sub_56445F40` 中：

- `(*(this+0x3c)->vtable+0xA4)(manager, a2, 0x40, &v10)` 根据外部 `record_id` 取 runtime record。
- 取回后读取 `*(u16*)v10` 做 type dispatch。
- `0x004D` 才进入 `sub_564468B0`，因此 `igTextBox` 特化 reader 处理的是 runtime record layout，不是直接处理 CFB `Sheet*` bytes。

### 10.2 上游 `sub_5644B640` 的两种模式

`sub_5644B640` 是当前发现的 vtable 入口之一，数据 xref 位于 `0x5665f280`。它有两种行为：

| 条件 | 行为 | 结论 |
|---|---|---|
| `a3 == 0` | 从 type=1 section payload 遍历 record id 列表，对每个 id 调 `vtable+0xA4`，筛选 `*(u16*)record == 0x0089` | 用于枚举某类 relation / endpoint-like runtime record |
| `a3 == 1` | 直接把外部 `a2` 作为 record id 传给 `sub_56445F40` | 用于解析指定 runtime record |

因此 `sub_56445F40(a2)` 的 `a2` 更应命名为 `record_id` / `runtime_record_id`，不能直接等同为 raw stream offset。

### 10.3 runtime section 编码

`sub_56455240` 先解析 record object，再取 type=1 section：

```text
section[0]        section type，低 7 bit 是 type，高 bit 标记 section list end
section[1]        encoded length；若为 0xFF，则使用扩展长度
section[2..4]     u16 extended encoded length，仅 section[1] == 0xFF 时存在
payload start     section+2 或 section+4
```

相关 helper：

| Helper | 作用 |
|---|---|
| `sub_564546F0(record, section_type, &section)` | 遍历 `record+24` 起始的 section list，按低 7 bit 找 section |
| `sub_56454A20(section)` | 读取 encoded section length |
| `sub_56454880(section)` | 返回 payload start：普通长度 `section+2`，扩展长度 `section+4` |
| `sub_56454860(section)` | 返回 payload length，即 encoded length 减 section header 长度 |
| `sub_56455130(section)` | 设置 section type byte 的 `0x80` end-of-list bit |

`sub_5644B640` 使用的 type=1 section payload 当前可观察到：

```text
payload+0x04      u16 record_count
payload+0x06...   6-byte stride entries
entry+0x00        u32 runtime record id
entry+0x04        2-byte side data / flags（语义未命名）
```

这解释了 `v12 + 6 * (index + 1)` 的访问形式：`index + 1` 会跳过 payload 前 6 字节 header，再以 6 字节 entry stride 取 record id。

### 10.4 `ImpIJPersistManager::vtable+0xA4` 具体实现

继续追 `this+0x3c` 的来源：

- `sub_56445C90` 创建 `PersistManager`，先 QueryInterface `IUnknown` 到 `this+0x44`。
- 随后用 IID `{1FC155A0-6BE3-101B-97A9-08003601CDC9}` 查询到 `this+0x3c`。
- 构造函数 `sub_56467810` 显示该 IID 对应 `ImpIJPersistManager::vftable`。
- `ImpIJPersistManager::vtable+0xA4` 是 `sub_56468DB0`。

`sub_56468DB0` 是薄 wrapper，核心逻辑在 `sub_56468DF0`：

```text
record_id >> 13          segment / bucket index
record_id & 0x1FFF       record descriptor index
record_descriptor+0x00   raw offset within loaded SerialCluster memory
record_descriptor+0x08   page / factory selector bits；高位参与 page entry lookup
record_descriptor+0x0E   flags / availability bits
```

流程：

1. `sub_56479970(record_id, &descriptor_slot)` 按 `record_id` 找 record descriptor。
2. 检查 descriptor flags：拒绝 deleted / unavailable / incompatible records。
3. 用 `record_descriptor[8] >> 20` 选 page/segment entry。
4. `sub_5648C0F0(...)` 从 page table 取 page entry。
5. 调 page entry 的 handler vtable `+0x70`，将 descriptor materialize 为 record pointer。

### 10.5 `SerialCluster::vtable+0x70` materialization

当前命中的 handler 是 `SerialCluster` 对 `{226B6EA0-DC85-11CD-AF23-080036350202}` 接口的实现：

- QueryInterface 证据：`sub_56494B70` 支持 IID `{226B6EA0-DC85-11CD-AF23-080036350202}`。
- vtable 证据：`SerialCluster::vtable+0x70 = sub_56493F50`。
- `sub_56493F50` 最终设置输出：

```text
out_record_ptr = serial_cluster_base + record_descriptor[0]
```

如果 cluster page 未加载，`sub_56493F50` 会先调用 page object vtable `+0x48` 进行加载；随后 `sub_56495BD0(stream, out_record_ptr)` 会按 4KB 页面边界通过 stream `Seek` / `Read` 映射需要的 bytes。`sub_56495BD0` 还会读取 `out_record_ptr + 2` 的 record length，以决定是否跨页继续加载。

配套反向函数也已确认：

```text
sub_56493BC0(serial_cluster, ptr, &offset)
  offset = ptr - serial_cluster_base
```

`sub_564944B0` 负责初始化 `SerialCluster` 的内存映射：

- `SerialCluster+0x2C`：`VirtualAlloc` 得到的 `serial_cluster_base`。
- `SerialCluster+0x30`：page loaded bitmap。
- `SerialCluster+0x34`：映射大小 / stream size 相关值。

`sub_56494C40` 与 `sub_56495BD0` 都使用同一公式做 stream IO：

```text
stream_seek_offset = page_or_record_ptr - serial_cluster_base
```

因此，在 `SerialCluster` 层面，runtime pointer 与 stream offset 已经可互相换算：

```text
runtime_record_ptr = serial_cluster_base + record_descriptor[0]
stream_offset      = runtime_record_ptr - serial_cluster_base
                   = record_descriptor[0]
```

### 10.6 `SerialCluster` stream object 的 CFB 绑定

继续向上追 `SerialCluster` 的 stream 来源，当前确认入口是 `ImpIPersistStorage::Load`：

- `ImpIPersistStorage::vtable+0x18 = sub_56469BF0`，对应 COM `IPersistStorage::Load(IStorage*)`。
- `sub_56469BF0` 保存传入的 `IStorage*` 到 manager 内部字段，并打开固定 PSM streams：
  - `sub_56491090(storage, L"PSMclustertable", mgr+0x2C)`
  - `sub_56491090(storage, L"PSMroots", mgr+0x28)`
  - `sub_56491150(storage, L"PSMspacemap", mgr+0x54)`
  - `sub_56491090(storage, L"PSMcluster0", &tmp)`
- `sub_56491090` 是 `IStorage::OpenStream` wrapper：调用 vtable `+0x10`，模式参数为 `18`。
- `sub_56491150` 是 `IStorage::OpenStorage` wrapper：调用 vtable `+0x18`，模式参数同为 `18`。
- 保存侧 `sub_56469950` / `sub_5646AE30` 对应使用 `sub_56490B30` / `sub_56490BF0` 创建 `PSMclustertable`、`PSMroots`、`PSMspacemap` 等 streams/storages。

这说明当前 `radsrvitem.dll` 证据链已经把 runtime record 指针绑定到 CFB 根 storage 下的 PSM 持久化体系，尤其是 `PSMcluster0` / `PSMclustertable` / `PSMroots` / `PSMspacemap`。本轮没有在该 DLL 中发现 `Sheet*` 字符串或直接打开 `Sheet*` stream 的路径；因此不能把 `record_descriptor[0]` 直接宣称为 Rust parser 看到的 raw `Sheet*` byte offset。

### 10.7 对 `.pid` 格式分析的影响

本轮证据把 `igTextBox` 对照边界进一步收窄：

- IDA 已确认 `0x004D = igTextBox`、UTF-16LE 文本语义、`TEXT` 属性写入。
- IDA 已确认 `sub_564468B0` 读取的是 runtime record layout，且存在 `a2[12] == 1/2/3` 三种 layout mode。
- IDA 已确认 `runtime record pointer = loaded SerialCluster base + record_descriptor[0]`，且 `record_descriptor[0]` 就是该 `SerialCluster` stream 内 offset。
- IDA 已确认该链路从 `IPersistStorage::Load` 进入 CFB 根 storage，并打开 `PSMclustertable` / `PSMroots` / `PSMspacemap` / `PSMcluster0`；当前证据更支持它是 PSM 持久化体系的 stream offset。
- 仍未确认该 PSM `SerialCluster` stream 与 `Sheet*` raw stream decoder 之间是否存在一层对象投影 / envelope / header offset。
- 现有另一个 IDA 实例 `core.dll` 中只找到 `ASHEET` / `DSHEET` 数据库属性初始化与 `CMPTSZ` sheet token 坐标调试/命令输出；未发现 `igTextBox` / IGDS type reader 或 PID `Sheet*` raw record reader 证据。
- IDA 仍未直接证明 Rust raw decoder 的 `payload+30 text_length`、`payload+32 text`、text 后 3 个 f64 等 raw `Sheet*` offset。
- 因此当前 parser 不应仅凭 `radsrvitem.dll` 改 `Sheet*` offset；下一步应切到更可能读取 `Sheet*` 原始几何的 DLL（优先 `J2DSrv.dll` / `style.dll` / `sppid.dll` / `XCeedRAD.dll`），或继续追调用方如何把 PSM runtime record 投影到 sheet-level raw geometry。
