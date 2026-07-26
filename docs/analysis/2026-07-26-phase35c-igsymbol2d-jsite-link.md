# Phase 35-C：`igSymbol2d` → `JSite<id>` 实例级关联

> 日期：2026-07-26
> 范围：证明每条 `igSymbol2d` 记录内嵌其符号引用所在 `JSite<id>` 存储的
> 数字 id，并把该连接件落进解码器与 normalized geometry
> （`SymbolInstance.symbol_path`）。

## 结论

**连接件已证明并落地**：`igSymbol2d` payload 中、紧贴
`IGSYMBOL2D_MATRIX_TAG`（`02 00 A7 50`）**之前的 4 字节 u32** 是同文件
顶层 `JSite<id>` 存储的数字 id。探针
`examples/probe_igsymbol2d_jsite_link.rs` 在全部 6 个注册 fixture 上
共验证 **132/132** 条记录：

| Fixture | records | id 命中 | 备注 |
|---|---:|---|---|
| DWG-0201GP06-01 | 20 | 20/20（+29 ×17，+31 ×3） | 命中 19/20 个已知 JSite（948 未被放置） |
| DWG-0202GP06-01 | 23 | 23/23 | 命中 12/13（23 未被放置） |
| 工艺管道及仪表流程-1 | 58 | 58/58（+31 ×2） | 命中 9/10（24 未被放置） |
| D06 | 6 | 6/6（+31 ×1） | 命中 8/9 |
| publish A01 | 2 | 2/2（+31 ×1） | Flanged Nozzle + Horizontal Drum |
| publish DWG-0202 | 23 | 23/23 | 同 DWG-0202 |

偏移在 +29 / +31 之间浮动只是矩阵标签位置浮动（113/115/123 字节记录
族的 header 增长发生在 id **之前**），id 与标签始终相邻——这与
Phase 14-N 用标签而非固定偏移锚定矩阵的结论一致。

跨 fixture 直方图（`+pos` 为 payload 偏移，命中/总数）：

```text
total records: 132
+29: 126/132   ← jsite_ref（其余 6 条在 +31，同样紧贴标签）
+117: 63/64    ← 尾部第二引用，指向无 .sym 路径的容器类 JSite（未采用）
```

每条命中的 `JSite<id>` 的 `JProperties` 均携带 `.sym` 库路径
（`\\WIN-SPID\...`、`\\SPID\...`、`\\MM-128\...` 等），符号名可从路径
文件名得出。`/JSitesList` 条目与 `JSite<id>` 存储号的既有相关性
（`src/parsers/jsites_list.rs`，Phase 29-M / Phase 30 IDA）为该连接件
提供了旁证。

## 记录内典型字节（DWG-0201 record[0]）

```text
+024: 0B 52 00 00 00 8F 01 00 00 02 00 A7 50 ...
       │  └─u32=82（前导引用，未命名）
       │              └─u32=0x018F=399 = JSite399（Flanged Nozzle.sym）
       └─0x0B 子记录标签                └─矩阵标签
```

`0F` 变体（D06 record[0]、A01 record[1] 等）把矩阵标签推后 2 字节，
id 仍在标签前 4 字节。

## 落地内容

- `SheetIgSymbol2dDecoded.jsite_ref: u32`（解码规则：
  `tag_at.checked_sub(4)`，下溢拒绝）；
- `DecodedIgSymbol2dRecord.jsite_ref`（`#[serde(default)]` 向后兼容）；
- `geometry.rs` 新增 `EmitContext`（文档级查找表，一次构建供全部
  emitter 共享），`IgSymbol2dEmitter` 用
  `jsite_ref → JSite.symbol_path（回退 local_symbol_path）` 解析
  `SymbolInstance.symbol_path`；provenance note 带上 `jsite_ref`；
- 单测：synthetic builder 增加 `jsite_ref` 参数；header 增长用例改为
  在 id 之前插入字节（贴合真实 +29/+31 分布）；
- 集成测试 `igsymbol2d_jsite_ref_resolves_to_symbol_paths`：
  6 fixture 全量断言 `jsite_ref ∈ JSite ids`、SymbolInstance 100%
  携带 `.sym` 路径、A01 spot-check（Flanged Nozzle / Horizontal Drum）；
- geometry golden 重祝福（`UPDATE_GEOMETRY_GOLDEN=1`，diff 逐项核对为
  `symbol_path` 新增与 note 扩充）。

## 证据等级

`corpus-statistical`（132/132、6 fixture、位置锚定于矩阵标签），
并有 `/JSitesList` ↔ `JSite<id>` 的 Phase 30 IDA 旁证。未做 IDA
native-reader 逐字段读取序证明；若后续在 `radsrvitem.dll` /
`ugeom2d1.dll` 中定位到 `igSymbol2d` 序列化体，可升级为 `ida-proven`。

## 尾部第二引用（未采用）

`+109..+117` 附近还有一个 u32 引用，稳定指向**无** `.sym` 路径的
容器类 JSite（DWG-0201 的 329/396、gongyi 的 7559/6963 等）。语义
未证明（疑似标签宿主 / 分组容器），本切片不解码、不发射，留给后续
需要时再立证据门。
