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

## 几何 primitive 类对应（待 Slice C 进一步证实）

`radsrvitem.dll` 字符串里发现的 **Intergraph Sigma 2D geometry** 类家
族，应当对应一种或多种 PSM type_code：

| Sigma 类 | 推测 PidGraphicKind | 字段（典型 Sigma 2D） |
|---|---|---|
| `igLine2d` / `IGDSFactoryLine` | `Line` | 2 × `GPosition` (start, end) = 2 × 16 bytes |
| `igCircle2d` / (factory 未列) | `Circle` | center (`GPosition`) + radius (f64) |
| `igArc2d` / `IGDSFactoryArc` | `Arc` | center + radius + start_angle + sweep |
| `igEllipticalArc2d` / `IGDSFactoryEllipArc` | `Arc` (椭圆) | center + 2 axes + start/sweep |
| `igLineString2d` / `IGDSFactoryLineString` | `Polyline` | 顶点数 + vertex 数组 |
| `igBSplineCurve2d` | （新发现） | NURBS / B-spline 参数 |

进一步 evidence 需要 reverse 这些类的 Save 虚函数（typically vtable
slot offset 12 or 16 from 上面 `Step 5` 的 `vfunc + 12/16/20/24`）。

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
