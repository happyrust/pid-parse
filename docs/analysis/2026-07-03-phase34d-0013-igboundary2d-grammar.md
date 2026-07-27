# Phase 34-D 预研：0x0013 igBoundary2d 的计数字段与 0x67 分组文法

> 请求产物路径沿用 Phase 34-D 预研日期。本文于 2026-07-18 在当前
> workspace 重新执行全量探针核验；当前 workspace 已包含后续
> `2026-07-07-phase34d-0013-igboundary2d-grammar-decode.md` 与 typed
> audit-only decoder。本文只记录 probe 证据、文法结论与 decoder slice
> 设计，不修改 parser、schema、writer 或 byte-audit。

## 结论

`0x0013` 的 fixture 文法已经钉死到可实现 typed decoder 的程度：

- payload `+22..+26` 是 little-endian `u32 segment_count`，20/20 均为 3；
- 段区从 payload `+28` 开始，共 `segment_count` 组，每组固定为
  `0x67 + start.x:f64 + start.y:f64 + end.x:f64 + end.y:f64`，即 33 字节；
- `0x67` 是每个 **segment group 的定长前缀 tag**，结构位置为
  `28 + 33*i`；不能通过扫描 payload 内所有值为 `0x67` 的字节来找边界；
- 段区精确范围为 `[28, 28 + 33*n)`；其后是 16 字节 anchor、1 字节
  flag、4 字节 `member_count` 与 `n` 个 8 字节 member reference；
- payload 总长满足 `bytes_to_follow = 49 + 41*n`。当前全部 `n=3`，
  因而 `btf=172`，6 字节 PSM header 后整条记录长 178 字节；
- 20/20 记录的 `member_count == segment_count == 3`；60/60 member OID
  解析到同一 Sheet 流内的 `0x0018 igLine2d`，并按相同下标、正向在
  `1e-9` 容差内匹配段几何。因此它是重列成员线段的 association，
  typed decoder 可实现，但不应再发射一份 closed polyline 造成重复几何。

严格说，`+22` 控制的是 **segment groups 与 member refs 的数量**。当前
三角形记录恰好也有 3 个唯一顶点，但没有 `n != 3` 的真实 fixture，故
API/DTO 应命名 `segment_count`，不应把它泛化命名为 `vertex_count`。

## 复现

```powershell
cargo run --quiet --example probe_0013_igboundary2d_grammar
```

本次实跑：

```text
total 0x0013 records: 20
closed loops (1e-9):  20
open/non-chained:     0
bad group parses:     0
member class all 0x00CB: 20 / bad 0
member oid resolved: 60 / missing 0
member geometry match: 60 / mismatch 0
anchor inside bbox: 20 / outside 0
distinct sub-headers: ×20 [01 00 00 00 03 00 00 00 02 01]
distinct trailers: ×20 trailer_len=45 flag=1 member_count=3 consumed_exact=true
```

六个本地 fixture 中只有三个路径有命中：原始 DWG-0202 为 5 条、
`工艺管道及仪表流程-1` 为 10 条、publish DWG-0202 副本为 5 条；
DWG-0201、D06、A01 均为 0。

## 完整 payload 文法

以下偏移均相对 6 字节 PSM header 后的 payload：

| 范围 / 偏移 | 编码 | 观察与约束 |
|---|---|---|
| `+0..+4` | `u32 oid` | 每条记录对象 ID |
| `+4..+8` | `u32 parent_ref` | 原样保留 |
| `+8..+12` | `u32 remaining_header` | 20/20 为 12 |
| `+12..+14` | `u16 sub_type_word` | 20/20 为 `0x0010` |
| `+14..+18` | `u32 index` | DWG-0202 族为 21，工艺管道族为 24 |
| `+18..+22` | `u32 marker` | 20/20 为 1 |
| `+22..+26` | `u32 segment_count = n` | 20/20 为 3 |
| `+26..+28` | `[u8; 2] sub_header_tail` | 20/20 为 `[2, 1]`；语义未命名 |
| `+28+33i` | `u8 segment_tag` | 对每个 `i in 0..n` 必须为 `0x67` |
| `+29+33i..+61+33i` | `4 × f64 LE` | `start.x, start.y, end.x, end.y` |
| `+28+33n..+44+33n` | `2 × f64 LE` | anchor `(x, y)` |
| `+44+33n` | `u8 trailer_flag` | 20/20 为 1；语义未命名 |
| `+45+33n..+49+33n` | `u32 member_count` | 20/20 等于 `n` |
| `+49+33n+8i..+57+33n+8i` | `u32 + u16 + u16` | `member_oid, class_word, sub_word` |
| payload end | — | `49 + 41n` |

对 fixture 的 `n=3`，段 tag 位于 `+28 / +61 / +94`，段区为
`+28..+127`；anchor 为 `+127..+143`，flag 为 `+143`，
`member_count` 为 `+144..+148`，三个 member refs 为 `+148..+172`。

### 为什么不是“裸 vertex 数组”

一个段组携带两个端点、4 个 f64，共 32 字节；其前还有一个固定
`0x67`，所以 stride 是 33 字节。闭环相邻段会重复共享顶点：
`segment[i].end ≈ segment[i+1].start`。因此：

- 当前每条记录有 3 个 segment groups；
- 有 6 个编码端点槽位；
- 因闭环共享，当前样本只有 3 个唯一顶点；
- 格式中没有另一个独立、连续的 `vertex[]` 区域。

## 逐记录 offset 表

缩写：`D` = `test-file/DWG-0202GP06-01.pid`；`G` =
`test-file/工艺管道及仪表流程-1.pid`；`P` =
`test-file/export-test/publish-data/DWG-0202GP06-01/DWG-0202GP06-01.pid`。
所有记录都位于 `/Sheet6`，`flags=0`、`btf=172`、`+22:u32=3`，
结构 tag 均为 `+28/+61/+94`，trailer 均精确消费到 `+172`。

| # | fixture | record range | oid | index | payload 内非结构性额外 `0x67` | member OIDs |
|---:|---|---|---:|---:|---|---|
| 1 | D | `0x0001AE..0x000260` | 81 | 21 | — | 70, 83, 82 |
| 2 | D | `0x000A7F..0x000B31` | 522 | 21 | — | 521, 524, 523 |
| 3 | D | `0x000C97..0x000D49` | 541 | 21 | — | 540, 543, 542 |
| 4 | D | `0x00460D..0x0046BF` | 4436 | 21 | — | 4435, 4434, 4441 |
| 5 | D | `0x0048A5..0x004957` | 4451 | 21 | — | 4386, 4445, 4449 |
| 6 | G | `0x0052AE..0x005360` | 6997 | 24 | `+48,+65,+81,+98` | 7613, 7507, 7543 |
| 7 | G | `0x0057B2..0x005864` | 7027 | 24 | — | 7089, 7007, 7022 |
| 8 | G | `0x005BD5..0x005C87` | 7051 | 24 | `+46,+63` | 7085, 7024, 7070 |
| 9 | G | `0x00648A..0x00653C` | 7099 | 24 | — | 7149, 7177, 7156 |
| 10 | G | `0x006D70..0x006E22` | 7164 | 24 | — | 7067, 7143, 7145 |
| 11 | G | `0x0095FA..0x0096AC` | 7528 | 24 | — | 6983, 7546, 6990 |
| 12 | G | `0x0098CB..0x00997D` | 7555 | 24 | — | 7622, 7501, 7610 |
| 13 | G | `0x00A295..0x00A347` | 7618 | 24 | — | 7599, 7513, 7396 |
| 14 | G | `0x00A441..0x00A4F3` | 7636 | 24 | — | 6980, 6966, 7631 |
| 15 | G | `0x00A6ED..0x00A79F` | 7652 | 24 | `+31,+41,+113,+123` | 7656, 7344, 7594 |
| 16 | P | `0x0001B6..0x000268` | 81 | 21 | — | 70, 83, 82 |
| 17 | P | `0x000A87..0x000B39` | 522 | 21 | — | 521, 524, 523 |
| 18 | P | `0x000C9F..0x000D51` | 541 | 21 | — | 540, 543, 542 |
| 19 | P | `0x004615..0x0046C7` | 4436 | 21 | — | 4435, 4434, 4441 |
| 20 | P | `0x0048AD..0x00495F` | 4451 | 21 | — | 4386, 4445, 4449 |

## 反例检查

### 原始 0x67 扫描有三个反例

记录 6、8、15 的 f64 原始表示内部恰好含有值为 `0x67` 的字节。若把
payload 中每个 `0x67` 都当成 tag，会得到：

```text
#6  [28, 48, 61, 65, 81, 94, 98]
#8  [28, 46, 61, 63, 94]
#15 [28, 31, 41, 61, 94, 113, 123]
```

但由 `segment_count` 驱动、按 33 字节 stride 读取时，三条记录的结构
tag 仍严格是 `[28, 61, 94]`，4×f64 均有限，trailer 边界也精确落在
`+127`。所以这些是对“按字节值扫描”的反例，不是对定长分组文法的反例。

### 文法本身无 fixture 反例

20/20 都满足：

1. `n = u32_le(payload[22..26]) = 3`；
2. `btf = 49 + 41*n = 172`；
3. 每个 `payload[28 + 33*i] == 0x67`；
4. 段区恰在 `+28..+127` 结束；
5. `member_count == n`，三个 8 字节引用恰好消费到 payload 末尾；
6. 3 个段以 `1e-9` 容差闭合；
7. anchor 位于段 bbox 内；
8. 3 个 member 均解析到同流 igLine2d，按下标正向匹配对应段。

证据边界：真实 fixture 只覆盖 `n=3`，所以 `49+41n` 对其他 n 的行为是
由字段结构推出并由 synthetic tests 验证的 decoder 泛化，不是多边形
实样本证据；`+26..+28`、trailer flag、member `class_word/sub_word`
也只有稳定取值，尚无 native reader 证据可进一步语义命名。

## 七层 decoder slice 设计

结论是 **可实现 typed audit-only decoder**；当前证据不支持额外 normalized
geometry emission。

1. **Probe**
   - 保留 `examples/probe_0013_igboundary2d_grammar.rs`；
   - 全量打印 payload、结构 tag、raw `0x67`、分组坐标、trailer、成员解析、
     闭环与 bbox 统计。
2. **布局**
   - 固定开销 `49`，每段开销 `33 + 8 = 41`；
   - 段区由 `segment_count` 驱动，不做 sentinel 扫描。
3. **Decoder API / DTO**
   - `decode_igboundaries(&[u8]) -> Vec<SheetIgBoundary2dDecoded>`；
   - `decode_igboundary_at(&[u8], offset) -> Option<...>`；
   - DTO：`byte_range/type_code/type_flags/bytes_to_follow/oid/parent_ref/
     sub_type_word/index/segment_count/sub_header_tail/segments/anchor/
     trailer_flag/member_refs`；
   - segment DTO 暴露 `tag_offset/start/end`，member DTO 暴露
     `member_oid/class_word/sub_word`；
   - 提供带容差的 `is_closed_loop`，不要把闭环作为接受记录的必要条件。
4. **Validation**
   - type `0x0013`、flags 0；
   - `remaining_header == 12`、`sub_type_word == 0x0010`、`+18 u32 == 1`；
   - `1 <= n <= defensive_cap`，所有长度计算使用 checked arithmetic；
   - `btf == 49 + 41*n`；
   - 仅在 `+28+33*i` 验证 `0x67`；
   - 坐标和 anchor 有限且落在统一坐标域上限内；
   - `member_count == n`；
   - 拒绝所有 segment 顶点完全相同的退化伪命中。
   - `[2,1]`、trailer flag、`0x00CB` 与 `13/12/12` 应先原样暴露并在
     fixture ratchet 断言，不宜在缺少变体证据时全部写死为 parser 门禁。
5. **Unit / panic tests**
   - canonical 三段闭环、开放链仍可 decode 但 `is_closed_loop=false`；
   - wrong type/flags/btf/remaining-header/sub-type/marker/tag/member-count；
   - count 0、超 cap、NaN/Inf/越域坐标、退化顶点；
   - 每个截断长度、空输入、`usize::MAX` offset、随机噪声、背靠背记录；
   - synthetic `n != 3` 用于验证长度算术，但明确不替代真实 fixture。
6. **Model / schema / ratchet**
   - `SheetGeometry::decoded_igboundaries` 保存 typed audit records 与完整
     provenance；
   - schema needle 覆盖记录、segment、member、`segment_count`、
     `member_oid`、`closed_loop`；
   - fixture 精确计数为 `0/5/10/0/0/5`，总计 20；
   - ratchet `btf=172`、n=3、20/20 closed、20/20 anchor-in-bbox、
     60/60 member resolved、60/60 forward geometry match。
7. **Pipeline / emission**
   - cluster 层接入 typed collection，byte-audit 在每条完整记录范围上标记
     `Decoded`；
   - geometry emitter 对该族显式 no-op：成员 igLine2d 已发射 Line，
     boundary 再发射 Polyline 会双计；
   - writer 行为不变。只有未来出现不重列成员线的独立 boundary fixture，
     并定义清楚下游契约后，才重开 geometry emission 决策。

## 最终判定

**可实现 decoder，且应是 fully typed / audit-only。** 当前 20 条记录没有
文法反例；`+22 u32`、33 字节 segment group、trailer 边界与
`btf=49+41n` 形成闭合的长度证明。仍缺的证据只影响未知字段语义和未来
几何发射策略，不阻塞 typed decoder。
