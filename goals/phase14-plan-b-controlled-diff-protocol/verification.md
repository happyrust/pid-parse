# 验证：Phase 14 Plan B 控制 diff fixture 采集协议

> **[DRAFT — awaiting plannotator gate]**

## 验证命令矩阵

| 命令 | 目的 | 通过判据 | 证据存放点 |
|---|---|---|---|
| `cargo build --locked --workspace --all-targets` | 编译 | exit 0 | `progress.jsonl` |
| `cargo test --locked --workspace --all-targets` | 全测试 | exit 0；测试数比上次 ≥ 1（新增协议自检） | `progress.jsonl` |
| `cargo test --locked --test inspect_cli controlled_diff_protocol_synthetic_two_case_walkthrough -- --nocapture` | **AC6** 决定性测试 | exit 0；输出 2 个 case，promoted_geometry=false | `progress.jsonl` |
| `cargo clippy --locked --workspace --all-targets -- -D warnings` | clippy 干净 | exit 0 | `progress.jsonl` |
| `cargo fmt --all -- --check` | 格式 | exit 0 | `progress.jsonl` |
| `cargo rustdoc --lib --locked -- -W missing-docs` | missing_docs ratchet | 0 = baseline 0 | `progress.jsonl` |
| `git ls-files test-file/controlled-diff/` | **AC5** | 仅 `.gitkeep` + 协议 README，无任何 `.pid` | shell 输出 |
| `gh run watch <run-id> --exit-status` | **AC7** CI | exit 0 | `progress.jsonl` |

## 手工核查

| 核查项 | 怎么做 | 通过判据 |
|---|---|---|
| 协议可读性 | 让另一个人通读协议文档 1 遍 | 读完能讲出"我能怎么去采 line case fixture"的步骤序列 |
| `ControlledDiffMetadata` schema 对齐 | 协议里的 sidecar JSON 字段表 × `controlled_diff.rs::ControlledDiffMetadata` 源码 diff | 字段名 / 类型 / 必选-可选 完全一致 |
| 协议自检步骤可重复 | 在新 powershell 窗口跑协议第 5 节的"自检"命令 | stdout / JSON 形状与协议 example 一致 |
| Plant 数据安全 | `git ls-files test-file/controlled-diff/` 不含真实 `.pid` | 仅占位文件，无 plant-proprietary 数据 |

## 证据规则

每运行一条命令，append `progress.jsonl`：

```json
{
  "type": "verification",
  "command": "cargo test ...",
  "exit": 0,
  "timestamp": "2026-05-XX...Z",
  "ac": ["AC6"],
  "artifact": "cargo output tail"
}
```

协议文档章节完工 append:

```json
{
  "type": "protocol_section_complete",
  "section": "smartplant_operation_steps",
  "case_count": 6,
  "doc_path": "docs/protocols/2026-05-XX-controlled-pid-diff-collection.md"
}
```

## 收口检查

merge 前最后一次跑：

```powershell
cargo build --locked --workspace --all-targets
cargo test --locked --workspace --all-targets
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
cargo rustdoc --lib --locked -- -W missing-docs
git push origin <branch>
gh run watch <run-id> --exit-status
```

任一 fail 立即停手 + 写 blockers.md + 回相应 Slice 排查。
