# Phase 19: PSM 0x0010 leading-word audit (sub-kind discriminator, partial)

## 目标产出

在 Phase 18 已落地的 `SheetSubRecord0x0010Decoded` audit-only 集合
之上，**加一个稳定 audit-only `leading_word: u16` 字段** = `payload[0..2]`
小端 u16。这是 Phase 19 probe 在 578 跨 4 fixture record 上确认的**部分
sub-kind discriminator**。

`leading_word` **不被命名为 `sub_kind`**——Phase 19 probe 同时证明：
单一固定偏移 discriminator **不能干净划分整个 0x0010 家族**（size 31
/70/13/16/43 在 +0 处异质），所以本 phase 不暴露任何 sub-kind 字段
名，**只暴露原始 word**。

下游消费者拿到的是：

- Phase 18 已有的 audit collection（`byte_range / type_code /
  type_flags / bytes_to_follow / raw_payload`） + **新增
  `leading_word`** （u16, LE, = `payload[0..2]`，长度 < 2 时为 `None`）
- 一份跨 fixture **`leading_word == 0x0002` 计数 = 164** 的 ratchet test
- 一份按 size bucket 区分 "discriminator-friendly" vs "heterogeneous"
  的 audit JSON 字段，用于后续 sub-kind reverse engineering

## 背景

Phase 18 把 0x0010 family 落地为 audit-only 集合（582 records）。 
Phase 19 probe（`examples/probe_psm_0x0010_sub_kind.rs`，本 phase 新增）
在 4 fixture /Sheet6 上 advancing-scan **578 records**（与 Phase 18 ratchet
的 582 相差 4——本 probe 的 visit predicate 是 `payload_end ≤ data.len()`
的硬等号，Phase 18 ratchet 还应用了 `bytes_to_follow ≥ 8` 之类的额外
validation；差异不影响 discriminator 分布结论），分析如下：

### 全局 `word@+0`(LE) 直方图（top 12）

| Word | Count | % | 说明 |
|---|---:|---:|---|
| `0x0002` | 164 | 28% | **跨 ≥ 40 个 size bucket 的支配性 leading word** |
| `0x0003` | 21  | 3.6% | 第二常见；常与 0x0002 共存于同一 bucket |
| `0x0001` | 18  | 3.1% | 第三常见 |
| `0x4C1C` | 8   | 1.4% | size=16 bucket 专属 |
| `0x4E1C` | 8   | 1.4% | size=16 bucket 专属 |
| `0x8EA5` | 7   | 1.2% | size=86 bucket 专属（85% 覆盖） |
| `0x4E1D` | 6   | 1.0% | size=16 bucket 专属 |
| `0x0004` | 5   | 0.9% | 与 0x0001-0x0003 同族 |

### "Discriminator-friendly" size buckets（word@+0 单一覆盖 ≥ 80%）

| Size | Count | Dominant word | 覆盖率 |
|---:|---:|---|---:|
| 12 | 1   | `0x0002` | 100% |
| 15 | 7   | `0x0002` | 100% |
| 19 | 3   | `0x0002` | 100% |
| 22 | 2   | `0x0002` | 100% |
| 24 | 10  | `0x0001` | 90% |
| 25 | 6   | `0x0002` | 100% |
| 26 | 3   | `0x0002` | 100% |
| 27 | 9   | `0x0002` | 100% |
| 29 | 4   | `0x0002` | 100% |
| 36 | 11  | `0x0003` | 100% |
| 37 | 3   | `0x0002` | 100% |
| 41 | 16  | `0x0002` | 100% |
| 42 | 4   | `0x0002` | 100% |
| 45 | 8   | `0x0002` | 100% |
| 47 | 4   | `0x0002` | 100% |
| 50 | 16  | `0x0002` | 81% |
| 69 | 4   | `0x0002` | 100% |
| 76 | 24  | `0x0002` | 83% |
| 86 | 7   | `0x8EA5` | 85% |
| ... | ... | ... | ... |

共 ~30 个 size bucket、~280 records 满足 "single-word discriminator
clean partition" 条件。

### "Heterogeneous" size buckets（无单一 word 覆盖 ≥ 50%）

| Size | Count | Top word @ +0 | 说明 |
|---:|---:|---|---|
| 31 | **182** | `0x0048`=1% / `0x00EF`=1% | 最大 bucket，明显 NOT 在 +0 discriminator |
| 70 | 53  | `0x4016`=5% / `0x5200`=5% | 几乎肯定包含坐标 / OID |
| 13 | 21  | `0x066C`=14% / `0x27B8`=14% | 紧凑型多形态 |
| 43 | 18  | `0xBE2E`=11% | |
| 46 | 21  | `0x0002`=19% | 部分覆盖 |
| 74 | 11  | 无 dominant | |
| 16 | 24  | `0x4C1C`/`0x4E1C` 双峰 | 16-byte records 自带 sub-family |
| 58 | 10  | `0xE3D9`=30% / `0x355F`=20% | |
| 17 | 5   | 无 dominant | |
| 28 | 2   | `0x5782`/`0xF9A0` 各 50% | 样本小 |

这 ~250 records（含 size 31 的 182 大 bucket）在 `+0` 处不是
discriminator——它们的 sub-kind 区分位（如果存在）在其它偏移，或者
本身就是不同 record class 的多态成员。

### 设计哲学

- Phase 14 GArc2d 错误命名教训：**没有 IDA 证据不命名字段语义**。
  即使 0x0002 出现率 28%，本 phase 也**不命名**为 `sub_kind` /
  `record_kind` / `family_tag`。
- Phase 15 GraphicGroup audit 模板：暴露原始 bytes + 完整 provenance
  + ratchet count，**等下游 IDA 工作来命名**。
- Phase 18 audit-only 模板：mirror。

## 上下文（必读）

| 文档 / 文件 | 作用 |
|---|---|
| `goals/phase18-psm-0x0010-sub-record/brief.md` | Phase 18 sub-record audit-only 设计 |
| `goals/phase18-psm-0x0010-sub-record/plan.md` | Phase 18 8 slice 模板 |
| `src/parsers/sheet_records.rs::decode_sub_records_0x0010` | 现有 decoder，本 phase 在其上加 leading_word |
| `src/model.rs::DecodedSubRecord0x0010Record` | 现有 model DTO，加新字段 |
| `src/schema.rs` | schema needles，加 `leading_word` |
| `tests/parse_real_files.rs::sub_records_0x0010_decoder_emits_audit_records_with_provenance` | Phase 18 ratchet，本 phase 加新 ratchet |
| `examples/probe_psm_0x0010_sub_kind.rs` | Phase 19 probe（本 phase 新增） |
| `docs/analysis/2026-05-17-phase19-rad-sibling-probe-null-result.md` | RAD sibling-sweep null-result（解释为什么本 phase 选 leading-word 而不是 sibling sweep） |

## 关键约束

- **不命名 sub-kind 字段**：新字段名必须是 `leading_word`（描述字节
  位置，不描述语义）。**禁止**用 `sub_kind`、`record_kind`、
  `family_tag`、`payload_kind` 等暗示语义的命名。
- **`leading_word` 必须是 `Option<u16>`**：payload 长度 < 2 时为
  `None`。本 phase probe 没看到 < 2 的 payload，但 schema 必须允许。
- **不引入 `PidGraphicKind` variant**：0x0010 仍是 sub-record。
- **不解析 reference chain**：Phase 19 不实现 JStyleOverride →
  0x0010 oid lookup。
- **不修改 Phase 18 既有字段**：`byte_range / type_code / type_flags
  / bytes_to_follow / raw_payload` 全部保持。
- **不退化 Phase 14/15/16/17/18 任何 baseline**。
- **不退化 Phase 18 audit count = 582**：本 phase 只加字段，不调
  validation。
- **panic-safe**：新字段提取必须用 `payload.get(0..2)`，不直接索引。
- 5 道 pre-commit gate 全绿：build / test / clippy -D warnings /
  fmt --check / missing-docs baseline=0。

## 非目标

- **不**反向 0x0010 sub-kind discriminator 偏移分布（size 31/70/13/16
  bucket 在 +0 不是 discriminator，本 phase 不深挖）。
- **不**实现 reference resolver。
- **不**新增 `PidGraphicKind` variant。
- **不**提取 plant instrument tag。
- **不**做 IDA 加载（Phase 18 blockers 列为 Stop-And-Ask）。
- **不**新增 fixture。
- **不**提交 `dlls/`、`.i64`、私有 fixture。
- **不**commit / push，除非用户明确授权。

## Ask Before（要先问）

- 偏离 audit-only 模板，把 `leading_word` 重命名为 `sub_kind` 之类
  语义字段。
- 加 `leading_word == 0x0002` 之外的额外字段（如 `leading_dword:
  u32`、`per_size_discriminator: u8`）。
- 加载 IDA instance（确认 0x0010 sub-kind 真实身份）。
- 任何 commit / push / 删除已存在测试前。
- 把 `leading_word == 0x0002` ratchet 数字向下调整（说明 decoder
  validation 太严格或 leading_word 提取规则不对）。
- 修改 Phase 18 ratchet 582 数字。
- 把 audit collection 升级为 typed sub-kind 集合。

## Done Means（完成判据）

同时满足：

1. `src/parsers/sheet_records.rs::SheetSubRecord0x0010Decoded` 新增
   `leading_word: Option<u16>` 字段。`decode_sub_records_0x0010` /
   `decode_sub_record_0x0010_at` 在 payload 长度 ≥ 2 时填充
   `Some(u16::from_le_bytes([payload[0], payload[1]]))`，否则 `None`。
   不修改 Phase 18 其它字段。
2. `src/model.rs::DecodedSubRecord0x0010Record` mirror 新增
   `leading_word: Option<u16>`；`From` impl 同步；`JsonSchema` derive
   自动覆盖。
3. `src/schema.rs` 默认 schema 加 `leading_word` needle（ratchet test
   验证）。
4. `src/streams/cluster.rs` / `src/cfb/reader.rs` / `src/geometry.rs`
   无需修改（新字段是值字段，不影响管线）。
5. `tests/parse_real_files.rs` 新增 cross-fixture ratchet test
   `sub_records_0x0010_leading_word_distribution_matches_phase19_probe`，
   断言：跨 fixture `leading_word == Some(0x0002)` 计数 = **164**；
   `leading_word == Some(0x0003)` 计数 = **21**；
   `leading_word == Some(0x0001)` 计数 = **18**；
   `leading_word == None` 计数 = **0**。
6. Phase 18 cross-fixture ratchet 仍输出 **582**（不退化）。
7. `tests/parser_panic_safety.rs` adversarial matrix **无需更新**
   （没有新 public parser entry）。但运行该测试必须通过。
8. `CHANGELOG.md` 写明 Phase 19 audit-only 字段扩展 + leading_word
   distribution + 不命名 sub-kind 的设计选择 + 与 Phase 18 的
   additive 关系。
9. 5 道 pre-commit gate 通过，`missing_docs` baseline 不上升（=0）。
10. Phase 14/15/16/17/18 baseline 全部保持。
11. `progress.jsonl` 对每个 AC 都有命令 / artifact / 输出摘要。

停止条件全部写入 `blockers.md`。
