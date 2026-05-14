# SmartPlant P&ID 控制 diff fixture 采集协议

> 版本：2026-05-13 (v1)
> 适用：SmartPlant P&ID 12.x
> 目标读者：拥有 SmartPlant P&ID 工作站合法访问权限的工程师 / 操作员
> 配套消费侧：`pid_parse::inspect::controlled_diff` (commit `54e5c06`)
> Goal package：`goals/phase14-plan-b-controlled-diff-protocol/`

## 1. 协议目的与边界

### 1.1 为什么需要这份协议

`pid-parse` 项目的 Phase 14（Sheet primitive 解码器）有两条互补的
byte-range 证据链：

- **A 链：IDA Pro 逆向 SPPID 运行时 DLL**——证据是反编译的字段
  偏移表，但被 B1（缺 `rad2d.dll` / `pidobjectmanager.dll`）硬阻塞，
  详见
  `docs/analysis/2026-05-13-ida-pro-mcp-reconnaissance.md`
- **B 链：受控 `.pid` 编辑前后字节 diff**——证据是 SmartPlant 自
  己写入的 byte 变更范围。**本协议是 B 链的可执行入口**

操作员按本协议在 SmartPlant 上做单一原子编辑、保存两个 `.pid`
快照、配 metadata sidecar，得到的目录就是 `pid_inspect
--controlled-diff-dir <root>` 的合法输入，进而产生
`ControlledDiffEvidenceReport`，把 SmartPlant 写入的 byte 差异变成
typed decoder 的输入证据。

### 1.2 协议交付的不是什么

- **不是** SmartPlant P&ID 安装 / 配置 / 入门指南
- **不是** Phase 14 typed decoder 的实现指引
- **不是** 自动化脚本（VBA / AutoIt 等半自动化方法**禁用**，
  因为会污染 byte diff 的可解释性）

### 1.3 一致性硬约束

| 维度 | 要求 |
|---|---|
| SmartPlant 版本 | 同一 case 的 before / after 必须同一个 SmartPlant minor version（12.0 vs 12.1 输出可能不同字节） |
| 操作员 | 同一 case 的 before / after 必须同一登录用户 |
| 时间窗 | 同一 case 的 before / after 必须连续操作（< 5 分钟），中间不要切换图 / 项目 / 设置 |
| 模板 | 所有 case 在同一空白模板上做（推荐 `A2-W-New.pid` 或团队约定基线） |
| 视图 / 缩放 | 不影响字节，但保持一致便于 review |
| 原子性 | **每个 case 必须只放 1 个对象 / 改 1 个属性 / 移 1 个对象**——禁止"放 1 条线 + 改一个 tag"叠加 |

任何一条违反，case 视为污染，要重做。

## 2. 前置条件

操作员侧：

- 一台装有 SmartPlant P&ID 12.x 的工作站，带有效 license
- 能用「File → Save As」把当前编辑窗口的 `.pid` 文件另存到任意路径
- Windows / PowerShell 基本操作能力
- 本仓库 (`pid-parse`) clone 到工作站本地，能跑
  `cargo run --bin pid_inspect`（构建一次：`cargo build --release --bin pid_inspect`）
- 安装好 `gh` CLI 或者能把目录打 zip 邮件发出来（如果工作站与项目仓
  库不在同一台机器）

工程侧（已就绪，不需要操作员动）：

- `pid_parse::inspect::controlled_diff` 模块已落地（`src/inspect/controlled_diff.rs`）
- `pid_inspect --controlled-diff-dir` 已落地（`src/bin/pid_inspect.rs`）

## 3. 目录约定

每次"一批采集"（例如：6 个 case，分别对应 line / polyline / circle /
arc / text / symbol）放在同一个根目录下，命名约定：

```
<batch_root>/
├── before/
│   ├── <case>.pid          # 操作前快照
│   └── ...
├── after/
│   ├── <case>.pid          # 操作后快照（与对应 before/<case>.pid 同名）
│   └── ...
├── metadata/
│   ├── <case>.json         # 与对应 .pid 同名的 sidecar
│   └── ...
└── README.md               # 该批采集的人读说明（操作员、时间、模板、SmartPlant 版本）
```

`<case>` 推荐命名：`line-01` / `circle-default-radius` / `text-tag-001` /
`symbol-vessel-vertical` 等——可读、不含空格、ASCII。

注意：

- `before/<case>.pid` 与 `after/<case>.pid` 文件名**必须一致**，
  `pid_inspect` 用文件名匹配 case
- `metadata/<case>.json` 文件名的 stem 必须与 `.pid` 文件名 stem 一致
- 同一 `<batch_root>` 下 case 名不重
- 实际 `.pid` 文件**不入 git**（`.gitignore` 已配置规则）；目录占位
  文件（`.gitkeep` / `README.md`）可以入 git

## 4. metadata sidecar JSON schema

字段对齐 `pid_parse::inspect::controlled_diff::ControlledDiffMetadata`：

| JSON 字段 | 类型 | 必选 | 含义 |
|---|---|---|---|
| `case` | string | ✓ | case 标识；必须与 `.pid` 文件名 stem 完全一致 |
| `operation` | string | ✓ | 人读 operation 名，建议 snake_case：`place_line` / `place_circle` / `place_polyline` / `place_arc` / `place_text` / `place_symbol` / `move_symbol` / `change_tag` 等 |
| `expected` | object \| array \| value | ✓ | 操作的预期几何 / 业务参数 payload，JSON 自由形（详见 §5 各 case 示例）。`pid_parse` 不解析它，仅作为人读凭据透传到 evidence report |
| `notes` | string | ✗ | 自由 notes：操作员姓名、时间戳、SmartPlant version、特殊情况 |

最小例（line case，必选三字段）：

```json
{
  "case": "line-01",
  "operation": "place_line",
  "expected": { "start": [0.0, 0.0], "end": [1.0, 0.0] }
}
```

完整例（line case，含 notes）：

```json
{
  "case": "line-01-template-a2",
  "operation": "place_line",
  "expected": {
    "start": [0.10, 0.10],
    "end": [0.20, 0.20],
    "coordinate_space": "normalized_page",
    "template": "A2-W-New.pid",
    "smartplant_version": "12.0 SP1"
  },
  "notes": "操作员 zhangsan, 2026-05-14 10:30 CST, 模板 A2-W-New.pid"
}
```

`expected` 字段不影响 `pid_inspect --controlled-diff-dir` 输出的 byte
diff，但**严格建议**包含：

- 几何坐标（如适用）：`start`, `end`, `center`, `radius`, `vertices`,
  `angle_start`, `angle_end`, `position`
- 坐标空间：`coordinate_space` ∈ `normalized_page` / `world` /
  `model_drawing_units`
- 模板：`template`
- SmartPlant 版本：`smartplant_version`

## 5. 六类 case 的 SmartPlant 操作步骤

每类 case 都遵循同一外层流程：

```mermaid
flowchart LR
    A[\u6253\u5f00\u6a21\u677f.pid] --> B[Save As before/<case>.pid]
    B --> C[\u505a\u552f\u4e00\u539f\u5b50\u64cd\u4f5c]
    C --> D[Save As after/<case>.pid]
    D --> E[\u8d34 metadata/<case>.json]
    E --> F[\u5173\u95ed\u4e0d\u4fdd\u5b58\u539f\u6a21\u677f]
    style C fill:#fef3c7,stroke:#f59e0b
    style E fill:#dcfce7,stroke:#16a34a
```

下面 §5.1 + §5.2 是**完整演示**（line + circle），其余 4 类
（§5.3–§5.6）只给操作步骤要点 + sidecar 模板，后续 batch 可按
§5.1 的详细度补全。

### 5.1 Case：`place_line`（**完整演示**）

#### 5.1.1 操作步骤

1. SmartPlant P&ID → File → Open → 选择模板 `A2-W-New.pid`（或团队基线）
2. 等模板完全加载（约 5-15 秒）
3. **File → Save As** → 目标路径 `<batch_root>/before/line-01.pid`
   → Save。此时 SmartPlant 仍然在打开该文件
4. **不要**点击图纸 / 调整视图 / 切换面板。直接进入下一步
5. 工具栏 → Piping 类目 → **Pipe Run** 工具（或快捷键，按团队习惯）
6. 在图纸空白处点一下放下起点（屏幕坐标可读，例如左下角靠中）
7. 移动鼠标到另一处（保持鼠标在同一图层，水平移动约 200 像素）
8. 再点一下放下终点
9. **Esc** 退出 Pipe Run 工具（避免连续多段画）
10. **File → Save As** → 目标路径 `<batch_root>/after/line-01.pid`
    → Save
11. **File → Close (不保存)** 关闭 SmartPlant 窗口

#### 5.1.2 metadata sidecar

`metadata/line-01.json`：

```json
{
  "case": "line-01",
  "operation": "place_line",
  "expected": {
    "start": [0.30, 0.40],
    "end": [0.45, 0.40],
    "coordinate_space": "normalized_page",
    "tool": "PipeRun",
    "template": "A2-W-New.pid",
    "smartplant_version": "12.0 SP1"
  },
  "notes": "操作员 zhangsan, 2026-05-14, A2-W-New 模板"
}
```

`start` / `end` 是**估算**的归一化坐标（从屏幕位置 / 标尺估读即可，
精度不影响 byte diff 评估）。

#### 5.1.3 预期 diff 形状

期望 `pid_inspect --controlled-diff-dir <batch_root>` 在该 case 报告：

- `stream_diffs >= 1`
- `modified_sheet_streams >= 1`（至少 `/Sheet6` 被修改）
- `first_modified.path` 以 `/Sheet` 开头（典型 `/Sheet6`）
- `only_in_before == 0` 且 `only_in_after == 0`（不应有 stream 增删）
- `first_modified.first_mismatch_offset` 是非零的 offset（不应该
  在 stream 起点；SmartPlant 通常追加新 record 在 Sheet 流的尾部
  附近）

如果实际 diff 形状与上面预期严重不符（例如 `only_in_after > 0` 或
`modified_sheet_streams == 0`），见 §7 故障排查。

### 5.2 Case：`place_circle`（**完整演示**）

#### 5.2.1 操作步骤

1. SmartPlant P&ID → File → Open → `A2-W-New.pid`
2. 模板加载完成
3. **File → Save As** → `<batch_root>/before/circle-default.pid`
4. 工具栏 → Equipment 类目 → **Vessel - Drum (Horizontal)** 或
   **Vessel - Drum (Vertical)** 任一（团队约定一个，固定即可），
   或更纯几何的选项：**Misc Annotations → Circle 注释**
   （**推荐使用 Misc Annotations Circle**，因为它最接近"纯几何
   primitive"——SmartPlant 把它作为 RAD2D circle 写入 Sheet 流，
   而 Vessel 是带业务对象的 symbol placement）
5. 在图纸空白处点一下放下圆心
6. 移动鼠标向外扩约 100 像素
7. 再点一下确定半径
8. **Esc** 退出工具
9. **File → Save As** → `<batch_root>/after/circle-default.pid`
10. **File → Close (不保存)**

#### 5.2.2 metadata sidecar

`metadata/circle-default.json`：

```json
{
  "case": "circle-default",
  "operation": "place_circle",
  "expected": {
    "center": [0.50, 0.50],
    "radius": 0.08,
    "coordinate_space": "normalized_page",
    "tool": "MiscAnnotations.Circle",
    "template": "A2-W-New.pid",
    "smartplant_version": "12.0 SP1"
  },
  "notes": "操作员 zhangsan, 2026-05-14, 使用 Misc Annotations Circle 而非 Vessel"
}
```

#### 5.2.3 预期 diff 形状

- `stream_diffs >= 1`，`modified_sheet_streams >= 1`
- `first_modified.path` 以 `/Sheet` 开头
- 与 `place_line` 相比，期望 `first_modified.len_after -
  first_modified.len_before` 更大（circle 至少 center + radius 共
  3 个 f64，line 是 4 个 f64，但 circle 通常带额外样式记录）

### 5.3 Case：`place_polyline`

#### 5.3.1 操作步骤要点

1. 工具：**Pipe Run** 工具（与 line 相同），但连续放 **3+ 段**
2. **File → Save As** before / after 与 §5.1 相同
3. 关键差异：第 5 步**不** Esc，连续 3 次点击不同位置，第 4 次点
   击 + 双击结束 polyline

#### 5.3.2 metadata sidecar 模板

```json
{
  "case": "polyline-3segments",
  "operation": "place_polyline",
  "expected": {
    "vertices": [[0.20, 0.30], [0.30, 0.30], [0.30, 0.40], [0.40, 0.40]],
    "segment_count": 3,
    "coordinate_space": "normalized_page",
    "tool": "PipeRun.MultiSegment"
  },
  "notes": "..."
}
```

### 5.4 Case：`place_arc`

#### 5.4.1 操作步骤要点

1. 工具：**Misc Annotations → Arc**（推荐，纯几何）
2. 三次点击：起点 → 弧上一点 → 终点（SmartPlant 用 3-point arc 模式）

#### 5.4.2 metadata sidecar 模板

```json
{
  "case": "arc-90deg",
  "operation": "place_arc",
  "expected": {
    "p1": [0.50, 0.20],
    "p2": [0.55, 0.25],
    "p3": [0.50, 0.30],
    "coordinate_space": "normalized_page",
    "tool": "MiscAnnotations.Arc",
    "arc_mode": "three_point"
  },
  "notes": "..."
}
```

### 5.5 Case：`place_text`

#### 5.5.1 操作步骤要点

1. 工具：**Notes → Free Text** 或 **General → Text** 注释
2. 在图纸空白处单击 → 输入文字 → Enter / Esc 提交
3. **重要**：文字内容保持简短可重复（建议固定字符串
   `"PID-TEST-001"`，避免每次随机文字污染 byte diff 对比）

#### 5.5.2 metadata sidecar 模板

```json
{
  "case": "text-fixed-001",
  "operation": "place_text",
  "expected": {
    "position": [0.10, 0.60],
    "text": "PID-TEST-001",
    "coordinate_space": "normalized_page",
    "tool": "General.Text"
  },
  "notes": "固定文本 PID-TEST-001 便于跨 case byte diff 比较"
}
```

### 5.6 Case：`place_symbol`

#### 5.6.1 操作步骤要点

1. 工具：从 Symbol Catalog 选定一个**单一 symbol**（推荐
   `Equipment → Vessel → Horizontal Drum`，团队约定唯一即可）
2. 单击图纸放下 symbol（不要 rotate / scale / 拖拽）
3. 关键差异：SmartPlant 此操作同时修改 Sheet 流（symbol 视图
   placement）+ Plant 业务对象表（new ModelItem entry）。预期
   `stream_diffs >= 2` 而不是 1

#### 5.6.2 metadata sidecar 模板

```json
{
  "case": "symbol-vessel-h-drum",
  "operation": "place_symbol",
  "expected": {
    "position": [0.40, 0.50],
    "symbol_path": "\\Equipment\\Vessels\\Horizontal Drums\\Horizontal Drum.sym",
    "coordinate_space": "normalized_page",
    "tool": "SymbolCatalog.Equipment.Vessel.HorizontalDrum"
  },
  "notes": "..."
}
```

#### 5.6.3 预期 diff 形状（差异）

- `stream_diffs >= 2`（Sheet 流 + Plant 业务对象表 / 关系流）
- `modified_sheet_streams >= 1`
- 可能 `only_in_after > 0`（新增 `/TaggedTxtData/...` 子流）

## 6. 验证步骤：把目录喂给 `pid_inspect`

采集完一批后，在工作站或项目根目录跑：

```powershell
# 假设在 pid-parse repo 根目录
cargo run --release --bin pid_inspect -- --controlled-diff-dir <batch_root> --json | Out-File -Encoding utf8 evidence.json
```

或人读形式（不带 `--json`）：

```powershell
cargo run --release --bin pid_inspect -- --controlled-diff-dir <batch_root>
```

### 6.1 期望 stdout（人读）

```text
Controlled PID diff directory: <batch_root>
Cases: 6

case=line-01 operation=place_line metadata=<batch_root>\metadata\line-01.json stream_diffs=1 modified_sheet_streams=1 only_in_before=0 only_in_after=0
first_modified path=/Sheet6 len_before=12345 len_after=12389 first_mismatch_offset=12340
before_context AA BB CC DD ...
after_context  AA BB EE FF ...

case=circle-default ...

(more cases)

No geometry promotion was performed.
```

### 6.2 期望 JSON

```json
{
  "root": "<batch_root>",
  "cases": [
    {
      "case": "line-01",
      "operation": "place_line",
      "expected": { "start": [0.30, 0.40], "end": [0.45, 0.40], ... },
      "stream_diffs": 1,
      "modified_sheet_streams": 1,
      "only_in_before": 0,
      "only_in_after": 0,
      "first_modified": {
        "path": "/Sheet6",
        "len_before": 12345,
        "len_after": 12389,
        "first_mismatch_offset": 12340,
        "before_context": "...",
        "after_context": "..."
      },
      "notes": "操作员 zhangsan, ..."
    },
    ...
  ],
  "promoted_geometry": false
}
```

注意 `promoted_geometry` 永远是 `false` —— `inspect::controlled_diff`
的 Phase 14 类型不变式保证了这一点（详见
`src/inspect/controlled_diff.rs` 的 `build_evidence_report`）。

### 6.3 自检 checklist

- [ ] 每个 case 输出 `stream_diffs >= 1`。如果某 case 是 0，说明
      "before / after 字节完全相同"——操作员错放成两个相同快照，
      重做
- [ ] 每个 case `only_in_before == 0` 且 `only_in_after <= 1`
      （后者允许 symbol case 有 1 个新流）。否则操作员在两次保存
      之间动了别的内容
- [ ] 每个 case `first_modified.path` 以 `/Sheet` 开头。否则可能是
      操作员在保存之间打开 / 关闭面板触发了 metadata 流更新
- [ ] `promoted_geometry == false`。否则下游消费侧出 bug —— 找
      `inspect::controlled_diff` 维护人

## 7. 故障排查

### 7.1 `stream_diffs == 0`

原因：before / after 字节完全一致。

可能：

- 在 Save As 后没做实际编辑就直接再 Save As
- 操作工具被取消（Esc 之后没真实放下对象）
- 操作员选错了"撤销保存"按钮

修复：重做 case，确认中间操作真实写入

### 7.2 `only_in_after > 1` 或 `only_in_before > 0`

原因：SmartPlant 在两次保存间隙做了非用户驱动的写入（auto-save / 拓
扑重算 / template 刷新）。

修复：

- 缩短两次 Save As 间的时间窗
- 关闭 SmartPlant 自动保存
- 重新打开 SmartPlant，干净状态重做

### 7.3 `first_modified.path` 不是 `/Sheet*`

原因：例如 `/\u0005SummaryInformation` 在变（OLE summary 时间戳更新）。

这不是错误，但表示 SmartPlant 同时改了非几何元数据。修复：

- 关闭 SmartPlant `Tools → Options → Auto-update document properties`
- 或接受这个 case 但在 `notes` 标注

### 7.4 `cargo run --bin pid_inspect` 报 parse error

原因：fixture `.pid` 文件被中途中断写入 / Save As 失败。

修复：检查文件大小（应在 KB-MB 级别，不应 < 10KB），重做

## 8. 数据安全

`.pid` 文件可能包含 plant-proprietary 数据。本协议产出的 fixture：

- **不**入项目 git（`.gitignore` 已配置 `test-file/controlled-diff/**/*.pid`）
- **不**通过开放渠道分享
- 团队内传输使用加密 zip 或 git-lfs（如团队约定）
- metadata sidecar JSON 可入 git（它只包含人写的元数据，无 plant
  bytes），但如果 `notes` 字段含敏感信息，需先脱敏

## 9. 后续工作（不在本协议范围）

按协议采集的 fixture 进入 `pid-parse` 后，下游 typed decoder 工作
（Slice C/D 在 `goals/phase14-sppid-sheet-geometry/`）会消费它来
反向 Sheet primitive 字节布局。本协议交付完成后，操作员可以重复
做以下扩展（独立工作，不需要 agent）：

- 同样 case 在不同模板上重做，对比 byte diff 一致性
- 同样 case 跨 SmartPlant minor version 对比（影响协议适用范围）
- 系列化 case：同种操作的 N 次重复，看 SmartPlant 自动生成的内部
  UID / GUID 是否稳定（Decoded confidence 升级的必要前提）

## 10. 变更日志

| 日期 | 版本 | 变更 |
|---|---|---|
| 2026-05-13 | v1 | 初版，覆盖 6 种原子操作、完整 line + circle 示例、`pid_inspect --controlled-diff-dir` 自检步骤 |
