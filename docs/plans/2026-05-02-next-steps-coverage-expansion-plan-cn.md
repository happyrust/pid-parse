# PID 解析覆盖率扩展方案

> 日期：2026-05-02
> 基线：promotable=5/12 CE0079 (42%)，5 near-miss 待解锁
> 目标：提升到 promotable=10/12 (83%)，解锁 Line 几何

## 0. 当前状态速览

```text
promoted:    326, 239, 452, 467, 602  (5个)
near-miss:   111, 537 (有identity无shape), 139, 147, 440 (有shape无identity)
distant:     35, 157, 433, 490, 68   (5个，差两条腿)
```

## 1. 解锁策略

### 策略 A：Fixture 扩容 → 增加 Shape Support

**目标**：为 fx=111, 537 提供 StableChunkShape evidence。

**原理**：这两个候选 score=75 且有 identity，只差 chunk-relative shape support >= 2。如果在其他 fixture 的 Sheet 中也发现相同 field_delta/coordinate_delta 模式，shape support 会累加。

**任务**：
1. 收集更多 `.pid` 样本（同一 plant 不同图纸、不同项目模板）
2. 在新 fixture 上运行 evidence inventory
3. 检查新 fixture 中 fx=111, 537 对应对象的 chunk-relative delta
4. 如果新数据引入同 shape class 的其他 field_x → support 提升 → StableChunkShape 触发

**验收**：
```powershell
cargo test --test parse_real_files available_pid_fixtures_geometry_evidence_inventory -- --nocapture
```

### 策略 B：Identity Index 增强 → 为 Shape-only 候选补 Identity

**目标**：为 fx=139, 147, 440 提供 GraphicIdentityNearby evidence。

**原理**：这三个候选 score=70 且有 shape，只差 identity。当前 identity 来源是 DA trailer 的 record_id 匹配。如果能增加 identity 来源（如 DrawingID ASCII/UTF-16LE），这些候选可能获得 identity。

**任务**：
1. 检查 fx=139, 147, 440 的 DA trailer 是否有 record_id
2. 检查这些 field_x 周围是否有 DrawingID hex 字符串
3. 如果 DrawingID 存在但未被当前 identity scanner 捕获 → 扩大扫描范围
4. 或检查是否有 same-object identity 在其他窗口

**验收**：
```powershell
cargo test --test parse_real_files sheet6_graphic_identity_scoring -- --nocapture
```

### 策略 C：CE0079 签名直接定位 → 跳过 Window 扫描

**目标**：利用已知的 CE0079 记录头部签名直接提取坐标。

**原理**：CE0079 签名后 +6 是 field_x，+N 是坐标字段（delta 已通过 promoted candidates 确认）。可以直接从签名位置提取坐标，不依赖 field_x_windows 的间接匹配。

**任务**：
1. 新增 `scan_ce0079_records` 函数，直接在 Sheet 流中扫描 CE 00 79 00
2. 提取每个签名后的 field_x 和对应 coordinate delta 处的坐标
3. 如果坐标通过质量过滤 → 作为 `RecordHeaderBacked` provenance 的 hint
4. 这条路径绕过了 identity/shape gate，需要新的验证方式

**风险**：CE0079 可能不是唯一的记录头部模式，直接依赖可能遗漏其他类型的对象。

**验收**：
```powershell
cargo test --test parse_real_files ce0079_direct_extraction -- --nocapture
```

## 2. 优先级排序

| 优先级 | 策略 | 投入 | 预期收益 | 风险 |
|---|---|---|---|---|
| P0 | B: Identity 增强 | 低 | 解锁 3 candidates | 低 |
| P1 | C: CE0079 直接提取 | 中 | 解锁全部 12 CE0079 | 中（需新验证） |
| P2 | A: Fixture 扩容 | 高（需样本） | 解锁 2 + 交叉验证 | 低 |

## 3. Line 几何生成前置条件

当 promotable >= 8 且坐标分布合理时：
1. 同一 record shape class 内的 points 可连线
2. 同一 field_x 对应的 endpoint 关系可提供连接方向
3. 生成 `PidGraphicKind::Line` + `Inferred` confidence
4. H7CAD 新增 `PID_GEOM_LINES` 层

## 4. 验证命令汇总

```powershell
cd D:\work\plant-code\cad\pid-parse
cargo test --locked --workspace --all-targets
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo test --test parse_real_files -- --nocapture

cd D:\work\plant-code\cad\H7CAD
cargo test -p H7CAD --bin H7CAD pid_import -- --nocapture
cargo check --locked --workspace --all-targets
```
