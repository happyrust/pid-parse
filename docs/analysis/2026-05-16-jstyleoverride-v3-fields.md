# PSM type `0x0030` (JStyleOverride V3) 权威字段表

> 日期：2026-05-16  
> 上游：`docs/analysis/2026-05-15-garc2d-packed-int-tail.md` §1-§11  
> IDA 反编译 dlls：`radsrvitem.dll` (port 13346) / `J2DSrv.dll` (port 13347) / `style.dll` (port 13348) / `JUTIL.dll` (静态 PowerShell)  
> Phase 16 IDA Slice A + B 总成果

## TL;DR

**PSM type code `0x0030` 真实身份 = `JStyleOverride` (style.dll RAD 7.0.0.108)**，**不是 `GArc2d`**。

序列化字段表（**Version 3**，跟 fixture 64 字节 payload 完美匹配）：

```
disk offset  size  type  field source            可信度
+0..3        4B    u32   this+22                 IDA ✅
+4..7        4B    u32   this+24                 IDA ✅
+8..11       4B    u32   this+25                 IDA ✅
+12..15      4B    u32   this+38                 IDA ✅
+16..23      8B    f64   this+26                 IDA ✅ probe v5 印证
+24..31      8B    f64   this+28 (rotation_angle) IDA ✅ probe v5 印证
+32..39      8B    f64   this+30                 IDA ✅
+40..47      8B    f64   this+34                 IDA ✅
+48..51      4B    u32   this+32                 IDA ✅
+52..55      4B    u32   this+47                 IDA ✅
+56..59      4B    u32   this+48                 IDA ✅
+60..61      2B    u16   this+36                 IDA ✅
+62..63      2B    u16   byte+146                IDA ✅
TOTAL: 64 字节
```

## 完整发现链

```
Phase 14 假设: decode_primitive_arcs / GArc2d (8 doubles)
                              ↓ ❌ 错误
radsrvitem.dll PSM table[48] → CLSID {47FCC338-2D0F-11D0-A1FF-080036A1CF02}
                              ↓
J2DSrv.dll @ sub_10001AB0 → 消费者 (用 JCoCreateInstance 创建实例)
                              ↓
SmartSketch 用自定义 JCoCreateInstance (来自 JUTIL.dll，非 Windows OLE 注册表)
                              ↓
JUTIL.dll @ 0x35680: RAD CLSID 注册表 (64B/条目)
   47FCC338 → style.dll : "JSL Override Style"
   47FCC339 → style.dll : "JSL Offset Line Generator"
   47FCC33A → style.dll : "JSL Bitmap Style"
                              ↓
style.dll DllGetClassObject (47FCC338 分支)
   → sub_10001600 → ClassFactory<JStyleOverride>::vftable @ 0x100697D4
                              ↓
✅ 真实 C++ 类名 = JStyleOverride
   继承链: JStyleOverride → JStyleR2d → JStyleBase
                              ↓
JStyleBase::IJPersistImp::Save/Load
   slot 5,6 = sub_10056DC0 (thunk to host vtable slot 32)
                              ↓
host vtable slot 32 = sub_10057B30 (version dispatcher)
   if version 2: 调 host slot 17 → JStyleOverride main vtable slot 17 = sub_1000F210 (14 DoIO 68B)
   if version 3: 调 host slot 52 → JStyleOverride main vtable slot 52 = sub_1000F030 (13 DoIO 64B) ✅ fixture
                              ↓
sub_1000F030 序列化 13 个字段，磁盘 64B
```

## probe v5 §11 字段假设对账

| Disk | probe v5 §11 hypothesis | IDA V3 实际 schema | 评价 |
|---|---|---|---|
| +0..3 | f64 center.x 前半 | u32 A | ⚠ schema 冲突 |
| +4..7 | f64 center.x 后半 | u32 B | ⚠ |
| +8..11 | f64 center.y 前半 | u32 C | ⚠ |
| +12..15 | f64 center.y 后半 | u32 D | ⚠ |
| +16..23 | f64 secondary anchor | f64 #1 ✅ | 一致 |
| +24..31 | f64 rotation_angle ({0, π/2, 3π/2}) | f64 #2 ✅ | 一致 |
| +32..39 | "packed int" | f64 #3 | probe denormalized 假象 |
| +40..43 / +44..47 | "various packed" | f64 #4 | probe denormalized 假象 |
| +48..51 | "ref_oid_c" | u32 E | 可能 reference oid |
| +52..55 | "various" | u32 F | 可能 reference oid |
| +56..59 | "ref_sub_record_oid" | u32 G | 可能 reference oid |
| +60..61 | "various" | u16 H | u16 |
| +62..63 | "various" | u16 I | u16 |

### 关键冲突 (待 controlled-diff 解决)

probe v5 把 fixture `+0..7` 解读为 f64 = 0.281208 (像归一化 center.x)，把 `+8..15` 解读为 f64 = 0.362367 (像归一化 center.y)。这些值字节模式形如标准化 f64：

```
dump[0] +0..7 bytes: B8 6A B7 AD 4E FF D1 3F (LE)
              u64 join: 0x3FD1FF4E_ADB76AB8 = f64 0.281208 ✓ normalized
              u32 split: A=0xADB76AB8, B=0x3FD1FF4E
```

按 IDA Version 3 schema，这是 2 个独立 u32 字段，但**字节模式天然形如 f64**。三种可能：

1. **RAD 内存 alias**：`this+22` 和 `this+23` 在内存中作为 f64 的两个 32-bit halves。但 V3 的 DoIO 跳过 `this+23`（只 IO `this+22` 和 `this+24`），所以这个假说不成立。
2. **fixture 使用更高版本 (V4/V5)**：可能存在另一个 dispatch path（slot 53 = 0x10070568 远地址，可能是 V4 handler），fixture 实际走该路径。需进一步反编译 vtable slot 53。
3. **fixture 的 u32 字段值碰巧形如归一化 f64**：4 个 u32 字段恰好是 oid / count / flag 类整数，字节模式恰巧落在 f64 normalized 区间。可能性最低。

最稳的下一步取证：
- 反编译 vtable slot 53 (sub_10070568 / 远地址)
- 用 fixture controlled-diff 协议造已知 plant tag 的 record，对比字节

## 反向工程 IDA 地址索引

| 符号 | 地址 | 备注 |
|---|---|---|
| `radsrvitem.dll!PersistTypeTable<PersistComTypeEntry>` 表 | `dword_5667B068` | 281 entries × 20B |
| `entry[48] = type 0x0030` | `dword_5667B068 + 960 = 0x5667B428` | CLSID 47FCC338 |
| `JUTIL.dll RAD CLSID 注册表` | file offset `0x35680` | 64B/条目 |
| `J2DSrv.dll CLSID 47FCC338 引用` | RVA `0x100145F8` (xref from sub_10001AB0) | 消费者 |
| `style.dll CLSID 47FCC338` | RVA `0x10066B64` | DllGetClassObject 分支 |
| `style.dll!ClassFactory<JStyleOverride>::vftable` | `0x100697D4` | first entry sub_10001600 |
| `style.dll!sub_10001600` (ClassFactory ctor) | sets vtable to ClassFactory<JStyleOverride> |
| `style.dll!JStyleOverride::vftable` | `0x1006A52C` | main vtable，50+ slots |
| `style.dll!JStyleBase::vftable` | `0x1006E87C` | parent vtable |
| `style.dll!JStyleBase::IJPersistImp::vftable` | `0x1006E9AC` | 16 slots, IJPersist thunk |
| `style.dll!sub_10056DC0` | IJPersistImp slot 5,6 thunk → host slot 32 |
| `style.dll!sub_10057B30` | host slot 32 = version dispatcher |
| **`style.dll!sub_1000F210`** | **JStyleOverride main vtable slot 17 = V2 IO (14 DoIO 68B)** |
| **`style.dll!sub_1000F030`** | **JStyleOverride main vtable slot 52 = V3 IO (13 DoIO 64B = fixture)** |
| `style.dll!sub_10055F30` | V3 header (writes version word to a3+18 bits 0-2) |
| `style.dll!HCreateOverrideStyle` | `0x10043500` | factory helper |
| `style.dll!HGetOverrideStyle` (3 overloads) | `0x10043920` / `0x10043C50` / `0x10044160` | get/clone |
| `style.dll!HloadOverrideData` | `0x10041A60` | in-memory copy with flag bitmask (parent class) |

## 已知遗留问题

| ID | 描述 | 解决路径 |
|---|---|---|
| Q-conflict | probe v5 +0..15 解读为 2×f64 vs IDA V3 解读为 4×u32 | 反编译 vtable slot 53 (V4 candidate) 或 Plan B controlled-diff |
| Q-ref-oid | probe v5 看到 +38/+42/+50 是 100% 命中其他 record 的 oid，但 IDA V3 schema 把它们标为 f64 字段 | 同上 |
| Q-tag | DWG-0202 oid=1 (btf=384) tail 含 `"A3-FA060201"` plant tag，但其他 record 大多不含 | 反编译 sub_100572C0 (V3 header 末段) 或 controlled-diff |
| Q-rotation-vs-sweep | +24..31 ∈ {0, π/2, 3π/2}，命名 `rotation_angle` 还是 `sweep_extent`？ | 用 IDA HGetOverrideStyle 看字段语义引用 |
| Q-tail | btf > 64 时尾部 12-320 字节是什么？plant tag + linkage chain？ | 待 Slice E (cluster 接入时) |

## Phase 16 建议

基于以上证据，Phase 16 改动建议：

1. **重命名 DTO**：`SheetPrimitiveArcDecoded` → 候选名：
   - `SheetJStyleOverrideDecoded` (按真实 C++ 类名)
   - `SheetStyleOverrideRecordDecoded` (按 RAD friendly name)
   - `SheetAnnotationRecordDecoded` (按 SmartPlant 实际使用语义)
2. **重命名 decoder**：`decode_primitive_arcs` → `decode_jstyle_override_records` 或 `decode_annotation_records`
3. **删掉错误约束**：移除 `axis_a.y.abs() <= 1e-6` 过滤（导致 51% 假阴性）
4. **stable DTO 字段**（基于双重证据：IDA + probe）：
   - `byte_range / type_code / type_flags / bytes_to_follow / oid` (Phase 14 已有)
   - `anchor: (f64, f64)` (= disk +16..31, 即 IDA f64#1 + f64#2)
   - 注：disk +0..15 因 schema 冲突未解决，**先保留为 raw 4 × u32**
   - **audit-only**: f64#3, f64#4, u32 E/F/G, u16 H/I 与 raw_tail (Vec<u8>)
5. **PidGraphicKind variant**：与用户拍板是否新增 `Annotation` / `Instrument` / `StyleOverride` variant，还是走 audit-only 不进 `PidGraphicEntity`
6. **Cross-fixture baseline**：放宽 `axis_a.y ≈ 0` 后跨 fixture decoded 数应 ∈ [90, 98]

## 复现

```powershell
# 跑 probe（已 v5）
cargo run --release --example probe_garc2d_packed_bytes 2>&1 |
    Out-File -FilePath C:\Users\dpc\AppData\Local\Temp\garc2d_probe.txt -Encoding utf8

# IDA MCP 切到 style.dll (port 13348)
# 反编译 V3 IO:
#   - sub_1000F030 (slot 52, fixture path, 64B)
#   - sub_1000F210 (slot 17, V2 alt path, 68B)
#   - sub_10057B30 (host slot 32 dispatcher)
#   - sub_10055F30 (V3 header)
```
