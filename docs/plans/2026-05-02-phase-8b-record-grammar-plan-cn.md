# Sheet Record Grammar 反推与坐标验证方案

> 日期：2026-05-02
> 目标：区分 Sheet 流中的结构字段和真实对象坐标，为 Line/Text 升级铺路
> 前置：Phase 8C-9B 链路已通，5 个 promoted points 中 4 个坐标疑似结构常量

## 0. 当前观察

5 个 promoted geometry hints 的坐标分析：

| field_x | score | coordinate | offset | 观察 |
|---|---|---|---|---|
| 239 | 105 | (206, 121) | 2008 | 重复坐标 |
| 452 | 105 | (206, 121) | 3272 | 重复坐标 |
| 602 | 105 | (206, 113) | 4956 | 类似 |
| 326 | 95 | (206, 121) | 1132 | 重复坐标 |
| 467 | 95 | (206, 121) | 3272 | 与 452 同位置 |

关键疑问：
- `0xCE=206` 和 `0x79=121` 多次出现，可能是记录头/分隔符常量
- 两个不同 field_x (452, 467) 共享同一 coordinate offset (3272)
- 这些值是否为 SmartPlant 单位系统中的有效坐标？

## 1. 调查方向

### 1.1 结构常量识别

在 Sheet 流中扫描 `0xCE` 和 `0x79` 的出现频率：

- 如果 `CE 00 79 00` 在非坐标上下文中也高频出现 → 确认为结构常量
- 如果仅在有 field_x 的 record 附近出现 → 可能是有效坐标范围标记

任务：
1. 统计 `CE 00 79 00 00 00` 在 Sheet6 中的全部出现位置
2. 将这些位置与已知 chunk 边界、endpoint record、object field_x 交叉比对
3. 如果确认为结构字段，将其加入 `is_structural_coordinate_value` 过滤器

### 1.2 Record 边界识别

当前 chunk boundary 检测基于 zero-run 和 magic bytes。需要补充：

- **Header 签名扫描**：检查 promoted records 的起始字节模式
  - 排名 1-3 的 records 周围都有 `01 56 00 01`（offset-8 处）
  - 排名 4 有 `FA 00 36 00`
  - 这些可能是记录类型标识符

- **Record 长度推导**：
  - 对比相邻 field_x 的 offset 差值
  - 如果差值稳定（如 100-120 bytes），说明记录定长或有 header+body 结构

任务：
1. 提取 top-5 promoted records 的 ±64 byte 上下文
2. 对齐寻找重复字节模式
3. 建立初步 record header/body/trailer 假说

### 1.3 坐标字段位置验证

需要确定 "coordinate_delta_from_chunk" 指向的是否真是坐标：

- 检查该位置的 i32 pair 是否在 SmartPlant 坐标范围内
- SmartPlant 坐标通常是 twips (1/1440 inch)，图纸范围约 0-50000
- 如果值 < 300 且高度重复，更可能是 enum/flag 而非坐标

任务：
1. 收集所有 promoted 和 near-promoted candidates 的坐标值分布
2. 生成直方图/统计摘要
3. 与已知 SmartPlant 坐标范围比对

## 2. 实施阶段

### Phase 8B-1：结构字段过滤器增强

目标：确认 (206, 121) 是否为结构常量并加入过滤。

任务：
- 新增 `sheet_byte_pattern_frequency` 分析函数
- 统计候选坐标值在 Sheet 流中的出现频次
- 高频 + 非 object-context → 加入 `is_structural_coordinate_value`
- 更新 evidence inventory 输出

验收：
```powershell
cargo test --test parse_real_files coordinate_frequency -- --nocapture
cargo test --locked --workspace --all-targets
```

### Phase 8B-2：Record 签名探测

目标：识别 Sheet record 的头部签名和长度模式。

任务：
- 新增 `sheet_record_signature_probe` 函数
- 扫描 promoted windows 周围的重复字节模式
- 输出签名候选 + 推测记录长度
- 为后续 record parser 提供种子数据

验收：
```powershell
cargo test --test parse_real_files record_signature -- --nocapture
```

### Phase 8B-3：坐标范围验证

目标：建立坐标值可信度评估。

任务：
- 新增 `coordinate_value_confidence` 评估
- 考虑：值范围、唯一性、与已知 SmartPlant 坐标系的一致性
- 如果 >50% promoted coordinates 为结构常量 → 需要更精确的坐标字段定位

验收：
```powershell
cargo test --test parse_real_files coordinate_confidence -- --nocapture
```

## 3. 成功标准

Phase 8B 完成标志：
1. 至少确认 1 类 Sheet record header 签名
2. 坐标字段过滤器排除已确认的结构常量
3. 至少 1 个 promoted coordinate 值在 SmartPlant 坐标范围内（或证明当前所有均为结构字段）
4. 所有测试通过，无 regression

## 4. 风险

| 风险 | 影响 | 缓解 |
|---|---|---|
| 所有坐标都是结构字段 | 当前 5 个 hints 全部无效 | 不降低 gate，而是改进坐标字段定位 |
| Record 签名不稳定 | 无法建立可靠 parser | 保持 probe-only，不急于 decode |
| Fixture 太少看不出规律 | 结论不可靠 | 优先 Phase 8A fixture 扩容 |
