# Verification: Phase 20 PSM 0x0010 IDA-confirmed RAD class identity

## Commands

本 phase 不改代码，所以没有新 `cargo test` 命令；5 道 pre-commit gate
仍要跑确认 Phase 14-19 baseline 不退化。

| Command | Purpose | Expected pass condition | Evidence location |
|---|---|---|---|
| `cargo build --locked --workspace --all-targets` | 确认 build 不退化 | exit 0 | `progress.jsonl` |
| `cargo test --locked --workspace --all-targets` | 确认所有 test 仍绿 | 851 lib + 90 integration, 0 failed | `progress.jsonl` |
| `cargo clippy --locked --workspace --all-targets -- -D warnings` | lint 不退化 | exit 0 | `progress.jsonl` |
| `cargo fmt --all -- --check` | format 不退化 | exit 0 | `progress.jsonl` |
| `cargo rustdoc --lib --locked -- -W missing-docs` | missing-docs baseline=0 不上升 | current=0 baseline=0 | `progress.jsonl` |
| IDA MCP `list_instances` (start + end of phase) | 确认未装载新 IDA instance | 开头和结尾 instance list 一致 | `progress.jsonl` |

## IDA-side Manual Checks

| Check | How | Pass condition |
|---|---|---|
| Class identity | analysis doc §1 | 含 RAD class 名 + CLSID + DLL + factory address |
| IO sequence | analysis doc §2 | 按 byte offset 列字段类型 |
| Sub-kind discriminator | analysis doc §3 | 含 offset + 数据类型 + 反编译片段 |
| Cross-fixture validation | analysis doc §4 | Phase 19 leading_word 数字与 IDA sub-kind enumeration 数字交叉对得上（至少一个 sub-kind = 164 records） |
| IDA address index | analysis doc §5 | 每个引用地址列基址相对偏移 + IDA port |
| Phase 16 chain reference | analysis doc §6 | 显式指明与 `JStyleOverride.referenced_oid_a/c` 的关系 |
| Known unknowns | analysis doc §7 | 明列剩余 audit-only 字段 / sub-kind |
| Phase 21 prerequisites | analysis doc §8 | 含 typed DTO 字段表草图 |

## Evidence Rules

- 每个 IDA tool call 后 append progress.jsonl entry：

```json
{"type":"ida_recon","timestamp":"...","ac":["AC1"],"ida_port":13346,"tool":"survey_binary","summary":"radsrvitem.dll: 5374 functions, 346 named. PSM dispatch table likely in unnamed functions; next: search_text for literal 0x10/0x0010 in .text."}
```

- 关键发现 append `ida_finding` entry：

```json
{"type":"ida_finding","timestamp":"...","ac":["AC1","AC2"],"finding":"0x0010 factory function at radsrvitem.dll+0x2A800. Reads bytes_to_follow at +6, then dispatches via vtable to sub-kind handler.","evidence":"reproduce via select_instance(13346) + analyze_function(0x564Cx800)"}
```

- 5 道 gate 写为单条 entry（gates 应该全绿因不改代码）：

```json
{"type":"gates","timestamp":"...","ac":["AC5"],"commands":["build","test","clippy","fmt","missing-docs"],"results":{"build":"ok","test":"ok 851 lib + 90 integration, 0 failed","clippy":"ok","fmt":"ok","missing_docs":"current=0 baseline=0"},"summary":"5/5 pre-commit gates green (no code changes, baseline preserved)."}
```

## 收口检查

merge / 完成前按顺序跑：

```powershell
cargo build --locked --workspace --all-targets
cargo test --locked --workspace --all-targets
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
cargo rustdoc --lib --locked -- -W missing-docs
```

任一 gate 失败：停止，记录 blocker。本 phase 不改 src/ 代码，gate 退化
意味着 Phase 19 commit 有意外副作用，需要排查。

## 完成签名

最后 append：

```json
{"type":"goal_complete","timestamp":"...","phase":"20","work_type":"reverse_engineering_only","rad_class":"<class name>","clsid":"<CLSID>","dll":"<dll name>","factory_address":"<addr>","sub_kind_discriminator_offset":<offset>,"sub_kind_discriminator_type":"<u8/u16/u32/u64>","sub_kind_enumeration":[{"value":"<v>","record_count":<n>}],"analysis_doc":"docs/analysis/2026-05-17-phase20-psm-0x0010-rad-class.md","phase14_baselines_preserved":true,"phase15_audit_preserved":true,"phase16_jstyle_preserved":true,"phase17_primitive_arc_removed":true,"phase18_audit_preserved":true,"phase19_leading_word_preserved":true,"normalized_geometry_unchanged":true,"gates":"5/5 green","src_code_changes":false,"new_ida_instances":false}
```

然后暂停等用户签收。Phase 21 typed DTO 实现需要单独 /goal 启动。
