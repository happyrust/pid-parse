# Controlled `.pid` diff fixtures

> 协议：`docs/protocols/2026-05-13-controlled-pid-diff-collection.md`
> 消费侧：`pid_parse::inspect::controlled_diff`
> Goal package：`goals/phase14-plan-b-controlled-diff-protocol/`

本目录用于存放按 [采集协议](../../docs/protocols/2026-05-13-controlled-pid-diff-collection.md)
产出的 `before/<case>.pid` + `after/<case>.pid` + `metadata/<case>.json`
三件套。

**真实 `.pid` 文件被 `.gitignore` 阻止入 git**（plant 数据敏感）。
本目录的 `before/`、`after/` 子目录通过 `.gitkeep` 占位文件保持
在 git 中可见，但 `*.pid` 实际内容仅在本地或团队私有传输通道
存在。

`metadata/*.json` 文件**可以**入 git（它们只包含人写元数据，
无 plant bytes），但若 `notes` 字段含敏感信息，请先脱敏。

## 自检（采集完成后）

```powershell
cargo run --release --bin pid_inspect -- --controlled-diff-dir test-file/controlled-diff
```

预期 stdout / JSON 形式见协议 §6。
