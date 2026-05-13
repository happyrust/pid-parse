# 验证：Phase 14 SPPID Sheet 几何 primitive 解码器

## 验证命令矩阵

| 命令 | 目的 | 通过判据 | 证据存放点 |
|---|---|---|---|
| `cargo build --locked --workspace --all-targets` | 全 workspace 编译干净 | exit 0，无 warning | `progress.jsonl`（含 cargo 输出尾 5 行） |
| `cargo test --locked --workspace --all-targets` | 全 970+ 测试通过 | exit 0，每个 `test result: ok. N passed; 0 failed` 都 N>0 或显式 0 但非 failed | `progress.jsonl` + `target/debug/...` 报告路径 |
| `cargo clippy --locked --workspace --all-targets -- -D warnings` | 0 warning / error | exit 0 | `progress.jsonl` |
| `cargo fmt --all -- --check` | 代码格式干净 | exit 0 | `progress.jsonl` |
| `cargo rustdoc --lib --locked -- -W missing-docs` + missing_docs ratchet | rustdoc 私有 link 干净 + missing_docs count = baseline | exit 0 且 `Select-String 'missing documentation for' \| Measure-Object` → 0 | `progress.jsonl` |
| `cargo test --locked -j 1 --test parse_real_files primitive_line_decoder_emits_decoded_lines_with_provenance -- --nocapture` | **AC5–AC7** 决定性测试：至少 1 条 decoded line | exit 0；输出包含 `decoded_line_count >= 1`；每条 line 携带 `byte_range` + `record_kind=PrimitiveLine` | `progress.jsonl` 全文输出 |
| `cargo test --locked -j 1 --test parse_real_files dwg0201_produces_inferred_endpoint_lines -- --nocapture` | **AC8** 回归保护：现有 inferred line 不退化 | exit 0；`inferred_lines >= 49`；`inferred_points >= 117` | `progress.jsonl` |
| `cargo test --locked -j 1 --test parse_real_files decoded_geometry_provenance_record_kind_matches_payload_kind -- --nocapture` | **AC9** provenance 三件套一致性 | exit 0；所有 decoded entity 的 `record_kind == PidGraphicKind::decoded_sheet_record_kind()` | `progress.jsonl` |
| `cargo test --locked -j 1 --test parser_panic_safety` | **AC6** panic-safety smoke | exit 0；新增的 `decode_primitive_line` 入口在 adversarial 字节下不 panic | `progress.jsonl` |
| `cargo test --locked -j 1 --test parse_real_files geometry_fixture_inventory_reports_normalized_geometry_counts -- --nocapture` | 跨 fixture inventory：decoded/inferred/probe-only 计数稳定 | exit 0；输出含 `decoded_line >= 1` 列 | `progress.jsonl` |
| `gh run watch <run-id> --exit-status` | **AC10** CI 远端验证 | exit 0；GitHub Actions 11 步全过 | `progress.jsonl` 含 run URL |

## 手工 / 半手工核查

| 核查项 | 怎么做 | 通过判据 |
|---|---|---|
| IDA 实例状态 | `CallMcpTool user-ida-pro-mcp list_instances` | 至少含 `rad2d.dll.i64` 一个新实例，`reachable: true` |
| `OpenStream("Sheet*")` 反编译可见 | 选中 IDA 实例 → `CallMcpTool user-ida-pro-mcp decompile <addr>` | 反编译输出包含 `OpenStream` / `CreateStream` 调用 + 字符串 `"Sheet"` 拼接 |
| record kind 派发函数可见 | 同上，`decompile <dispatcher_addr>` | 反编译输出含 switch / 跳表，至少 5 个 case 分支 |
| `PrimitiveLine` 字节布局文档与真实 hex 对得上 | 在 `DWG-0201GP06-01.pid /Sheet6` 用 `pid_inspect --probe-sheet --probe-sheet-chunks Sheet6` 抽 3 条 record，按 `docs/analysis/2026-05-XX-rad2d-primitive-line-layout.md` 解 | 至少 3 条 record 的字段值（start.x / start.y / end.x / end.y）解出来是 finite f64 且落在图纸坐标域 |
| Coverage 分级未误升 | 看 `src/inspect/coverage.rs` 输出对 `Sheet*` 的分类 | 没有 documentation-only 升级；任何升级都对应 typed decoder 在该 fixture 上真实消费 |
| `controlled_diff` 不变式未被破坏 | `cargo test --locked --lib inspect::controlled_diff` | 5 单测 + 1 doc-test 全过；`promoted_geometry` 仍为 `false` 硬编码 |

## 证据规则

- 每运行一条命令，在 `progress.jsonl` append 一条 JSON line：

```json
{
  "type": "verification",
  "command": "cargo test ...",
  "exit": 0,
  "timestamp": "2026-05-14T...Z",
  "ac": ["AC7"],
  "artifact": "path-to-output-file-or-cargo-output-snippet"
}
```

- IDA 反编译 / 字节布局类核查 append `type: "ida_artifact"`：

```json
{
  "type": "ida_artifact",
  "ida_instance": "rad2d.dll@13346",
  "addr": "0x...",
  "kind": "decompile|struct|callsite|recordkind_dispatch",
  "ac": ["AC2", "AC3"],
  "artifact": "docs/analysis/2026-05-XX-rad2d-sheet-callsites.md#section"
}
```

- 不允许只声明 "测试通过"。每条 evidence 必须能从 `progress.jsonl`
  反查到具体命令 / 文件 / 行号 / IDA 函数地址
- 不允许把 inferred line 误标 decoded：`decoded_line_count` 计数器
  必须只对 `PidGeometryConfidence::Decoded` 累加
- 半手工核查（IDA 反编译可见性、字节布局对账）必须**截图或粘贴**核
  心反编译输出 + 真实 hex 到 `progress.jsonl` 的 artifact 字段

## 收口检查

merge 前最后一次确认（按顺序跑）：

```powershell
cargo build --locked --workspace --all-targets
cargo test --locked --workspace --all-targets
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
cargo rustdoc --lib --locked -- -W missing-docs
# 5 道 pre-commit gate 必须 5/5 绿

git push origin <branch>
gh run watch <run-id> --exit-status
# CI 11 步必须 11/11 绿
```

每一步 append `progress.jsonl`。Gate fail 立即停手、写 `blockers.md`、
回 Slice D 或 Slice C 排查。
