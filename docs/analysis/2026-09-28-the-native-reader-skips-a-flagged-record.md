# 原生读取器跳过的记录：PSM 类型字的 `0x8000` 位（2026-09-28）

> 起因：OpenCADStudio 计划 `docs/plans/2026-09-28-pid-import-next-steps.md` R1 量 11 条 run 打架标签时，发现工艺图 `/Sheet6` 里 oid 6345 存了两条 `igTextBox`。
> 结论同日按该计划 P-D12 落地（本仓 `parse_live_psm_header`）。探针 `examples/probe_run_conflicts.rs`，棘轮 `tests/render_gap_census.rs::the_records_the_native_reader_skips_are_counted_apart`。

## 一句话

PSM 记录头的类型字高两位是旗标；`PSMSerializeIn` 读到 `0x8000` 置位就把整条记录 seek 过去、连 oid 都不读（05-14 反编译笔记 §PSMSerializeIn）。
本仓从 Phase 14 起把这两位读进 `type_flags`，只有 `DependencyObject` / `JStyleOverride` 两个解码器拒 `type_flags != 0`，其余族照解照画。
语料五图里带这一位的记录 **42 条**，每一条都是同一 oid 活记录的**旧副本**（同插入点、同定义本体、活副本总在更靠后的字节），或整个已退役的孤儿存储——
OCS 因此把工艺图 13 个符号多画了 1–3 遍、把旧管道号 `250-LNG-57602- - ` 叠在 `250-LNG-57602-6D02-C` 上。

## 证据

### 1. 原生行为

`docs/analysis/2026-05-14-radsrvitem-psm-serialize-bytes.md`：

```c
v3 = read(&v36, 2);          // type word
v3 = read(&v40, 4);          // BytesToFollow
if ((v36 & 0x8000) != 0) {
    seek(v40 + v8);          // skip the whole record
    return 1024;
}
v3 = read(&v37, 4);          // oid -- only for a live record
```

`type_flags = type_word >> 14`，所以 `0x8000` ↔ `type_flags & 0b10`。`0x4000` 语料里无一处置位；原生对它的处理未见，本仓不碰它。

### 2. 语料普查（`probe_run_conflicts` + `render_gap_census`）

| 图 | 流 | 族 | 条数 | 与活记录的关系 |
|---|---|---|---:|---|
| 工艺 | `/Sheet6` | `igSymbol2d` | 27 | 13 个 oid（6302 / 6971 / 6987 / 7002 / 7019 / 7034 / 7052 / 7066 / 7084 / 7104 ×3 / 7122 / 7138 / 7269）各 1–3 份旧副本，与活副本**同插入点、同 `definition_sheet_ref` / `definition_site_ref`**；活副本总在更大的字节偏移 |
| 工艺 | `/Sheet6` | `igTextBox` | 1 | oid 6345：旧文本 `250-LNG-57602- - `（字节 1615..1799）与现文本 `250-LNG-57602-6D02-C`（46104..46294）同插入点 (0.1705, 0.2215)、同层 12、同段落样式 36 |
| 0202 | `/Sheet6615` | `igLine2d` 4 + `igRectangle2d` 1 | 5 | 孤儿存储里的矩形与四条边——该存储没有活记录 |
| 0201 | `/Sheet6` | `igSmartFrame2d` | 5 | 页框 oid 947 的副本；`frame` 发射器早按范围去重，页面无变 |
| A01 | `/JSite204/Sheet6` | `igLine2d` | 4 | oid 110 / 111 / 236 / 一条 |

合计 42。`style_link_ratchet` 里那条「156 条标签对 155 条索引、差的一条是 oid 6345 碰撞」的备注，记的正是这条旧文本。

### 3. 落地后的数

- 工艺放置 58 → **31**（`every_placement_names_a_body_the_drawing_carries`），放置笔画 (287, 237) → (181, 157)，显示笔画 237 → 157。
- 0202 `igLine2d` 46 → 42，A01 80 → 76；线样式关联 669 → 638；调色板 `As Drawn` x43 → x17（旧放置点名的样式）。
- 文字：形状 3 12 → 11，选择子 1 / 2 = 248 / 11，带 run 166；标签 155 = 索引 155。
- 拒收 / 无解码器两表**不动**（0 / 4 / 8 / 0 / 0，0 / 0 / 0 / 0 / 1）：跳过的记录是第三类，不算缺口、不进 warnings。
- golden：0202 219 → 215 实体、工艺 476 → 448、A01 117 → 113。

## 做法

- `parsers/sheet_records.rs`：`PSM_TYPE_FLAG_NATIVE_SKIP`、`PsmHeader::native_reader_skips`、`parse_live_psm_header`；22 个族解码器的 `decode_at` 都从它起步。
  `parse_psm_header` 不变——普查与探针要看见旗标。
- `parsers/undecoded_census.rs`：`unclaimed_counts` 的 `keep` 多收一个「原生跳过」布尔；`undecoded_type_code_census` / `refused_record_census` 排除带位记录；
  新增 `native_skipped_record_census` / `SkippedRecordCount`。
- `model/sheet.rs` `SheetGeometry::skipped_records`，`streams/cluster.rs` 填它；`geometry.rs` `NormalizedPidGeometry::skipped_graphic_records`（只列，不告警）。

## 没做 / 开口

- `0x4000` 位：语料无例，不处理。
- 原生 `PSMSerializeIn` 跳过记录后返回 1024——调用方是否据此把该 oid 从对象图里摘掉未查；本仓的空间图 / 对象图仍按原样解析，只是几何族不解这些记录。
  语料上活副本都在、oid 不缺，所以 crossref / 语义连接不受影响（`semantic_join` 2 / `geometry_profile` 2 绿）。
- OCS 侧：四图实体数、工艺放置数、`--export` 字节随之变；`pid_import` 与批量基线按 OCS 计划重钉，逐实体比只许少、不许多或挪。
