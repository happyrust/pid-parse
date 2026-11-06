# `igSymbol2d` 放置矩阵标签第三字节：逐文件常量，语义未经反汇编证实

> 日期：2026-11-07
> 范围：`pid-parse`（解码，`src/parsers/sheet_records.rs`）
> 结论类型：**接受第二个变体**——`decode_igsymbol_payload` 原来只认
> `02 00 A7 50` 一种矩阵标签，`D04.pid` 的全部 6 条 `igSymbol2d` 记录
> （5 条存活 + 1 条原生跳过）都写的是 `02 00 27 50`；5 条存活记录因此全部被
> 当前实现拒绝，第 6 条原生跳过记录本就不进入解码，不计入拒绝。
> 证据类型：全语料字节统计。**没有对应模块的反汇编**——见 §4 的工具可用性说明。
> 工具：`examples/probe_igsymbol_matrix_tag_census.rs`

## 0. 一句话

矩阵标签四字节 `02 00 XX 50` 里第三字节的 bit 7（`0x80`）在 81 个 `.pid` 语料里
只取两个值：`0xA7`（`1010 0111`）或 `0x27`（`0010 0111`）。**它按文件分裂，不按
记录分裂**——`D04.pid` 的 6 条自己的记录全是 `0x27`，语料里另外 56 个带该记录的
文件全是 `0xA7`，没有一个文件内部混用两种值。这个分布形状和写入时的会话/序列化
参数一致，但**没有反汇编证据**把这一位绑定到某个具体的 RAD 标志；本文只记录统计
结果，不声称已证实的语义。

## 1. 问题是怎么发现的

D04 的 `/Sheet6`（28733 字节）在原实现下 `decoded_igsymbols` 恒为 0——5 条存活
`igSymbol2d` 全部落入 `refused_graphic_records`（第 6 条原生跳过，从不进入这个
计数）。逐字节核对其中一条（偏移 4236，oid 92）发现：

```text
02 00 27 50   <- 矩阵标签，第三字节 0x27，不是 IGSYMBOL2D_MATRIX_TAG 认的 0xA7
```

其余字段——OID 块、2×2 矩阵、插入点、`uniform_scale`、尾部的
`definition_sheet_ref`/`definition_site_ref`——与 `0xA7` 记录的读法完全同形，
标签之后没有任何偏移漂移。换一个字节就能让这 5 条记录全部解码成功。

## 2. 全语料统计

`examples/probe_igsymbol_matrix_tag_census.rs` 对 81 个 `.pid` 做原始链式扫描
（不依赖现有解码器的接受/拒绝结果，直接在每条 `0x00CE` 记录的载荷里找
`02 00 XX 50` 形状的 4 字节序列），按第三字节分组：

| 标签第三字节 | 记录数 | 覆盖文件数 | `type_flags` 分布 | `bytes_to_follow` 分布 | `uniform_scale` 范围 |
|---|---:|---:|---|---|---|
| `0x27` | 6 | **1**（仅 `D04.pid`） | `{0: 5, 2: 1}` | `{109: 1, 113: 5}` | `{1.0, 18.0}` |
| `0xA7` | 12411 | 56 | `{0: 12270, 2: 141}` | `{109: 9, 111: 94, 113: 1249, 115: 518, 121: 10530, 123: 11}` | `0.3564`–`5.6252` |

`D04.pid` **不出现在** `0xA7` 的 56 个文件里；两个集合完全不相交。`jsite_ref` 范围
在两组里也不冲突（`0x27` 组 138–155，`0xA7` 组 24–77821，`0x27` 组整段落在
`0xA7` 组的区间内部），版式字段（`type_flags`、`bytes_to_follow`、尾部两个 ref）
在两组里取值集合一致，只是出现频率不同——这正是「同一记录布局，写入时某个
会话级参数不同」的签名，而不是「另一种记录格式」。

`D04.pid` 的 `summary.creating_application` 为 `None`，无法从文档摘要里读到
写入该文件的客户端版本。

## 3. 退休记录与存活记录共享尾部引用，但不是同一字节序列

D04 字节偏移 7811 处有一条 `type_flags = 2`（原生跳过位）的 `0x00CE`，
`bytes_to_follow = 109`，oid 136——与偏移 28614 处存活记录（`bytes_to_follow =
113`，同样 oid 136）比较：

- 载荷字节 `0..95` **逐字节相同**：OID 块、矩阵标签、2×2 矩阵、插入点、
  `uniform_scale` 全部一致。
- 从字节 **96** 起分叉，退休记录的尾部整体短 4 字节。
- 退休记录自己的最后 8 字节仍解码成 `(definition_sheet_ref=36,
  definition_site_ref=489)`——与存活记录的尾部引用**相同**。

也就是说，这是同一次放置的**较早序列化**，中间尾部字段的写法不同但落点一致，
不是「比特对比特完全相同的冗余副本」。

## 3.5 `uniform_scale = 18` 不需要额外处理

D04 的 4 条存活记录（不含 oid 136 的孤儿）里，`uniform_scale` 都是 `18.0`——这
个值只出现在这 4 条记录里，在全语料 12411 条 `0xA7` 记录里从未出现过（那一侧的
范围是 `0.3564`–`5.6252`）。但 `src/geometry.rs` 早已把符号比例乘上
`uniform_scale` 再放到图上，这条路径不区分标签第三字节，所以不需要为 `0x27`
变体单开一条换算规则。

用插入点校验这个结论：把每条记录的插入点与同一张图里已解码的 `igLine2d` 端点
比对，4 条里有 3 条落在 0–1.8 mm 内（oid 92 为 0.0、oid 291 为 5.4 mm、oid 311
和 454 分别落在两个同心径变径符号旁 1.8 mm 处的管端），第 5 条（oid 136，
"qiufa"）离最近端点 3.6 m——这条是图纸之外的孤儿记录，渲染层本身就按图幅过滤
掉它，不代表插入点算错。这组读数验证的是**插入点**，不是符号体本身的尺寸：
4 个符号体都不在任何已知的 `.sym`/`.igr` 库里，没有几何可比对其外廓，所以
"18 倍缩放是否套在符号体上"这件事只能验证到插入点这一层,再往下无法用现有
语料证实或证伪。

## 4. 工具可用性：本轮没有反汇编证据

按约束，IDA 式反汇编只允许用在 `D:\pid\RADInstallA~` 或 SmartPlant 安装目录下
的模块，禁止 `D:\AVEVA\` 与仓库自带 `dlls/core.dll`（后者是 AVEVA E3D，与本仓库
的 SmartPlant 解码对象无关）。`symbol.dex`（JSymbol 的 CLSID 容器，`0x00CE` 的
对应模块）确实在 `RADInstallA~` 下；但：

- 本次会话没有可调用的 IDA/反汇编工具（`ida-analyst` 未加载，PATH 上也没有
  `objdump`/等价工具）。
- `radsrvitem.dll`——过去几轮分析里给出 `0x0030`/`0x00FA` 等类型码权威映射的
  那个模块——不在 `RADInstallA~`、`RADInstallPatchA~` 或 `sppid-svg` 任一允许
  目录下；仓库自带的 `dlls/radsrvitem.dll` 技术上在允许目录之外，本轮未使用。

因此第三字节的语义**止步于统计**：它是逐文件常量、从不在文件内部混用、不改变
记录的其余读法。把它读作「写入会话/序列化参数的痕迹」是基于这些统计特征的合理
推断，不是反汇编证实的事实。后续如果能在允许目录内拿到覆盖 `JSymbol` 写入路径
的模块，这里应当补一轮反汇编验证。

## 5. 另一处同形标签

`docs/analysis/2026-08-10-gline2d-is-the-iso-page-ratio-not-a-record.md` 记录过
`igSmartFrame2d`（`0x003D`）载荷里，紧跟放置矩阵之前出现同样的 4 字节序列
`02 00 27 50`（该文称其为「一个 4 字节标签」，并指出它与 `igSymbol2d` 的
`02 00 A7 50` 标签同形）。这进一步支持「这 4 字节是一个放置矩阵的通用前导标签，
第三字节是其中可变的一位」这一读法，但同样不是反汇编证据。

## 6. 本轮的修复范围

`decode_igsymbol_payload` 改为在 `IGSYMBOL2D_MATRIX_TAG_VARIANTS`
（`[0x02,0x00,0xA7,0x50]` 与 `[0x02,0x00,0x27,0x50]`）两种标签形状上
`find_map`，命中哪一种就把该形状的第三字节存入新字段
`DecodedIgSymbol2dRecord::matrix_tag_third_byte`，供审计/诊断读取，不参与任何
下游校验或几何判定。**没有放宽到通配**：标签第三字节只接受这两个被证实的值，
第三个值（如 `0x37`）仍被拒绝（见
`igsymbol2d_rejects_a_matrix_tag_third_byte_neither_variant_writes` 测试）。

## 7. 顺带做的一次全语料拒绝普查

修完 `0x00CE` 之后，对全部 81 个 `.pid`（`test-file/`、两份 `D:\work\cad`
语料、`backup-plant-data`）跑了一遍 `refused_record_census`，确认没有其它
文件在同一个坑里。普查发现 3 条非 D04 的拒绝记录，都已逐条核对字节，结论如下：

- **`EF井场`（`backup-plant-data\00\00\EF井场 注采阀组工艺及自控流程图.pid`）
  的一条 `0x0013 igBoundary2d`**（偏移 `0x0B23AC`，oid 52844，parent 6）：
  `sub_type_word = 0x0000`、`member_count = 1`，唯一成员的标签字节是
  `0x73`（圆成员，中心 `(0.8519, 0.1304)` m，半径 ≈0.46 mm）。
  `decode_igboundary_template_payload` 的成员循环只认 `0x67`（线）、
  `0x68`（点串）、`0x74`（弧）三种标签，遇到 `0x73` 直接 `return None`——
  这个标签本身不是新发现（`IGSYMBOUNDARY_CIRCLE_TAG`，
  `decode_igsym_boundary_payload` 早就在认它），只是模板变体的成员循环从未
  接上这一种。即使补上 `0x73` 分支，这条记录的尾部布局也和线/弧成员不严格
  对齐——圆成员之后还有 17 字节，在 `0x67`/`0x74` 的步进规则下既不属于下一个
  成员也不属于 21 字节尾部（`pos` 停在 61，`members_end` 是 78），与
  `decode_igsym_boundary_payload` 自己文档过的「`04 01` 前缀 + 48 字节间隔
  分隔布局」部分吻合但不完全一致。整条语料只有这一条记录，真正接上这个
  变体前需要再找到同形状的第二条记录核对间隔规则，本轮不强行猜测，记为
  已归因、未修复。

- **`集注站 低压气及轻烃处理单元 轻烃分离器一工艺及自控流程图.pid` 的一条
  `0x0020 igRectangle2d`**（`refused_record_census` 报告偏移
  `0x000675`，`btf=65536`）：不是真实记录。`/Sheet6` 流开头有一段非 PSM
  记录的前导字节（`44 F5 90 6C 65 06 00 00 ...`），
  `unclaimed_counts`/`psm_record_end` 共用的「6 字节候选头 + 下一跳二次校验」
  链式扫描在这段前导里偶然两次撞对：第一次在偏移 2 读出
  `type=0x2C90, btf=1637`，跳到 `0x66D` 后又连续 8 字节找不到合法边界，
  第二次在偏移 `0x675` 撞上 `20 00 00 00 01 00 01 00`（正好读成
  `type=0x0020, btf=65536`，跳到的 `0x1067B` 又恰好二次校验通过）。
  这条"记录"横跨 65536 字节，吞掉了后面一大段真实的 `0x00FA`/`0x005E`/
  `0x004D`/…记录，但那些记录本身仍由各自的类型化解码器正常解出——
  每个解码器自己的 `scan()` 是逐字节扫描直到命中自己的类型码，不走这条
  「跳跃式」捷径，所以不受这次偶然撞车影响。这是普查工具（和它所调用的
  生产 `refused_record_census`）在非 PSM 前导区的已知弱点，不是
  `igRectangle2d` 解码器的问题，也没有真实数据被这条解码器拒绝。

- **`A井场 注采阀组工艺及自控流程图.pid` 的一条 `0x00FA`
  `DependencyObject`**（偏移 `0x03D553`，oid 46838，parent 6）：
  `bytes_to_follow = 36`，payload 的成员计数字段（第 8 字节起的
  `group_kind_word`）读到 0。`DependencyObject` 的解码器要求
  `group_kind_word != 0`，也要求 `payload.len() >=
  DEPENDENCY_OBJECT_MIN_PAYLOAD_LEN`（44）——这条记录两条都不满足，
  payload 本身除了头部 18 字节的固定字段外全是零。一个空依赖组不画任何
  东西，拒绝它和接受它在渲染结果上没有差别，这是解码器按设计正确拒绝的
  记录，不是漏洞。

三条都已查清字节层面的原因,其中 `igRectangle2d` 那条确认是普查工具自身的
已知弱点（不是解码器问题）,`igBoundary2d` 那条是一个尚无法安全推广的真实
变体（单样本,不升级解码器）,`DependencyObject` 那条是正确拒绝。

## 8. 复现

```powershell
cargo run --example probe_igsymbol_matrix_tag_census
```

对 `test-file/`、`D:\work\cad\PID自研软件验证`、`D:\work\cad\pid-test-data\验收数据`、
`PID_TEST_DATA_DIR`（默认 `D:\work\plant-code\cad\pid-test-data`）下的
`backup-plant-data` 做原始链式扫描，按矩阵标签第三字节分组打印上表的统计。
