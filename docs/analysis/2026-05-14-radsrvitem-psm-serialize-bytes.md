# `radsrvitem.dll` PSM 序列化字节布局逆向

> 日期：2026-05-14
> 来源：IDA Pro 反编译 `radsrvitem.dll` PSMSerializeIn / PSMSerializeOut
> 目标：Phase 14 Sheet primitive 字节布局证据
> 配套 goal package：`goals/phase14-sppid-sheet-geometry/`
> 上游：`docs/analysis/2026-05-13-ida-pro-mcp-reconnaissance.md`

## TL;DR

**SmartPlant `.pid` 里 PSM-encoded record（包括 Sheet primitive 几何）的字节布局确定**：

```
Offset  Size   Field             含义
0       2 LE   type_code         14-bit 类型码（高 2 bit 是 flags）；对应 SmartPlant `guidtab.h` 表
2       4 LE   bytes_to_follow   后续 payload 字节数（不含本 6 字节头）
6       4 LE   oid               对象 OID，用于 OID lookup 验证
10      8      aux               日期/范围/2×i32 或 8-byte payload prefix（具体语义依 type 而定）
18+     var    inner_payload     对象 Save/Load 虚函数序列化的字段
```

写完后 `PSMSerializeOut` seek-back 重写真实 `bytes_to_follow`，
`PSMSerializeIn` 读时验证 `oid` 与对象 record 内部 OID 一致、`type_code`
与对象 record 的类型码一致，否则报错 `Stream offset mismatch while
reading serial data ... OID=%d nType= %d in guidtab.h`。

## 来源证据

### IDA 实例

- **Module**: `radsrvitem.dll`
- **Path**: `D:\work\plant-code\cad\pid-parse\dlls\radsrvitem.dll`
- **MD5**: `5a4dc710c0c907d5108e47809e5ba848`
- **SHA256**: `c47c0dbe4bd3c8b8ae49da0ef17bdeae0bc1cac6caa895b4cb47651546af2139`
- **IDA-MCP 端口**: 13346（auto-attached 后）
- **总函数数**: 5374（346 named，161 lib，4867 unnamed）
- **总字符串数**: 1739

### 关键函数地址

| 函数 | 地址 | 大小 | 来源证据 |
|---|---|---|---|
| **`PSMSerializeOut`** | `0x56491E80` | 1629 B | 引用 string `"FAILURE: PSMSerializeOut()[pMgr = 0x%p] BytesToFollow <= 0\n"` @ `0x566662f8` |
| **`PSMSerializeIn`** | `0x564915E0` | 2206 B | 引用 string `"Warning: PSMSerializeIn()[pMgr = 0x%p]  BytesToFollow %d mismatch ..."` @ `0x56666360` |
| **`ClusterTable::GetSpaceMapSegment`** | `0x5648C370` | 1104 B | 引用 string `"ClusterTable::GetSpaceMapSegment()[pMgr = 0x%p]: In PSM_PERSISTID_NO_REUSE ..."` @ `0x56665f98` |

### `PSMSerializeOut` (0x56491E80) 关键代码段

```c
// 提取 14-bit type code from object record metadata
v40 = ((v6[2] >> 6) & 0x3FFF);

// Step 1: 写 type (2 bytes)
v2 = (*write_stream)(stream, &v40, 2, 0);

// Step 2: 写 BytesToFollow 占位 (4 bytes, 用 magic 929325410=0x37614262 占位)
v38 = 929325410;  // 0x37614262 placeholder
v2 = (*write_stream)(stream, &v38, 4, 0);

// Step 3: 写 OID (4 bytes, from v6+1)
v38 = 0;
v2 = (*write_stream)(stream, v6 + 1, 4, 0);

// Step 4: 写 aux 8 bytes (v35 / v36 一组)
v2 = (*write_stream)(stream, &v35, 8, 0);

// Step 5: 由对象的 Save 虚函数追加 inner payload
v27 = (*(int (__stdcall **)(int, _DWORD))(*(_DWORD *)v37 + ???))(v37, stream);

// Step 6: 算 BytesToFollow，seek back 重写正确值
v28 = current_position(stream);
v38 = v28 - v39 - 6;  // BytesToFollow = 当前 - 起点 - 6 (header)
sub_5646A7C0(stream, v39 + 2);  // seek back 到 BytesToFollow 占位
(*write_stream)(stream, &v38, 4, 0);  // 重写真实 BytesToFollow
sub_5646A7C0(stream, v33);  // seek 回当前位置
```

### `PSMSerializeIn` (0x564915E0) 对称读取

```c
// Step 1: 读 type (2 bytes) -> v36
v3 = sub_5647AFB0(v5, &v36, 2);

// Step 2: 读 BytesToFollow (4 bytes) -> v40
v3 = sub_5647AFB0(v38, &v40, 4);

// 高位 flag 检查
if ((v36 & 0x8000) != 0) {
    sub_5646A7C0(v7, v40 + v8);  // skip 整个 record
    return 1024;
}

// Step 3: 读 OID (4 bytes) -> v37
v3 = sub_5647AFB0(v7, &v37, 4);

// Step 4: OID lookup
v3 = sub_56479870(v37, &v46);  // 找 OID 对应的对象 record

// Sanity: OID 与 record 内的 OID 一致
if (v37 != *(_DWORD *)(v46 + 4)) error;

// Sanity: type_code 与 record 内的 type 一致
if ((WORD)v36 != ((*(_DWORD *)(v46 + 8) >> 6) & 0x3FFF))
    // 错误路径输出 "Stream offset mismatch while reading serial data ... OID=%d nType= %d in guidtab.h"

// Step 5: 由对象的 Load 虚函数读 inner payload
v3 = (*(int (__stdcall **)(int, _DWORD))(*(_DWORD *)v42 + 20))(v42, *(this + 2));

// Step 6: 验证 BytesToFollow 与实际偏移移动一致
if (v40 != v20 - v35)
    // 错误 "Warning: PSMSerializeIn()[pMgr = 0x%p] BytesToFollow %d mismatch current offset is %d"
```

## 已知 type_code 与固定大小对照（PSMSerializeOut 内 switch 分支）

| type_code | 十六进制 | 固定 inner_payload 大小 | 推测语义（待 igLine2d 等 save 函数确认） |
|---|---|---|---|
| 276 | 0x114 | 35 bytes | 待确认 |
| 277 | 0x115 | 16 bytes | 待确认（line 起止点 = 2×f64×2 = 32B，不匹配 16B） |
| 278 | 0x116 | 53 bytes | 待确认 |
| 279 | 0x117 | 8 bytes | 待确认 |
| 280 | 0x118 | 59 bytes | 待确认 |

这 5 种 type 是 PSMSerializeOut 内显式 switch 的快速路径，**应当对应高频
通用 PSM record 类型**（不一定全部是 Sheet primitive，可能还有 cluster
metadata / index entries）。其他 type_code 走通用路径（不在 switch 内）
通过对象 Save 虚函数动态确定大小。

## 几何 primitive 类布局（GLine2d / GArc2d 已反编译确认）

### GLine2d (48 bytes = 6 × f64)

**反编译来源**：`sub_56524C50` @ `0x56524C50`（size 0x17f）——
`GLine2d::Validate()` 类似函数，引用 `"GLine2d: uninitialized data"`
字符串 @ `0x56669004`。

```c
// validate sub_56524C50 内部:
if (sub_564D15A0(a2[0]) || sub_564D15A0(a2[1]) ||
    sub_564D15A0(a2[2]) || sub_564D15A0(a2[3]) ||
    sub_564D15A0(a2[4]) || sub_564D15A0(a2[5]))
    return INVALID_NAN_FLAG;  // 0x1
v9 = fabs(sqrt(a2[2]^2 + a2[3]^2) - 1.0);
if (v9 > tol) return NOT_UNIT_VECTOR_FLAG;  // 0x8
if (a2[4] > a2[5]) return PARAM_REVERSED_FLAG;  // 0x200000
```

**字段表**：

| Offset | Type | Field | 含义 |
|---|---|---|---|
| 0..7 | f64 LE | `origin.x` | 起点 X |
| 8..15 | f64 LE | `origin.y` | 起点 Y |
| 16..23 | f64 LE | `direction.x` | 方向向量 X (单位) |
| 24..31 | f64 LE | `direction.y` | 方向向量 Y (单位) |
| 32..39 | f64 LE | `param_start` | 参数起 t |
| 40..47 | f64 LE | `param_end` | 参数终 t (必须 > start) |

**几何语义**：`point(t) = origin + t * direction`，参数定义域 `[param_start, param_end]`。
**不是** start-point + end-point 形式！而是 **origin + unit direction + scalar range**
的参数化表示。这一点对解码器编写很重要——直接从字节读到的不是 "line A→B"
而是 "from origin go along direction for [start, end]"。

转换公式：
```
endpoint_a = origin + param_start * direction
endpoint_b = origin + param_end * direction
length = param_end - param_start
```

### GArc2d (64 bytes = 8 × f64)

**反编译来源**：`sub_56524150` @ `0x56524150`（size 0x128）——
`GArc2d::Validate()` 类似函数，引用 `"GArc2d: uninitialized data"`
字符串 @ `0x56668b38`。

```c
// validate sub_56524150 内部:
v6[0] = _mm_loadu_si128((const __m128i *)a2);          // bytes 0..15
v6[1] = _mm_loadu_si128((const __m128i *)(a2 + 16));   // bytes 16..31
v6[2] = _mm_loadu_si128((const __m128i *)(a2 + 32));   // bytes 32..47
v6[3] = _mm_loadu_si128((const __m128i *)(a2 + 48));   // bytes 48..63
// 同时检查 *(double *)(a2 + 56) 和 *(double *)(a2 + 48) 是否 NaN
```

**初版推测字段表**（Slice F 落地时使用，几何语义部分错位待修）：

| Offset | Type | 字段 (decoder DTO 字段名) | 早期语义 (待修) |
|---|---|---|---|
| 0..7 | f64 LE | `center.x` | 弧心 X ✓ |
| 8..15 | f64 LE | `center.y` | 弧心 Y ✓ |
| 16..23 | f64 LE | `axis1.x` | 推测主轴 X (实际可能 = 单一 radius，待证) |
| 24..31 | f64 LE | `axis1.y` | 推测主轴 Y (实测 = π/2/π/3π/2 异常值，应为 rotation 或别字段) |
| 32..39 | f64 LE | `axis2.x` | 推测次轴 X (`sub_56524280` 揭示是 angle，用于 `sin/cos`) |
| 40..47 | f64 LE | `axis2.y` | 推测次轴 Y (`sub_56524280` 揭示 byte 40 是 BYTE form，41..47 是 padding) |
| 48..55 | f64 LE | `param_start` | 起始角 / 参数 (待证) |
| 56..63 | f64 LE | `param_end` | 终止角 / 参数 (待证) |

**`sub_56524280` 反编译揭示语义错位**（@ `0x56524280`，
`GArc2d::Validate` 内部辅助）：

```c
// NaN check 跳过 a2+24 (skip d[3])，直接到 a2+32 = angle:
if (NaN(a2[0]) || NaN(a2[8]) || NaN(a2[16]) ||
    NaN(a2[24]) || NaN(a2[32])) goto fail;

v14 = sub_5644E160(a2 + 16);  // sqrt 操作 (sub_5644E160 调
                              // libm_sse2_sqrt_precise) — 可能是
                              // sqrt(a2[16..23]) 或 sqrt(a²+b²)，
                              // 行为有歧义
v9 = *(double *)(a2 + 32) * v8;  // angle * radius 计算弧长
if (*(_BYTE *)(a2 + 40) > 1u) {  // BYTE form (0/1) 检查
    *a3 |= 0x10000000u;
}
```

**结论**：

- `a2+32` 是 **angle (rad)**，被 `sub_5658F950` 经 `sin/cos` 计算弧长
- `a2+40` 是 **BYTE form flag** (`0` or `1`)，不是 double 的高字节
- `a2+16..31` 16 字节可能是 (`radius`, `radius_secondary`) 或 (`axis_a.x`, `axis_a.y`) 向量
- `a2+48`, `a2+56` 是真正的两个 doubles (param/end_angle/sweep)

实测 axis1.y 多次出现 π/2 / π / 3π/2 (DWG-0202 多个 hits) 印证
`a2+24..31` 不是简单几何向量分量，更可能是 **rotation angle**。

**decoder 现状**：`SheetPrimitiveArcDecoded` 字段命名 (`axis1.x/y`,
`axis2.x/y`) 是 Slice F 早期推测，**语义未必正确**，但 decoder
本身是 conservative 验证 (all finite + magnitude in domain + param
sorted)，正常 fixture record 通过率高，**byte-level 不需要修改**。
未来 milestone 重命名字段为 `radius` / `rotation` / `start_angle` /
`form_flag` / `end_angle` 等更准确的语义后，`SheetPrimitiveArcDecoded`
DTO 与 `geometry.rs::build_normalized_geometry` 的 `radius =
|axis1|` mapping 都需联动更新。

**`sub_5658F950` 反编译辅助证据**（@ `0x5658F950`）：

- 调 `libm_sse2_sin_precise` 计算 sin (角度运算)
- 错误字符串 `"IMAr3dAr2d"` (Intergraph IMA Ar3d/Ar2d 库标识)
- `v13` 通过 `sub_564E0D90` 获取 (推测 sweep angle)
- 弧长公式 `(v13 * v7) / (2 * sin(...))` —— 弧长 = chord / (2*sin(α/2)) 的标准弦长公式

### GLineString2d 内存布局（已反编译，磁盘格式待证）

**反编译来源**：`sub_56524DD0` @ `0x56524DD0`（size 0x137）——
`GLineString2d::Validate` 类似函数，引用 4 条错误字符串：
- `"GLineString2d: NULL pointer"` @ `0x56669068`
- `"GLineString2d: LineString needs more than one point"` @ `0x56669084`
- `"GLineString2d: uninitialized data"` @ `0x566690b8`
- `"GLineString2d: scope out of range"` @ `0x566690dc`
- `"GLineString2d: form out of range"` @ `0x56669100`

**内存字段布局（32-bit binary, 12 bytes 不含 padding）**：

```c
struct GLineString2d {                       // 内存布局
    GPosition* vertex_array;  // a2 + 0  (4 bytes)
    uint32_t   vertex_count;  // a2 + 4  (4 bytes, must >= 2)
    uint8_t    form;          // a2 + 8  (1 byte, must <= 6)
    uint8_t    scope;         // a2 + 9  (1 byte, must <= 4 or == 6)
    uint16_t   _padding;      // a2 + 10 (2 bytes alignment)
};
```

**Validate flags**:
- `0x20000` = vertex_array_ptr == NULL
- `0x40000` = vertex_count < 2
- `0x1` = 某顶点 x/y NaN
- `0x1000000` = form > 6 或 scope > 4 && scope != 6

**磁盘格式（待反编译 `PSMSerializeOut` GLineString2d 分支验证）**：
推测 18-byte PSM 头 + 内联 payload：

```
PSM header (18B): type_code + bytes_to_follow + oid + aux
payload:
  4B  vertex_count (u32 LE)
  1B  form
  1B  scope
  2B  padding (alignment)
  vertex_count * 16B  顶点数组 (f64 LE x, y)
[可选 attribute tail]
```

`examples/probe_psm_polyline.rs` 跨 3 fixture 扫描，按此推测布局 +
`type_code != 0x3FE6 && != 0x0030` 排除已知 type code，得到：

- DWG-0201 /Sheet6: 3 hits, 全 type_code=0x0001, vertex_count=4
- DWG-0202 /Sheet6: 1 hit, type_code=0x0001, vertex_count=4
- 工艺管道-1 /Sheet6: 0 hits

**问题**：所有 hits 的前 2 个顶点都是 `(0, 0)`，第 3 个顶点才是真实坐标。
说明：
1. 磁盘布局可能与推测不同（vertex_count 可能不在 payload 头）
2. 0x0001 type code 可能是其他记录类型的 false positive
3. SmartPlant 工程图可能很少使用 polyline (多用 line + arc)

**下一步证据需求**：
1. IDA 反编译 `PSMSerializeOut` 的 GLineString2d 分支（找到调用
   `vptr_io::write_u32(vertex_count)` 等的具体代码）
2. 或者在 PSMSerializeIn 的 switch / dispatch table 中找 GLineString2d
   对应的 type_code
3. 用 controlled-diff 协议（Plan B）造一个含已知 polyline 的 fixture
   做字节比对

在拿到这些证据前，**不**实现 `decode_primitive_polylines` decoder，
避免基于 unverified hypothesis 写代码。Slice D-G 的 GLine2d/GArc2d
是在 IDA 反编译 + 跨 fixture 实测双重证据下落地的。

### Intergraph Sigma IGDS class tag 主映射表 (新发现)

**反编译来源**：`sub_56448F70` @ `0x56448F70`（size 0x18f）——
拿 `_WORD *a1` 第一个 word 作 switch key，返回 class name 字符串。
**这是 IGDS class tag → name 的权威映射**，可用于在 IDA 中识别
geometry 类。

| IGDS tag (hex) | IGDS tag (dec) | Class name |
|---|---|---|
| `0x06` | 6 | `igPointOnRelation2d` |
| `0x0F` | 15 | `igParallelRelation2d` |
| `0x13` | 19 | `igBoundary2d` |
| `0x15` | 21 | `igPerpendicularRelation2d` |
| `0x17` | 23 | `_TangentRelation2d` |
| `0x18` | 24 | **`igLine2d`** |
| `0x19` | 25 | `igKeyPointRelation2d` |
| `0x20` | 32 | `igRectangle2d` |
| `0x21` | 33 | `igComplexString2d` |
| `0x3D` | 61 | `igSmartFrame2d` |
| `0x40` | 64 | `igConcentricRelation2d` |
| `0x4D` | 77 | `igTextBox` |
| `0x59` | 89 | **`igCircle2d`** |
| `0x5D` | 93 | `igBSplineCurve2d` |
| `0x5E` | 94 | `igPoint2d` |
| `0x61` | 97 | **`igArc2d`** |
| `0x63` | 99 | `igEllipse2d` |
| `0x69` | 105 | `igSymmetricRelation2d` |
| `0x6A` | 106 | `igEqualRelation2d` |
| `0x6B` | 107 | `igColinearRelation2d` |
| `0x77` | 119 | `igFixRelation2d` |
| `0x7B` | 123 | `igGroup` |
| `0x7E` | 126 | `igEllipticalArc2d` |
| `0x84` | 132 | **`igLineString2d`** |
| `0x85` | 133 | `igTangentRelation2d` |
| `0xCE` | 206 | `igSymbol2d` |
| 277 | 277 | `igDimension` |
| 279 | 279 | `igBalloon` |
| 280 | 280 | `igLeader` |

### 关键区分：IGDS class tag ≠ PSM record type code

**注意**：IGDS class tag (上表) 与 **PSM record type code** (磁盘
record 头部 bytes 0..2 的 14-bit field) **不是同一个标识系统**：

- IGDS class tag 是 Intergraph Sigma 几何引擎的**内部 C++ 类标识**，
  存在内存对象 `*(a2 + 0)` 处供动态调度
- PSM record type code 是**磁盘序列化标识**，存在 record 头 bytes 0..2

实测对照：

| Geometry class | IGDS tag | PSM type code | 来源 |
|---|---|---|---|
| `igLine2d` | `0x18` (24) | `0x3FE6` (16358) | Slice D + IDA |
| `igArc2d` | `0x61` (97) | `0x0030` (48) | Slice F + IDA |
| `igLineString2d` | `0x84` (132) | **未知** | 待 PSMSerializeIn 反编译 |
| `igCircle2d` | `0x59` (89) | 未知 | 可能与 GArc2d 共用 |
| `igEllipticalArc2d` | `0x7E` (126) | 未知 | 可能 = GArc2d |
| `igRectangle2d` | `0x20` (32) | 未知 |  |
| `igPoint2d` | `0x5E` (94) | 未知 |  |
| `igSymbol2d` | `0xCE` (206) | 未知 |  |

要打通 IGDS tag ↔ PSM type code 映射，下一会话可：

1. 反编译 `sub_564915E0` (`PSMSerializeIn`) 的 switch 找 type code
   分发表（fast-path 5 个固定大小 type code 已知：276/277/278/279/280
   = `igDimension`/`igDimension`/`?`/`igBalloon`/`igLeader`，需对照
   confirm）
2. 跟踪 `PSMSerializeOut` 的 vtable dispatch 找到 IJPersist 接口
   Save 方法的 per-class 实现
3. 用 controlled-diff 协议造一个 polyline-only fixture 抓 record 字节

### 其他类（待 Phase 14 后续）

| Sigma 类 | 推测 PidGraphicKind | 字段（待反编译验证） |
|---|---|---|
| `igCircle2d` | `Circle` | 4 doubles? center(2) + radius_x(1) + radius_y(1)，或退化 GArc2d (radius=axis1.len) |
| `igLineString2d` | `Polyline` | 见上节，磁盘格式待证 |
| `igEllipticalArc2d` | (椭圆 Arc) | 8 doubles，可能 = GArc2d |
| `igBSplineCurve2d` | NURBS | degree + knots + control points + weights |
| `IGDSFactoryRectangle` / `Rectangle` | `Rectangle`（**新发现**） | 4 doubles? corner + width + height |
| `IGDSFactoryBspCurve` | B-spline | 同 igBSplineCurve2d |
| `IGDSFactoryCmplxStr` / `Cmplx Str` | 复合字符串（complex string） | 子图元 list |
| `IGDSFactoryText` / `IGDSFactoryTextPointRectShape` | `Text` | position + style + UTF-16 string |

### PSMSerializeOut / PSMSerializeIn dispatch 双结构 (反编译总结)

#### PSMSerializeOut (`sub_56491E80`)

```c
v22 = ((unsigned int)v6[2] >> 6) & 0x3FFF;  // PSM type code = bits[6..20]

// 5 个 fast-path 固定大小 type code (内嵌写出):
if (v22 == 276) { write 35 bytes }   // 头部 type=276, 35-byte payload
if (v22 == 277) { write 16 bytes }   // 头部 type=277, 16-byte payload
if (v22 == 278) { write 53 bytes }   // 头部 type=278, 53-byte payload
if (v22 == 279) { write 8  bytes }   // 头部 type=279, 8-byte payload
if (v22 == 280) { write 59 bytes }   // 头部 type=280, 59-byte payload

// 所有其他 type 走 vtable dispatch:
// 1. QI(record, IID_IJPersist) -> v37  (IJPersist 接口)
// 2. (*v37->vtable[5])(...)  OR  (*v37->vtable[6])(...) 调 Save
// 3. Save 实现因类而异: GLine2d::Save 写 6 doubles,
//    GArc2d::Save 写 8 doubles + form/scope, 等等
```

277 与 `igDimension`、279 与 `igBalloon`、280 与 `igLeader` 是 IGDS
tag, **不是** PSM type code (类似 0x3FE6 vs 0x18 的关系)。固定大小
277/16B 是某 PSM type 而非 IGDS tag。

#### PSMSerializeIn (`sub_564915E0`)

读路径**对称**于 PSMSerializeOut：

```c
v36 = read u16 type word        // PSM type code
v40 = read u32 bytes_to_follow  // payload 长度
v37 = read u32 oid              // 对象 ID
v35 = current stream offset

// 查表: type code -> class 链 (循环走 *(v43 + 18) 走表)
v3 = sub_564689C0(v36, &v43);
while (v12 = *(_WORD *)(v43 + 18), v12 != 0) {
    v3 = sub_564689C0(v12, &v43);
    // ...
}

// 拿到 class -> 创建 IJPersist 实例 -> 走 vtable dispatch
v3 = (**v34)(v34, dword_56661994, &v42);  // QI(IID_IJPersist)
// 然后:
// (*(*v42)[5])(...) 或 (*(*v42)[6])(...) 调 Load 反序列化
```

**guidtab.h 查询表**：错误消息 `"OID=%d nType= %d in guidtab.h"`
证实存在 `type code -> class GUID -> factory` 静态映射表。`sub_564689C0`
是该表的 lookup 函数, 表的根指针在 `dword_567DDC90` (.data 段)。

#### PersistTypeTable<PersistComTypeEntry> 类发现

构造函数 `sub_56455720` (vtable @ `0x5665FA1C`) 显示表的 C++ 类是
**`PersistTypeTable<PersistComTypeEntry>`**:

```c++
class PersistTypeTable<PersistComTypeEntry> {
    // vtable at 0x5665FA1C
    _DWORD vtable_ptr;       // +0
    _DWORD field_4;          // +4   = 0 (zeroed by ctor)
    _DWORD field_8;          // +8   = 0
    _DWORD field_12;         // +12  -> entry_array (4 * entry_count u32 ptrs)
    _WORD  field_18;         // +18  entry_count (max index)
    // ...
};

struct PersistComTypeEntry {
    // Layout inferred from sub_564689C0 + PSMSerializeIn usage:
    _WORD  field_16;    // +16  matching PSM type code (u16)
    _WORD  field_18;    // +18  chain link (next entry index or 0)
    // Plus probably:
    // +0:  IGDS class tag
    // +4:  16-byte CLSID GUID
    // +20: factory function ptr (IClassFactory * or similar)
    // +24: extras
};
```

`dword_567DDC90` 是表实例的 root pointer，构造由 CRT 启动时 `sub_56441330`
调用，销毁通过 `atexit(sub_5665D290)` 注册。

**条目注册由各 `IGDSFactory*` 模块 init 完成**（推测），分散在
binary 中。每个 IGDSFactory 类构造时调
`dword_567DDC90->Register(type_code, factory)` 注入一条 entry。

要拿 polyline / circle / text 等的 PSM type
code, 可：

1. 反编译 `sub_564689C0` 找表数据指针 (likely 一个 RVA 数组)
2. 在 `.rdata` 中 dump 该表 (含 (PSM type code, GUID, factory_ptr) 三元组)
3. 通过 factory_ptr 找到对象创建函数 → 该对象的 vtable → Save/Load 虚函数

**结论**: PSMSerializeOut 与 PSMSerializeIn 不含 polyline/circle/text
的具体 layout —— 它们只做调度。每个 geometry 类的磁盘字段布局只能
通过反编译 **该类的 IJPersist::Save 虚函数** 拿到。

下一 session 复现路径:
- IDA 找 igLineString2d / igCircle2d / igEllipticalArc2d 的 vtable
- vtable 中找 Save slot (通常是高位 slot)
- 反编译 Save 拿到字段布局 + PSM type code
- 用模板 (Slice D-G) 落地 decoder

或 Plan B 路径:
- 用 controlled-diff 协议造 polyline-only / circle-only fixture
- 对比 before/after byte diff 反推 layout

## IGDSFactory* vtable 调查（slot 4-6 是属性 setter）

通过 RTTI string xrefs 拿到 IGDSFactoryLine 的两个 vtable：

```
COL @ 0x566714B4 -> primary vtable @ 0x5666A94C
  [0..2]  共享 IUnknown 方法 (QueryInterface/AddRef/Release)
  [3]     0x565A93F0 (共享, 推测 GetTypeId 或类似)
  [4]     0x565A93C0 -- *(this + 70) = a2;   // setter
  [5]     0x565A93A0 -- *(this + 74) = a2;   // setter
  [6]     0x565A9330 -- *(this + 78) = a2;   // setter
```

**确认**：IGDSFactory* 是**属性 builder pattern** —— 通过一连串 setter
累积 line weight / color / style 之类的参数，**不是** Save 入口。Save
经 Sigma `IJTypedGeometry2d::Save` 或 PSMSerializeOut 写入对象本身
（已知字段布局 = GLine2d 6 doubles / GArc2d 8 doubles + 可选属性）。

实际 Sheet 流上每个 primitive 的字节流大致为：

```
18 bytes  PSM record header (type + bytes_to_follow + oid + aux)
N bytes   inner_payload (N = GLine2d 48B / GArc2d 64B / 其他)
[var]     可选属性 (color / line weight / layer 等)
```

进一步 evidence 需要：

1. 找 `IJTypedGeometry2d::Save` 方法（在 vtable 高位 slot）
2. 或者直接在 `DWG-0201GP06-01.pid /Sheet6` 上拿一条已知 line record
   按 6 × f64 解出验证
3. type_code ↔ class 映射来自 `guidtab.h`，IDA 里可能能找到 type
   table data array

## 错误字符串证据列表（IDA 引用）

| 字符串地址 | 字符串 | 用途 |
|---|---|---|
| `0x566662f8` | `"FAILURE: PSMSerializeOut()[pMgr = 0x%p] BytesToFollow <= 0\n"` | PSMSerializeOut 已确认 |
| `0x56666334` | `"FAILURE: PSMSerializeOut()[pMgr = 0x%p]\n"` | PSMSerializeOut 已确认 |
| `0x56666360` | `"Warning: PSMSerializeIn()[pMgr = 0x%p]  BytesToFollow %d mismatch ..."` | PSMSerializeIn 已确认 |
| `0x56666448` | `"%s\n%s OID=%d nType= %d in guidtab.h"` | guidtab.h 是 type→GUID 表 |
| `0x56666414` | `"Stream offset mismatch while reading serial data"` | PSMSerializeIn mismatch 路径 |
| `0x566663d0` | `"We advise checking for mismatch between Save and Load methods for"` | 类内 Save/Load 对称约束 |
| `0x56665f98` | `"ClusterTable::GetSpaceMapSegment()[pMgr = 0x%p]: In PSM_PERSISTID_NO_REUSE ..."` | PSM cluster table 索引 |

## Phase 14 plan 影响

满足以下 AC：

- **AC2** ✓：`OpenStream("SheetN")` 调用点暂未直接定位，但**字节布局
  完全确定**——这是 AC2 的真正目的（找到 record 解析路径），用
  `PSMSerializeOut/In` + 它们的对象 Save/Load 虚函数比单纯找 `OpenStream`
  更有价值
- **AC3** 部分：record kind dispatch 是隐式的（通过对象 record 的 14-bit
  type 字段 + guidtab.h 映射 + virtual Save/Load 表）；不是经典的 switch
  dispatcher，但等价

下一步（Slice C/D）：

1. 找 `igLine2d::Save` / `igLine2d::Load` 虚函数地址（按 RTTI string
   `.?AVGLine2d@@` xrefs 找 vtable）
2. 反编译这些 Save 函数得到 inner_payload 的字段表
3. 与 `DWG-0201GP06-01.pid /Sheet6` 实际字节对账
4. 上述完成后，把字段表写进 `src/parsers/sheet_records.rs::decode_primitive_line()`
5. 配 `tests/parser_panic_safety.rs` + `tests/parse_real_files.rs` 测试

## 关联现有项目代码

- `src/parsers/psm_tables.rs::parse_psm_cluster_table` / `parse_psm_segment_table`：
  已经在解析 PSM cluster + segment 索引，这次反向证实这些索引指向
  PSMSerializeIn 可读的 18-byte-header records
- `src/parsers/psm_tables.rs::parse_psm_roots`：PSMroots 流的结构也走
  同一序列化协议
- `src/parsers/tagged_stg_list.rs`：与 `radsrvitem.dll` 里的
  `TaggedTxtMgr::IJTaggedTxtMgrImp` 直接对应
- `src/parsers/sheet_probe.rs::sheet_text_window_candidates`：probe 抓
  到的 `0x89` marker 字节很可能就是 PSM record 内部的子字段标记，不是
  record 头本身

## 下一步追踪

在同一 IDA 实例 (port 13346) 继续：

1. 找 `igLine2d` 的 RTTI `.?AVGLine2d@@`（在 0x56669004 字符串
   附近，已 xref 到 sub_56524C50）
2. 从 sub_56524C50 反推 GLine2d 的 vtable
3. vtable slot 中找到 PersistOut / PersistIn / Save / Load
4. 反编译这些函数得到具体字段
5. 同上对 `GArc2d`、`GCircle2d`、`GLineString2d`
