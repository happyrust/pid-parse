# Writer 容器保真：CLSID / 时间戳 / state_bits

本文记录 `PidWriter` 在 **CFB 容器级元数据**（与流内容**无关**）上能做什么、不能做什么。v0.3.13 (Phase 9k) 起，随着 `cfb = "0.14"` 升级，之前标记"❌ 无公开 API"的限制**全部解锁**。

## 为什么要关心这些

对 SmartPlant / Smart P&ID 自己而言，stream 字节是内容、**CLSID 是身份**、**时间戳是审计**、**state_bits 是状态机信号**。`PidWriter` 从 v0.3.13 起全部保真。

## 能力矩阵（cfb 0.14，v0.3.13+）

| 项目 | 读取 | 写入 | `pid-parse` 表现 |
|---|---|---|---|
| Root Storage CLSID | ✅ `root_entry().clsid()` | ✅ `set_storage_clsid("/", uuid)` | ✅ **保留**（v0.3.2 起） |
| 非 root Storage CLSID | ✅ `entry(path).clsid()` | ✅ `set_storage_clsid(path, uuid)` | ✅ **保留**（v0.3.7 起） |
| Storage 创建时间 | ✅ `entry.created()` | ✅ `set_created_time(path, SystemTime)` | ✅ **保留**（v0.3.13 起） |
| Storage 修改时间 | ✅ `entry.modified()` | ✅ `set_modified_time(path, SystemTime)` | ✅ **保留**（v0.3.13 起） |
| 对象 state_bits（storage + stream）| ✅ `state_bits()` | ✅ `set_state_bits(path, u32)` | ✅ **保留**（v0.3.13 起） |
| Stream 时间戳 | — | — | N/A（CFB spec 规定 stream 不单独带时间戳，跟随父 storage）|
| Stream CLSID | ✅ | ❌ 无公开 API | ❌ 不保留（cfb upstream 限制）|
| CFB Version (V3/V4) | ✅ | ✅ `create_with_version` | V4 硬编码（写出） |
| Stream 目录顺序 | 原生 | 自由 | BTreeMap 字典序（非源物理顺序）|

## 选择理由

- **Root + 非 root CLSID**：SmartPlant / SPPID 用 CLSID 识别容器身份（`{16ce6023-5f5b-11d1-9777-08003655f302}` 是 SmartPlant P&ID classic CLSID），丢掉就会被识别为"未知 COM 对象"。真实样本 `DWG-0201GP06-01.pid` 有 3 个非 root CLSID（`/JSite329` / `/JSite396` → `0a1cf23d-…`；`/JSite948` → `7effbe60-…`），Phase 9e 起全部保留。
- **Storage 时间戳**：cfb 0.14 新增 `set_created_time` / `set_modified_time` 接受任意 `SystemTime`，不再局限于 "set to now"。真实样本有 **25 个非 epoch 时间戳**（root / JSite×19 / PSMspacemap / TaggedTxtData），Phase 9k 起全部保留。
- **State_bits**：CFB 规范里是用户自定义的 32-bit 状态标志。SPPID 可能用它标记"已修改"、"已锁定"等。Phase 9k 起非零值全部保留。
- **Stream CLSID**：cfb 0.14 仍然没有公开的 `set_stream_clsid` API；真实样本里所有 stream 的 CLSID 都是 nil，目前影响几乎为零。
- **CFB Version**：v0.3.x 固定写 V4（cfb 默认值）。真实 SmartPlant 样本是 V4，所以在实践上不失真。
- **目录物理顺序**：BTreeMap 字典序写出，与源的 "创建顺序 / 删除洞 / 碎片化" 物理顺序不同。对内容驱动的消费方无影响。

## 验证

- `tests/writer_roundtrip.rs::storage_timestamps_and_state_bits_round_trip`（v0.3.13+）：内存 fixture 烧 `t_created=unix+1_700_000_000` + `t_modified=unix+1_800_000_000` + `state_bits=0x0123`，round-trip 保持。
- `tests/writer_real_files.rs::real_file_passthrough_preserves_storage_timestamps`（v0.3.13+）：真实样本上 **25+ 个非 epoch timestamps** 经 round-trip 一一保持。
- `tests/writer_real_files.rs::real_file_passthrough_produces_empty_diff_full`（v0.3.13+）：`diff_packages` 在**所有 6 个维度**（stream 字节 / root CLSID / 非 root CLSID / 非 epoch timestamps / state_bits / path 存在性）全部为 0 diff。
- `tests/writer_roundtrip.rs::root_clsid_round_trips_when_source_has_one`：内存 fixture 写任意 CLSID → round-trip 保持。
- `tests/writer_roundtrip.rs::non_root_storage_clsid_round_trips`：内存 fixture 写非 root CLSID → round-trip 保持。
- 模块内单元测试：`diff_flags_storage_timestamp_mismatch` / `diff_flags_state_bits_mismatch` / `diff_flags_non_root_storage_clsid_mismatch` / `diff_flags_root_clsid_mismatch` 覆盖 diff 模型每个维度的失配检测。

## 对下游消费者（SPPID 加载器）的建议

v0.3.13 起的 passthrough round-trip 已经做到**容器级字节几乎无损**（除 stream 目录物理顺序与 stream CLSID 外）。对绝大多数 SPPID 使用场景都足够兼容。

如果你的消费方对**某个特定字段**敏感而仍然看到问题，请：
1. 跑 `pid_inspect a.pid --diff b.pid`（其中 a 是原文件，b 是 round-trip 输出）
2. 贴 diff 报告到 issue；如果 diff 非空，多半是本文档"❌"行之一，需 cfb upstream 动作
3. 如果 diff 完全为空但消费方仍报错，那问题可能在 stream 内容语义（例如 `/Unclustered Dynamic Attributes` 的某个字段编码）—— 非本文档范围

## 升级历史

| 版本 | cfb 依赖 | 新增保真能力 |
|---|---|---|
| v0.3.2 | 0.10 | stream 字节 passthrough |
| v0.3.3 | 0.10 | + root storage CLSID |
| v0.3.7 | 0.10 | + 非 root storage CLSID |
| **v0.3.13** | **0.14** | **+ storage 时间戳 + state_bits** |

## 下一步（候选，未提上计划）

- Stream CLSID 保留（等 cfb upstream 开放 `set_stream_clsid` API）
- CFB Version（V3 vs V4）自适应保真（目前固定 V4 写出）
- `SummaryInformation` property set 写入（与 SPPID 的 Title / Author 对齐）
