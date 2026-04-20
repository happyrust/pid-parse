# Writer 容器保真：CLSID 与时间戳

本文记录 `PidWriter` 在 **CFB 容器级元数据**（与流内容**无关**）上能做什么、不能做什么。

## 为什么要关心这些

对 SmartPlant / Smart P&ID 自己而言，stream 字节是内容、**CLSID 是身份**、**时间戳是审计**。
`PidWriter` 第一版在 `cfb = "0.10"` 约束下，能力矩阵如下。

## 能力矩阵 (`cfb` 0.10)

| 项目 | 读取 | 写入 | `pid-parse` 表现 |
|---|---|---|---|
| Root Storage CLSID | ✅ `root_entry().clsid()` | ✅ `set_storage_clsid("/", uuid)` | ✅ **保留**（v0.3.2 起） |
| 非 root Storage CLSID | ✅ `entry(path).clsid()` | ✅ `set_storage_clsid(path, uuid)` | ✅ **保留**（v0.3.7 起） |
| Root 创建时间 | ✅ `entry.created()` | ❌ 仅 `touch(path)`（=now） | ❌ 失真（新建容器） |
| Root 修改时间 | ✅ `entry.modified()` | ❌ 仅 `touch(path)` | ❌ 失真 |
| Stream CLSID | ✅ | ❌ 无公开 API | ❌ 不保留 |
| Stream state_bits | ✅ `state_bits()` | ❌ 无公开 API | ❌ 不保留 |
| CFB Version (V3/V4) | ✅ | ✅ `create_with_version` | V4 硬编码（写出） |
| Stream 目录顺序 | 原生 | 自由 | BTreeMap 排序（字典序，非源顺序） |

## 选择理由

- **`cfb::CompoundFile::create`** 默认把 root CLSID 设为 nil UUID，这会让 SPPID 把文件识别为"未知 COM 对象"。我们显式调用 `set_storage_clsid("/", source_clsid)` 把这个身份还原。真实样本 `DWG-0201GP06-01.pid` 的 root CLSID 是 `{16ce6023-5f5b-11d1-9777-08003655f302}`（SmartPlant P&ID classic CLSID）。
- **时间戳**在 `cfb` 0.10 下只能用 `touch(path)`，等同于"把修改时间设为当前时间"，无法赋任意时间。与其伪造一个保留感（写回"假旧时间戳"），不如**直接承认会刷新**。
- **非 root Storage CLSID**（v0.3.7 新增保留）：虽然 Phase 9a 跳过了这一项，但真实样本 `DWG-0201GP06-01.pid` 实际有 **3 个**非 root storage 携带非 nil CLSID——`/JSite329` / `/JSite396` / `/JSite948`，分别指向 SmartPlant 内部 COM 组件（`0a1cf23d-…` 和 `7effbe60-…`）。Phase 9a 的 passthrough 在流字节层面完美，但会悄悄丢掉这 3 个 CLSID。v0.3.7 起 `parse_package` 捕获、`write_to` 写回、`diff_packages` 观察这三件事一起做全。

## 验证

- `tests/writer_roundtrip.rs::root_clsid_round_trips_when_source_has_one`：内存 fixture 写入任意 CLSID → 解析 → 回写 → 再解析，CLSID 字段相等。
- `tests/writer_roundtrip.rs::fixture_without_clsid_reports_none`：nil UUID 规范化为 `Option::None`（方便代码分叉判空）。
- `tests/writer_roundtrip.rs::non_root_storage_clsid_round_trips`（v0.3.7+）：内存 fixture 给 `/UnknownStorage` 烧一个 `F29F85E0-…` → round-trip 保持。
- `tests/writer_real_files.rs::real_file_passthrough_preserves_root_clsid`：真实 `.pid` 样本的 CLSID 经 passthrough round-trip 保留。
- `tests/writer_real_files.rs::real_file_reports_non_root_storage_clsids_deterministically`（v0.3.7+）：真实样本的非 root CLSID 条数 / 值在 round-trip 后完全一致，非 nil 约束生效。

## 当宿主要求高保真时的工作方式

如果下游消费者（SPPID 加载器、HCS 等）对容器级元数据的差异敏感：

1. **只改 stream 内容**（metadata-only 或 SheetPatch）：此路径下 CLSID 已保留，唯一缺口是 timestamps。通常能被 SPPID 接受（SPPID 会自己更新 Last Modified）。
2. **用 `PidWriter` 输出后二次后处理**：读回文件 → 手动覆盖 timestamps（通过 `cfb` 以外的工具如 ssstic / CompoundFile.NET）→ 保存。超出 `pid-parse` 范围。
3. **等 `cfb` crate 开放时间戳 API**：已知 upstream 讨论中。

## 下一步（候选，未提上计划）

- Stream CLSID / state_bits 若 upstream 开放 API 则接入
- 基于 `ssstic` 或自定义 dir-sector 编辑的时间戳后处理
- `SummaryInformation` property set 写入（与 SPPID 的 Title / Author 对齐）
