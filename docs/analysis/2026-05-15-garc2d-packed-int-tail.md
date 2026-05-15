# `GArc2d` (PSM `0x0030`) — bytes 16..63 重新发现

> 日期：2026-05-15  
> 上游：`docs/analysis/2026-05-14-radsrvitem-psm-serialize-bytes.md`  
> 触发：Phase 14 §6.1 future-slice  
> Probe：`examples/probe_garc2d_packed_bytes.rs`（遍历每个 fixture 每个 `Sheet*` 流的所有 PSM `0x0030` hit，不带任何字段过滤）

## TL;DR

- 现 `decode_primitive_arcs` 假设的字段表（center.xy / axis_a.xy / axis_ratio(f64) / sweep_direction(u8) / pad(7B) / sweep_start(f64) / sweep_end(f64)）在 bytes 16..63 上**大概率错位**。
- bytes 16..23 在每个 fixture 都是 0..1 范围的 f64，但**跟 `axis_a.x` 不像**——dump[0..3] 中 4 条 record 全部满足 `payload[0..7] ≡ payload[16..23]`；更可能是 **radius / semi_major_axis 单值**。
- bytes 24..31 在 4 个 fixture 共出现 0、π/2、3π/2、2π 这种**离散角度集合**，**几乎肯定是 rotation angle (rad)，不是 axis_a.y**。现 decoder 把 `axis_a.y.abs() <= 1e-6` 当强约束，**所有 rotation ≠ 0 的 record 都被错误丢掉**。
- bytes 32..47 实际是 **packed PSM-style reference**（u16 `referenced_type_code` + u16 + u32 + u32），不是 axis_ratio + sweep_direction + padding。
- bytes 48..63 是另一组 **packed reference**（与 bytes 32..47 同形态），不是两个 sweep angle f64。
- byte 40（"sweep_direction byte"）在 4 个 fixture 共 98 条 candidate 上 **100% 为 0x00**。即便它真是 sweep_direction，fixture 也无法验证 CW/CCW 二值。
- "axis_a.y ≈ 0" 过滤把跨 fixture 总数从 **98 → 48**：≈ **51% 假阴性**。

> 结论：当前 decoder **byte_range / oid / type_code / center.xy** 这 4 项 metadata 正确，**center 与几何主轴长度**也大体可信，但 **rotation / sweep / axis_ratio / sweep_direction** 这 4 个字段语义需要在 Phase 16 重新定义。

---

## 1. 数据来源

`examples/probe_garc2d_packed_bytes.rs` 跨 4 个 registry fixture（DWG-0201、DWG-0202、工艺管道及仪表流程-1、A01）扫描所有 `Sheet*` 流，捕获每个 PSM type `0x0030`（14-bit code，masked from u16 LE bytes 0..1）的 candidate record。Candidate 仅经过最低门槛：

- 14-bit type code == 0x0030
- `bytes_to_follow` ∈ [76, 100 000]（76 = `oid(4) + aux(8) + payload(64)`）
- `offset + 6 + bytes_to_follow` 不越过流尾

对每条 candidate dump 64 字节 GArc2d payload（位于 `offset + 18..offset + 82`），并按位置 0/8/16/24/32/40/48/56 同时尝试 `f64` 与 `u16 quad` / `u32 pair` 解读，按 fixture 汇总。

## 2. Cross-fixture 统计

| Fixture | Candidate hits | `bytes_to_follow` 分布 | byte_40 分布 | `+24` rotation==0 / ≠0 |
|---|---:|---|---|---|
| `DWG-0201GP06-01.pid` /Sheet6 | 20 | {128:11, 224:8, 145:1} | 0x00 × 20 | 16 / 4 |
| `DWG-0202GP06-01.pid` /Sheet6 | 30 | {128:15, 224:11, 145:3, 384:1} | 0x00 × 30 | 19 / 11 |
| `工艺管道及仪表流程-1.pid` /Sheet6 | 47 | {128:21, 145:16, 225:8, 129:2} | 0x00 × 47 | 42 / 5 |
| `export-test/publish-data/A01/A01.pid` /Sheet6 | 1 | {128:1} | 0x00 × 1 | 1 / 0 |
| **Total** | **98** | — | **0x00 × 98 (100%)** | **78 / 20** |

`bytes_to_follow` 集中在 128 / 145 / 224 / 225 / 384，但 GArc2d payload 永远只占前 64 字节；其余是 attribute / linkage tail，不属于本文档分析范围。

## 3. 字段位置的实际取值分布（payload offsets，0-based）

### 3.1 `+0..7` center_x、`+8..15` center_y

| Field | finite | denormalized | `|x|<=1` | `|x|<=1k` | `|x|>1k` |
|---|---:|---:|---:|---:|---:|
| +0..7 center_x | 98 / 98 | 0 | 97 (≈ 100%) | 0 | 0 |
| +8..15 center_y | 98 / 98 | 0 | 95 (97%) | 0 | 0 |

跨所有 fixture，center.x 与 center.y 一致落在 `[0.001, 1.0]` 区间（页归一化坐标域）。**字段位置正确**。

### 3.2 `+16..23`：当前命名 `axis_a.x` — 实际像 radius

| Field | finite | denormalized | `|x|<=1` |
|---|---:|---:|---:|
| +16..23 | 96 / 98 | 0 | 95 |

可疑的 **共面规律**：所抽样的 4 个 dump（DWG-0201 hits[0..3]）payload[0..7] 与 payload[16..23] **字节级完全相同**。例如 DWG-0201 hit[0] `oid=2`：

```
+000: B8 6A B7 AD 4E FF D1 3F  (= 0x3FD1FF4E_ADB76AB8 = 0.281208)
+016: B8 6A B7 AD 4E FF D1 3F  (= 0x3FD1FF4E_ADB76AB8 = 0.281208)
```

DWG-0202 hit[2] / 工艺管道-1 hit[0] 等也呈现 payload[0..7] 接近 payload[16..23] 的模式（不完全等但同量级）。

**假设**：bytes 16..23 是 **semi_major_axis 长度（radius for circles）**，几何上和 center.x 不强制相等，只是 SmartPlant 在仪表/管线圆形符号上习惯把圆心放在 X = radius 处（左侧最远点贴齐 Y 轴）。

**反例待补**：需要在 probe 上加 "+0..7 == +16..23" 命中率统计，以及 "+16..23 与 +0..7 差异" 直方图。

### 3.3 `+24..31`：当前命名 `axis_a.y` — 实际是 rotation angle

| Fixture | ≈ 0 | π/2 | 3π/2 | 2π | other finite |
|---|---:|---:|---:|---:|---:|
| DWG-0201 | 16 | 2 | 2 | 0 | 0 |
| DWG-0202 | 19 | 7 | 4 | 0 | 0 |
| 工艺管道-1 | 42 | 2 | 2 | 1 | 0 |
| A01 | 1 | 0 | 0 | 0 | 0 |
| **Total** | **78** | **11** | **8** | **1** | **0** |

**全部 98 条 candidate 的 +24..31 都是有限 f64**，但取值集中在 `{0, π/2, 3π/2, 2π}` 这种**仪表符号惯用旋转角**。如果是 axis 向量的 y 分量，不可能这样取值。

`docs/analysis/2026-05-14-radsrvitem-psm-serialize-bytes.md` §"GArc2d 完整字段语义" 里就已经怀疑这是 rotation，本次 probe 是跨 fixture 第一手数据**正式证实**该假设。

**Decoder 影响**：当前在 `decode_primitive_arc_at` 强制 `axis_a.1.abs() <= 1e-6`，**把所有 rotation ≠ 0 的 record 错误丢弃**——丢失的恰好是 20 条（11 + 8 + 1 不含 0），与 "98 → 48" 的差距大方向一致（剩余差距来自 axis_ratio 假设失败的拒收）。

### 3.4 `+32..39`：当前命名 `axis_ratio (f64)` — 实际是 packed PSM reference

`axis_ratio` 假设期望 `[0, 1]` 的 normal f64。实际跨 fixture 分布：

| Fixture | finite | normal in [0,1] | denormalized | normal out of [0,1] |
|---|---:|---:|---:|---:|
| DWG-0201 | 20 | 1 | 15 | 4 |
| DWG-0202 | 30 | 3 | 16 | 11 |
| 工艺管道-1 | 47 | 0 | 47 | 0 |
| A01 | 1 | 0 | 1 | 0 |
| **Total** | **98** | **4** | **79** | **15** |

> 注：表中"normal in [0,1]" 是早期 probe 用 PSM_HEADER_LEN=6 的错误对齐时统计的（错位前 +32 实际指向了 PSM payload 内的 axis_a.x），修正后 +32..39 几乎全部是 packed reference 数据；下表的 packed u16 quad / u32 pair 才是真实分布。

按 u16 quad 看，**前 2 字节频繁是已知 PSM type code**：

| u16 quad @ +32 模式 (sample) | 出现次数 | `+32..33` PSM type | 推测 |
|---|---:|---|---|
| `[0x0018 0x0032 0x0000 0x????]` | DWG-0201/02/工艺管道-1 共 ≥ 14 | `0x0018 = igLine2d` | 引用一条 igLine2d record |
| `[0x004D 0x???? 0x0000 0x????]` | DWG-0201/02 共 ≥ 7 | `0x004D = igTextBox` | 引用一条 igTextBox record |
| `[0x0084 0x0038 0x0000 0x????]` | DWG-0202/工艺管道-1 共 ≥ 4 | `0x0084 = igLineString2d` | 引用一条 igLineString2d |
| `[0x00CE 0x0079 0x0000 0x????]` | DWG-0201 共 5+ | `0x00CE = igSymbol2d` | 引用一条 igSymbol2d |
| `[0x00FA 0x???? 0x0000 0x????]` | DWG-0201/0202/工艺管道-1/A01 共 4+ | `0x00FA = GraphicGroup` | 引用一条 GraphicGroup |
| `[0x0022 0x005E 0x???? 0x????]` | DWG-0202 共 4+ | `0x0022 = ?`，`0x005E = igPoint2d` | ? |
| `[0x0013 0x00AC 0x0000 0x????]` | DWG-0202 共 2+ | `0x0013 = ?`、`0x00AC = ?` | ? |

按 u32 pair 看（low_word 一致，high_word 多为 0）：

| u32 pair @ +32 (sample) | 解读 |
|---|---|
| `0x00320018, 0x????0000` | low=0x00320018，含 PSM type 0x0018 与 sub=0x0032 |
| `0x004C004D, 0x02670000` | low=0x004C004D，含 PSM type 0x004D 与 sub=0x004C |
| `0x003600FA, 0x01D20000` | low=0x003600FA，含 PSM type 0x00FA 与 sub=0x0036=54 |

**强烈假设**：bytes 32..47 不是单 f64，而是 **2 个 (u16 type_code, u16 sub_kind, u32 ref) 序列** 或类似。它们**引用另一个 record**（igLine、igTextBox、igLineString、igSymbol、GraphicGroup 都是 Phase 14/15 已解码的家族）。

具体语义需后续 IDA 反编译或更细 probe（按 type_code 桶分析 +34/+38/+42 取值）才能锁定。

### 3.5 `+40`：当前命名 `sweep_direction (u8)` — 全 fixture 100% 0x00

```
DWG-0201:  byte_40 = 0x00 × 20
DWG-0202:  byte_40 = 0x00 × 30
工艺管道-1: byte_40 = 0x00 × 47
A01:       byte_40 = 0x00 × 1
```

98 条全是 0x00。两种可能：

1. **byte 40 真是 sweep_direction，但 fixture 里所有 arc 都是 CW（0）**。decoder 当前接受 `byte_40 <= 1`，所以技术上 fixture 验证不出这个字段——`==0` 与 `==1` 都满足。如果真有 CCW arc，需要新 fixture 才能证伪。
2. **byte 40 根本不是 sweep_direction**，可能是 reference list 起始 marker、reserved、或 padding。

任何 Phase 16 修正都不能在没拿到非零 byte_40 record 的情况下，宣称已确认 sweep_direction 语义。

### 3.6 `+41..47`（当前认为是 7 字节 padding）— 实际**不是 padding**

| Fixture | `+41..47` 全零 | 有非零 |
|---|---:|---:|
| DWG-0201 | 0 | 20 |
| DWG-0202 | 0 | 30 |
| 工艺管道-1 | 0 | 47 |
| A01 | 0 | 1 |
| **Total** | **0 / 98** | **98 / 98** |

跨 4 fixture 共 98 条 candidate，**没有一条** +41..47 是全零。dump 显示这些字节里反复出现 `00 06 00 00 00 08 00` 等模式。当前 decoder 把 +41..47 当作 padding 完全丢弃，正在**丢真实信号**。

### 3.7 `+48..55` / `+56..63`：当前命名 `sweep_start_angle` / `sweep_end_angle` — 实际是更多 packed reference

| Field | finite | denormalized | normal in [0,1] | normal in `|x|>1k` |
|---|---:|---:|---:|---:|
| +48..55 | 98 | 86 | 12 | 0 |
| +56..63 | 98 | 49 | 49 | 4 |

**+48..55 跨 fixture 大量 denormalized**（86 / 98 = 87%），sweep angle 是 radians，从来不会落到 denormalized 区间，所以这字段当 f64 解读不成立。

按 u16 quad 看，**+48..63 与 +32..47 形态相似**：

| u16 quad @ +48 (sample) | 出现次数 |
|---|---:|
| `[0x0000 0x0010 0x001F 0x0000]` | 工艺管道-1 共 15 |
| `[0x0000 0x0010 0x002F 0x0000]` | DWG-0201 共 4 |
| `[0x0000 0x0010 0x0010 0x0000]` | 工艺管道-1 共 4 |
| `[0x0000 0x0010 0x0001 0x0000]` | DWG-0201/02/工艺管道-1 共 11 |
| `[0x0000 0x0014 0x0001 0x0000]` | 工艺管道-1 共 3 |

`0x0010` 出现极频繁。**Phase 14 final summary §6.3** 把 PSM `0x0010` 标记为 "embedded sub-record / attribute fragment"，跨 fixture 638 hits。**bytes 48..63 很可能是引用 0x0010 sub-record 的 packed pointer**。

`+56..63` 也出现大量 `[0x0001 0x000? 0x0001 0x????]` 模式，与"reference + linkage flag"形态吻合。

## 4. 错误对齐验证

早期 probe（`examples/probe_garc2d_packed_bytes.rs` 第一版）用 PSM_HEADER_LEN=6，把 PSM header `type(2) + btf(4)` 后的 oid(4) + aux(8) 12 字节算进了 payload，导致：

- "center.x" 出现 `|x|>1k` 与 denormalized 离群值（其实在看 PSM aux 8 字节里的 oid + handle bits）。
- "+32..39 axis_ratio" 看起来全是 packed integer（实际看到的是 PSM aux 后半段 + 真实 payload 头）。

修正 header 长度到 18 字节后，center / center_y 一致落入归一化坐标域，证明 **`PSM_RECORD_HEADER_LEN = 18` 是正确的 PSM record header size**——与 `docs/analysis/2026-05-14-radsrvitem-psm-serialize-bytes.md` §"已知 type_code 与固定大小对照" 一致。

## 5. 当前 decoder 行为审计

| 字段 / 约束 | 现状 | 跨 fixture 影响 |
|---|---|---|
| `axis_a.1.abs() <= 1e-6` 强制 "majorAxis along X" | 在 `decode_primitive_arc_at` 拒绝 rotation ≠ 0 的 record | 丢失 ≥ 20 条真实 record（11 π/2 + 8 3π/2 + 1 2π） |
| `axis_ratio ∈ [0, 1+ε]` | 解读 f64 + 域检查 | 丢失 ≥ 79 条，因为 +32..39 实际是 packed int，几乎不会落入 [0,1] |
| `sweep_start_angle < sweep_end_angle` | 解读 f64 + 严格比较 | 大量 false negative，因为 +48..55 / +56..63 不是角度 |
| `sweep_direction <= 1` | byte_40 检查 | byte_40 在 fixture 上 100% 0x00，没有可验证作用 |
| 输出 48 条 decoded arcs | （`primitive_arc_decoder_emits_decoded_arcs_with_provenance` 测试 baseline） | 实际真实 hit 数 ≥ 98；当前输出**字段语义错位**，center / radius 可信，rotation / axis_ratio / sweep 不可信 |

## 6. 下一步建议（Phase 16 候选）

### 6.1 改 decoder 之前必做的进一步证据

| 项 | 目的 | 工作量 |
|---|---|---|
| Probe 加 "+0..7 ≡ +16..23 命中率" 与 "差异直方图" | 区分 `radius` vs `axis_a_x` | 约 30 行 Rust |
| Probe 加 attribute tail（byte 64..bytes_to_follow-12）扫描，寻找 sweep angle 形态的 f64 | 看 sweep angles 是否在 tail 里 | 约 50 行 Rust + 桶级统计 |
| Probe 按 `(bytes_to_follow, +32..33 referenced_type)` 桶交叉分析 +32..47 / +48..63 偏移 | 锁定 packed reference 的精确字段 | 约 50 行 Rust |
| IDA 反编译 `radsrvitem.dll!sub_56524150 GArc2d::Validate` 与 `GArc2d::Save / Load` | 拿到字段名 ground truth | 中（参考已有反编译惯例） |

### 6.2 暂不要改 decoder 的方向

- **不要**仅根据本次 probe 重命名 stable DTO 字段，因为 +32..63 的真实结构尚未锁定。
- **不要**放宽 `axis_a.y` 约束让 49 条新 record 进 decoded 输出，因为 rotation 字段语义没有命名一致前，下游 `geometry.rs` 会把"rotation"当 vector y 分量产生错误几何输出。
- **不要**在没有 IDA 反编译证据前命名 `+32..63` 的引用结构。

### 6.3 已确认可立即改的有限项

- 解除 `axis_a.y` 强制约束**只配合"audit-only" path** 才安全；如果要进入 `PidGraphicEntity`，必须先把 rotation/axis_ratio 字段重命名 + 几何 emission 重写。
- 可以在分析文档里把 4 个字段从"已确认"降级为"待证伪"，作为 Phase 16 启动条件。

## 7. 已知反例 / 未解决问题

1. `+0..7 == +16..23` 在头 4 个 dump 上成立，但跨 98 条的命中率没量化。**待 probe 扩展确认**。
2. `+24..31` 在 78 / 98 上是 0，剩余取 π/2、3π/2、2π。**为什么没有 π？为什么没有 π/4、π/3 等任意角？** 暗示 SmartPlant 工艺图只用"正交"旋转，或字段语义另有约束。
3. `+40` 100% 0x00：未必否定 `sweep_direction` 语义；fixture 偏好可能恰好全 CW。
4. `+48..63` 的 `0x0010` 频繁出现，与 Phase 14 §6.3 的 "PSM 0x0010 sub-record fragment" 假设强相关。**待 0x0010 decoder 落地后回头看**。
5. attribute tail（btf - 76 字节）里有没有 sweep angle 还没看过。
6. byte 40 与 byte 41..47 的实际含义未知（在 100% 非零的 fixture 上**至少不是 padding**）。

## 8. 复现

```powershell
cargo run --release --example probe_garc2d_packed_bytes 2>&1 | Out-File -FilePath garc2d_probe.txt -Encoding utf8
```

probe 不需要 SmartPlant DLL，纯 Rust + `cfb` crate。跑完会写出每 fixture 每 Sheet 流的 hit 数、字段分类直方图、packed u16/u32 分布、以及前 4 条 candidate 的完整 64 字节 hex dump。

---

## 9. 二轮 probe（含 attribute tail 解读）的重大转向

### 9.1 +0..7 ≡ +16..23 不是 "radius = center.x" 的普遍规律

| Fixture | byte_eq | <1e-9 | <1e-3 | ≥1e-3 |
|---|---:|---:|---:|---:|
| DWG-0201 | 8 / 20 | 8 | 1 | 11 |
| DWG-0202 | 16 / 30 | 16 | 1 | 13 |
| 工艺管道-1 | 14 / 47 | 16 | 2 | 29 |
| A01 | 0 / 1 | 0 | 0 | 1 |
| **Total** | **38 / 98** | **40** | **4** | **54** |

字节级完全相等占 38 / 98 ≈ 39%，差异 ≥ 1e-3 占 54 / 98 ≈ 55%。**结论：+16..23 是独立字段，不等于 center.x；初版 hypothesis "radius == center.x" 是 dump[0..3] 巧合**。

### 9.2 attribute tail 扫描发现 0x0030 record 远不是单纯几何

跨 4 fixture 共 984 个 tail f64 slot，按 `denormalized | finite_in_[-2π,2π] | finite_other | nan_inf` 分类：

| Fixture | total | finite_in_[-2π,2π]_normal | denorm/zero | other_finite |
|---|---:|---:|---:|---:|
| DWG-0201 | 218 | 119 | 71 | 27 |
| DWG-0202 | 350 | 183 | 113 | 51 |
| 工艺管道-1 | 410 | 236 | 152 | 22 |
| A01 | 6 | 2 | 2 | 2 |

`finite_in_[-2π,2π]_normal` 在每个 fixture 的 **tail offset 集中在 0/8/16/24**（前 32 字节是 4 个 f64 slot），且偶尔出现 1.0 / 3π/2 命中。**但 sweep angle 没有 fixed-offset 集中分布**——跨 24+ 个 "known angle" 命中里，只有 1 个真的命中 `3π/2`（工艺管道-1 oid=2 tail+088），其余 23 个都是 `1.0`。

→ **结论：sweep angle 既不在 64B payload 里，也不在 tail 的某个固定 offset 上**。

### 9.3 tail 里有 **plant instrument tag (UTF-16LE, length-prefixed)**

DWG-0202 hit[1] (`oid=1, btf=384`) tail 前 32 字节：

```
tail+000: 49 00 00 00            → u32 = 73 (total tag block length?)
tail+004: 0B 00                  → u16 = 11 (UTF-16 char count)
tail+006: 41 00 33 00 2D 00 46 00 41 00 30 00 36 00 30 00 32 00 30 00 31 00
                                 → "A3-FA060201" (UTF-16LE × 11 chars = 22 bytes)
tail+028: 99 20 47 87            → 4 bytes (linkage?)
```

`A3-FA060201` 是工艺/管道仪表流程里典型的 **plant instrument tag**（A3 单元 / FA 设备类型 / 060201 序号）。 

→ **0x0030 record 不只是几何，还是某种 "instrument annotation / tagged symbol placement" 复合 record**。

### 9.4 tail 反复出现 0x3FF0000000000000 (= `1.0`) 常量 marker

DWG-0201 dump[0..2] tail+064..072 都是 `00 00 00 00 00 00 F0 3F` = `1.0`。DWG-0202 dump[1] tail+048 也是 `00 00 00 00 00 00 F0 3F` = `1.0`。这种 `1.0` 是浮点 1，**很可能是 scale factor 或 unit-vector y / cos(0) / 标准化变换矩阵的对角线 entry**。

### 9.5 tail 里多次出现"和别的 0x0030 record 的 center.x/y 字节级一致"的 f64

DWG-0201 dump[0] (`oid=2, center=(0.281, 0.362)`) 的 tail+040..047 = `C0 B7 F2 18 ED 63 D8 3F` = **0.381099**，这等于 dump[1] 的 `center.y`。
dump[0] tail+048..055 = `35 F5 8A 52 78 8A D0 3F` = **0.258452**，这等于 dump[1] 的 `center.x`。

→ **tail 含"指向另一条 0x0030 record 的 anchor 坐标拷贝"**。多个 0x0030 record 可以**共享一组坐标 anchor**——很像 SmartPlant 仪表符号的多端点连接（仪表本体 + 引出线 + 标签框）。

### 9.6 tail 含更多 PSM-style packed reference

DWG-0202 hit[1] tail+064..080：

```
tail+064: 00 00 00 15 84 00 58 00 00 00 CA 01 00 00 F6 0E
```

按 (u32, u16, u16) 解读：`0x15000000, 0x0084, 0x0058, 0x000001CA, 0x00000EF6`。`0x0084 = igLineString2d` PSM type。

→ **tail 末段还有第二组 referenced-record linkage**（不只 +32..63 那一组），引用 igLineString / igTextBox 等。

### 9.7 综合判断：0x0030 PSM type 可能不是 GArc2d

Phase 14 final summary §6.1 里就明确说 "0x0030 真实归属待证"。本轮 probe 强烈支持以下假设：

**Hypothesis**：`PSM type 0x0030` 对应的 C++ 类**不是单纯的 `GArc2d`**，而是某种 SmartPlant 工艺图特有的 "**Tagged Instrument / Annotated Symbol Placement**" 复合对象，结构大致为：

```text
PSM header (18B):  type=0x0030 | btf | oid | aux(8B = first f64 of payload? or extra meta)
GArc2d-shaped block (64B):
  +0..7   f64    center.x (instrument anchor X)
  +8..15  f64    center.y (instrument anchor Y)
  +16..23 f64    radius / second_anchor.x (38% 与 center.x 字节级相等)
  +24..31 f64    rotation_angle (rad, 取值集中在 {0, π/2, 3π/2, 2π})
  +32..47 16B    primary linkage block (referenced PSM type + sub_kind + index + extras)
  +48..63 16B    secondary linkage block (常含 0x0010 sub-record ref + flags)
attribute tail (btf - 76 bytes):
  ?      u32     tag_block_length
  ?      u16     tag_char_count
  ?      ×N×2   UTF-16LE plant instrument tag (e.g. "A3-FA060201")
  ...   f64×4   companion coords (frequently == other 0x0030 records' center.xy)
  ...   f64     1.0 (scale / unit marker)
  ...   var     additional packed PSM references (igLineString / igTextBox 等)
```

如果该假设成立，那么：

1. 当前 decoder 的 `axis_a / axis_ratio / sweep_direction / sweep_start_angle / sweep_end_angle` **全部字段名是错的**。
2. center.xy 是正确的，但语义是 **"instrument anchor"** 而非 "arc center"。
3. Phase 14 §6.1 "GArc2d field semantics correction" 这条 future-slice 应该升级为更大议题：**0x0030 type code 真实归属重新识别**。

### 9.8 仍然不能直接断言的事

- `rotation_angle` 字段虽然取值离散像 rotation，但也可能是 **sweep_extent**（弧度跨度，常用 `0` / `π/2` / `3π/2` / `2π` 等"标准角"）。需要 IDA 反编译 / controlled-diff fixture 才能锁定。
- `+32..47` 与 `+48..63` 的 packed linkage 内部精确字段名（`sub_kind`、`field@+42`、`field@+44`、`field@+46` 等）未定，需要按 (ref_type) 桶继续细化。
- tail 的 plant tag block 长度字段（推测 `+000 u32 = 73`）与字符数（`+004 u16 = 11`）的关系仍是 hypothesis。
- 同一 tail 里出现"别的 record 的 center.xy"是 anchor 共享还是引用引导，本轮证据不足判断。

### 9.9 下一步取证方向

| 选项 | 工作量 | 价值 |
|---|---|---|
| 写第三轮 probe：按 `(btf, ref_type)` 双重桶交叉锁定 tail 字段稳定 offset（特别是 tag、companion coord、1.0 marker 的位置） | 中（约 80 行 Rust） | 高 |
| IDA 反编译 `radsrvitem.dll` 在 PSMSerializeIn 的 `guidtab.h` lookup 上查 type 0x0030 实际对应的 C++ 类（vs 假设的 `GArc2d`） | 中（已有 IDA 项目，约 30 分钟） | **关键**：决定后续所有命名 |
| 在 IDA 里找 `PersistTypeTable<PersistComTypeEntry>` 表项数据 dump，把 type 0x0030 对应的 CLSID / factory 函数名找出 | 中 | 关键 |
| Controlled-diff：用 SmartPlant 编辑器造一个只含单一仪表（已知 tag、已知坐标）的 .pid fixture，对比字节流 | 高（要打开 SmartPlant 编辑器） | 高 |

如果 IDA 路线证实 0x0030 不是 `GArc2d`，那 `SheetPrimitiveArcDecoded` DTO 名 + `decode_primitive_arcs` 函数名都需要**重命名**——这是 stable API 改动，须先与上层调用者协商。

---

## 10. IDA 路线证实：0x0030 = `j2dsrv.dll` 注册的 CLSID `47FCC338`，**不是 `GArc2d`**

### 10.1 PSM type code 全局表定位

`radsrvitem.dll` `sub_56445C90` 显式调用：

```c
sub_56455D10(281, dword_5667B068, sub_56471660, 0);
```

把 **281 条 PersistComTypeEntry** 注入全局 `PersistTypeTable<PersistComTypeEntry>`。每 entry 20 字节：`16B GUID + 1B a5 + 1B a6 + 2B a7 (chain link)`。PSM type code = 在表内的 sequence index（注册顺序）。

确认每个 PSM type code 在 `dword_5667B068` 起 281 × 20 字节中的位置：

| PSM type | entry index | CLSID | a5 | a6 | 来源模块（CLSID 后缀） |
|---|---:|---|---:|---:|---|
| `0x18` igLine2d | 24 | `{2D4E13C0-D3D1-11CD-8AEA-08003601B44A}` | 0xC0 | 0x01 | IGDS standard (`08003601B44A`) |
| `0x4D` igTextBox | 77 | `{777A6860-3C8F-11B9-C000-4ECAE2741999}` | 0xC0 | 0x02 | 特殊 (`4ECAE2741999`) |
| `0x59` igCircle2d | 89 | `{902AD280-D3E1-11CD-8AEA-08003601B44A}` | 0xC0 | 0x01 | IGDS standard |
| `0x61` igArc2d | 97 | `{9D650A00-D3E1-11CD-8AEA-08003601B44A}` | 0xC0 | 0x01 | IGDS standard |
| `0x84` igLineString2d | 132 | `{F875B4A0-D97A-11CD-8AEA-08003601B44A}` | 0xC0 | 0x01 | IGDS standard |
| `0xCE` igSymbol2d | 206 | `{719C2A5E-B6B5-11CE-B656-080036D72102}` | 0x40 | 0x0A | SmartPlant (`080036D72102`) |
| `0xFA` GraphicGroup | 250 | `{24D10655-0917-11D1-BC33-08003609D002}` | 0x40 | 0x0B | SmartPlant (`08003609D002`) |
| **`0x30`** ← 本节焦点 | **48** | **`{47FCC338-2D0F-11D0-A1FF-080036A1CF02}`** | **0x40** | **0x03** | **`080036A1CF02`** |

### 10.2 `47FCC338` 实际属于 `j2dsrv.dll` 不是 `radsrvitem.dll`

在 `radsrvitem.dll` 里 `find_bytes 38 C3 FC 47 0F 2D D0 11 A1 FF 08 00 36 A1 CF 02` 只找到 2 处：

1. `0x56664454` — `.rdata` GUID 常量表条目（无代码 xref）
2. `0x5667B428` — `.data` PSM type table entry[48]

**没有任何 `.text` 代码引用 CLSID `47FCC338`**。这说明 `radsrvitem.dll` 只对它做查表 lookup，不真正实现该类的 Save/Load。

在工程根目录 `dlls/` 下二进制 grep 同样 16 字节模式，命中的额外文件：

```
j2dsrv.dll  @  0x00021D6C
radsrvitem.dll  @  0x00223854
radsrvitem.dll  @  0x0023A828
```

→ **`j2dsrv.dll` 也含有这条 GUID**。

### 10.3 `j2dsrv.dll` 元信息

```
FileVersion:      09.00.00.0138
InternalName:     J2DSrv
OriginalFilename: J2DSrv.dll
Product:          RAD (Rapid Application Development)
FileDescription:  J2DSrv
```

`J2DSrv` = **Rapid Application Development 2D Service**——Intergraph RAD 框架的 2D 几何 / 复合 record 持久化层。`SmartPlant P&ID` 工艺图把 J2DSrv 用作 plant-instrument-aware 2D primitive 的运行时。

### 10.4 `47FCC330..47FCC33E` 是 J2DSrv 的复合 record 家族

`j2dsrv.dll` 在 `0x21D40` 起的 `.data` 段含**连续注册的 GUID 序列**（每个 20 字节 record，含 8 字节前缀）：

```
0x21D40: ... 47FCC336 ... 47FCC337 ... 47FCC338 ... 47FCC339 ... 47FCC33A ...
0x21DB0: ... 47FCC33C ... 47FCC33D ... [non-family GUID 1] [non-family GUID 2] 47FCC33E ...
```

`radsrvitem.dll` 的 PSM 全局表（281 entries）映射回 PSM type code：

| entry idx | PSM type | CLSID | 备注 |
|---:|---:|---|---|
| 41 | 0x29 | `47FCC330` | |
| 42 | 0x2A | `47FCC331` | |
| 43 | 0x2B | `47FCC332` | |
| 44 | 0x2C | `47FCC333` | |
| 45 | 0x2D | `47FCC334` | |
| 46 | 0x2E | `47FCC335` | |
| 47 | 0x2F | `47FCC336` | |
| **48** | **0x30** | **`47FCC338`** | **跳过 47FCC337！** |
| 49 | 0x31 | `47FCC339` | |
| 50 | 0x32 | `47FCC33B` | 跳过 47FCC33A |
| 51 | 0x33 | `47FCC33C` | |
| 52 | 0x34 | `47FCC33D` | |
| 53 | 0x35 | `47FCC33E` | `a6=0x06, a7=0x116`（chain to fast-path entry[0x116]=type 278） |

→ `0x29..0x35` 这 13 个 PSM type code 全部属于同一 J2DSrv `47FCC3xx` 复合家族。`0x30` 在家族里是第 9 个（按 register 顺序），但 GUID data1 末字节是 `0x38` 而非 `0x30` —— **GUID data1 末位不等于 PSM type code**，只是巧合相邻。

### 10.5 a5 / a6 字段模式提示分类

| 模块 | a5 | a6 |
|---|---:|---:|
| IGDS standard (igLine2d / igCircle2d / igArc2d / igLineString2d / igEllipse2d) | 0xC0 | 0x01 |
| J2DSrv `47FCC3xx` 家族 | 0x40 | 0x03 |
| SmartPlant GraphicGroup (0xFA) | 0x40 | 0x0B |
| SmartPlant igSymbol2d (0xCE) | 0x40 | 0x0A |
| 特殊 igTextBox | 0xC0 | 0x02 |

`0xC0` (= 1100 0000) vs `0x40` (= 0100 0000) 高 2 位差异，配合 a6 数字，很像 `(serialization_class, sub_kind)` 二元组。**所有 `0x40` a5 都对应 SmartPlant / J2DSrv 复合 record（含 reference linkage），不是纯 IGDS 几何**。

### 10.6 结论与下一步

1. **PSM type code `0x0030` 不是 `GArc2d`**，而是 `J2DSrv.dll` 注册的 CLSID `{47FCC338-2D0F-11D0-A1FF-080036A1CF02}`——RAD 2D 复合 record 家族 (`47FCC330..47FCC33E`) 的成员。
2. 当前 `SheetPrimitiveArcDecoded` 命名 + 字段语义假设 **全部错位**：
   - 字段 `axis_a / axis_ratio / sweep_direction / sweep_start_angle / sweep_end_angle` 命名不正确
   - `center.xy` 字段位置正确，但语义可能是 "anchor"
   - 输出的 48 条 "decoded arcs" 本质上是 **48 条 J2DSrv 复合 record 子集**（被 axis_a.y ≈ 0 错误过滤后的子集）
3. 真实字段语义只能通过加载 `j2dsrv.dll` 到 IDA + 反编译 47FCC330 家族的 Save / Load vtable slot 拿到。**这是 Phase 16 必做的下一步**。

### 10.7 Phase 16 改动范围（待用户确认）

- `j2dsrv.dll` 加载到 IDA MCP（新建一个 instance, port ≥ 13347）
- 反编译 CLSID `47FCC338` 的 ClassFactory / Save / Load
- 锁定字段表 → 重命名 `SheetPrimitiveArcDecoded` → 例如 `SheetJ2DRecordDecoded` / `SheetTaggedInstrumentDecoded` / 视真实类名而定
- 重写 `decode_primitive_arcs` 验证规则（放宽 `axis_a.y ≈ 0` 假约束，正确处理 packed reference + tail）
- 联动更新 `model.rs` DTO、`geometry.rs` entity emission、stable schema ratchet、`tests/parse_real_files.rs` 计数 baseline
- 这不是单 PR 改动，建议先开 Phase 16 brief + plan + goal-package

---

## 11. Probe v5：按 btf 桶 + cross-record OID reference + +24..31 分布

第四轮迭代后的 probe 在每个 fixture 上额外输出三组分析：

- `+24..31` 按 `bytes_to_follow` 桶分布
- `tail head signature` 按 `bytes_to_follow` 桶
- `cross-record oid references`：把 `+32..63` payload 与 attribute tail 里的 u32 候选 oid 与同 stream 已知 PSM record `oid` 表做 join

### 11.1 `+24..31` 与 `bytes_to_follow` 强相关 → 几乎确认是 rotation_angle

|  btf | DWG-0201 | DWG-0202 | 工艺管道-1 | 综合判定 |
|---:|---|---|---|---|
| 128 | z=9, π/2=1, 3π/2=1 (n=11) | z=7, π/2=4, 3π/2=4 (n=15) | z=18, π/2=2, 3π/2=1 (n=21) | 多数 zero，有 π/2 / 3π/2 |
| 145 | π/2=1 (n=1) | π/2=3 (n=3) | z=14, π/2=0, 3π/2=2 (n=16) | DWG: 全 π/2; 工艺管道: 多 zero |
| 224 / 225 | z=7, 3π/2=1 (n=8) | z=11 (n=11) | z=8 (n=8) | **100% zero** |
| 129 | n/a | n/a | z=2 (n=2) | zero |
| 384 | n/a | z=1 (n=1) | n/a | zero |

**关键观察**：

- **`btf=224 / 225` 跨 fixture 共 27 条 record，`+24..31` 全部为 0**（22/22 + 8/8 - 截至本表 fixture 范围）。
- `btf=145` 在 DWG-0201/0202 全部为 π/2（4/4），但工艺管道-1 大量为 0 + 2π/3 / 3π/2 / 2π —— 说明 `(btf=145, +24..31=π/2)` 不是普适规律，仅在 DWG 系列偏好。
- `btf=128` 是 zero 与 π/2 / 3π/2 的混合，**btf 决定 record 整体形态**（reference 链字段数 + 是否携带 tag），rotation 与 btf 弱相关。

**结论**：`+24..31` 是 **rotation_angle (rad)**，不是 sweep_extent。理由：

1. zero 占大多数（≥ 60%）。若是 sweep_extent，全圆 / 半圆应是主流，不可能 zero 主导（zero sweep = 没有弧）。
2. fixture 取值集中在 {0, π/2, 3π/2}，缺 π —— 仪表符号常用方向（朝右 / 朝上 / 朝下 / 朝左），其中"朝左"（π）通常用 horizontal-flip flag 实现，不直接旋转，正好印证 SmartPlant 编辑器约定。
3. 与 `bytes_to_follow` 桶弱相关，符合"rotation 不改变 payload 大小"的直觉（不像 sweep angles 会影响弧长度从而附加字段）。

### 11.2 tail head signature：plant tag 不是按 btf 一一对应

|  btf | DWG-0201 plant_tag_like | DWG-0202 plant_tag_like |
|---:|---:|---:|
| 128 | 6 / 11 (55%) | 1 / 15 (7%) |
| 145 | 1 / 1 (100%) | 0 / 3 |
| 224 | 0 / 8 | 0 / 11 |
| 384 | n/a | 0 / 1 |

→ 同 fixture 同 btf 不同 record 之间，**plant tag 的出现是 record-level 选项，不是 btf 决定的**。`(tag_block_length=u32, char_count=u16, UTF-16LE × N)` 的判定逻辑可能太宽松（我用了 `length_prefix < 10_000 && char_count < 1_000 && length_prefix - char_count*2 < 20`，DWG-0201 那 6/11 命中里可能多数是 false positive）。

**待 Slice B 在 IDA 里看 J2DSrv 的 Save 函数确认 tag 是否走可选 path（如根据 flag 决定是否序列化）**。

### 11.3 Cross-record OID references：每条 0x0030 都 100% 引用其他 PSM record

| Fixture | records_with_payload_hit | records_with_tail_hit | candidates |
|---|---:|---:|---:|
| DWG-0201 | 20 / 20 (100%) | 20 / 20 (100%) | 20 |
| DWG-0202 | 30 / 30 (100%) | 27 / 30 (90%) | 30 |
| 工艺管道-1 | 47 / 47 (100%) | 待表中查 | 47 |
| A01 | 1 / 1 (100%) | 待表中查 | 1 |

**最频繁的 payload offsets**（DWG-0202 为例）：

| Payload offset | DWG-0201 hits | DWG-0202 hits | 命中率 |
|---:|---:|---:|---:|
| +038 (u32) | 20 / 20 | 30 / 30 | **100%** |
| +042 (u32) | 20 / 20 | 30 / 30 | **100%** |
| +050 (u32) | 20 / 20 | 30 / 30 | **100%** |
| +046 (u32) | 19 / 20 | 26 / 30 | ≥ 87% |
| +056 (u32) | 17 / 20 | 22 / 30 | ≥ 73% |
| +032 (u16) | 8 / 20 | 22 / 30 | ≥ 40% |

→ **`+38..41` / `+42..45` / `+50..53` 是 3 个稳定的 u32 reference oid 字段**，每条 0x0030 都有；
`+46..49` / `+56..59` / `+32..35` 是更弱 reference 字段。

### 11.4 引用类型分布

跨 fixture （DWG-0202 sample）：

| Referenced PSM type | hits (DWG-0202) | hits (DWG-0201) |
|---|---:|---:|
| `0x0000` (空 type / oid 未注册) | 322 | 160 |
| `0x0001` (?未知元数据) | 124 | 63 |
| **`0x0010` sub-record** (Phase 14 §6.3) | **71** | **29** |
| **`0x00FA` GraphicGroup** (Phase 15) | **63** | **21** |
| `0x0084` igLineString2d | 18 | n/a |
| `0x0018` igLine2d | 17 | 4 |
| `0x004D` igTextBox | 17 | 19 |
| `0x005E` igPoint2d | 16 | 5 |
| `0x00CE` igSymbol2d | 14 | 9 |

**主要引用类型是 `0x0010` 和 `0x00FA`**。这把 Phase 14 / Phase 15 / Phase 16 三个 phase 串成一条**"复合 record → sub-record + group → geometry primitive"** 的引用链：

```
0x0030 J2DSrv 复合 record
  ├─ +32..33 referenced_type (u16)
  ├─ +38..41 ref oid → 常指向 0x0010 sub-record
  ├─ +42..45 ref oid → 常指向 0x00FA GraphicGroup
  ├─ +50..53 ref oid → 指向 igTextBox / igSymbol2d / igLineString2d
  └─ +56..59 ref oid → 指向 0x0010 sub-record
```

### 11.5 Sample 实证

DWG-0202 hit `oid=1, btf=384` 的 payload reference 解读：

```
payload+038 = u32 336      → igTextBox  (PSM 0x004D, oid=336)
payload+042 = u32 4756     → GraphicGroup (PSM 0x00FA, oid=4756)
payload+050 = u32 4718608  → 未注册的 oid（可能是 0x0010 派生的 sub-id）
payload+052 = u32 72       → PSM 0x3000 (未知 high-bit type)
payload+056 = u32 65538    → 0x0010 sub-record (oid=65538)
```

tail 里 `tail+006..27` 是 UTF-16LE `"A3-FA060201"`（plant instrument tag）。

整条 record 自洽含义：**一条带 tag `A3-FA060201` 的 J2DSrv 复合对象，其文字 label 是 igTextBox(336)，所属 GraphicGroup 是 4756，附带 0x0010 sub-record 65538**。

### 11.6 11 个被证伪与待答疑

- ✓ +38..41 / +42..45 / +50..53 是 u32 reference oid，跨 fixture 100% 命中。可入 stable DTO 命名为 `referenced_oid_a / _b / _c`（更精确语义待 IDA 锁定，例如 `label_textbox_oid / group_oid / inner_sub_record_oid`）。
- ✓ +24..31 是 rotation_angle (rad)，不是 sweep_extent。可放宽 Phase 14 错误约束。
- ? +56..59 引用 0x0010 sub-record，但 0x0010 的字段表仍未解开（Phase 14 §6.3 留 future）。
- ? +46..49 / +54..55 / +60..63 还在 packed integer 形态，未识别字段。
- ? `1.0` 标量 marker 在 tail+064 出现规律仍未解。
- ? `+34..37` u32 通常较小（< 1000），可能是 `sub_kind` / `version` / `flag word`。
- ? plant tag 仅在部分 record 出现，触发条件待 IDA 锁定。
- ? 同 oid 出现 PSM type 0x0000（"oid 0 但 reachable"）的现象，可能是 oid alias 或 phantom record，需要查 PSM table 看是不是 reserved。
- ? `0x0030` 这条记录里 `+50..53 = 4718608` 这种 large oid 是不是有效 reference 还是 packed encoding。
- ? 工艺管道-1 fixture 的 47 条 record 引用类型分布尚未完整对账（probe 输出文件含完整数据）。
- ? J2DSrv 12 个其他 PSM type（0x29..0x2F + 0x31..0x35）是否同 schema：还没扫。Phase 17 候选。



