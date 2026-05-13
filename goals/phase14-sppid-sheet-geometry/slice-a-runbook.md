# Slice A 执行 Runbook：定位 SPPID Sheet 流调用点

> 用法：B1 解锁（用户提供 `rad2d.dll` / `pidobjectmanager.dll` 等核
> 心 DLL）后，按本 runbook 一步步执行。每一步对应 progress.jsonl 的
> 一条 evidence 条目。完成本 runbook 即满足 plan.md AC1 + AC2。

## 前置条件

- B1 解锁判据：`CallMcpTool user-ida-pro-mcp list_instances` 含
  `rad2d.dll` 一个 `reachable: true` 实例（端口预计 13346+，
  IDA 自动分配）
- 用户已经把对应 `.dll` 文件放进 `D:\work\plant-code\cad\pid-parse\dlls\`
  （或 `E:\weixin\xwechat_files\happydpc_b2ec\msg\file\2026-05\bin\`
  待 agent 拷贝）

## 步骤 1：拷贝 DLL 进 dlls/ 并打开 IDA 实例

```powershell
# 假设用户把 DLL 放进微信 bin 目录
$src = "E:\weixin\xwechat_files\happydpc_b2ec\msg\file\2026-05\bin"
$dst = "D:\work\plant-code\cad\pid-parse\dlls"
foreach ($name in @("rad2d.dll", "pidobjectmanager.dll", "sigma2d.dll", "igrgcdt.dll", "rad2dapp.dll", "rad2dctrl.dll")) {
    $sp = Join-Path $src $name
    if (Test-Path $sp) {
        Copy-Item $sp $dst -Force
        Write-Host "copied $name"
    }
}
```

然后**用户**手动在 IDA Pro 里打开新拷贝的 DLL（agent 不能远程启动
IDA Pro GUI；IDA MCP server 会自动 attach 新进程）。

evidence: `{"type":"slice_a_step","step":"copy_and_open","dlls_copied":[...]}`

## 步骤 2：确认 IDA 实例上线

```text
CallMcpTool user-ida-pro-mcp list_instances {}
```

预期：返回 list 包含一个新条目 `binary == "rad2d.dll"`，
`reachable: true`，记录其 `port`（下面用 `<RAD2D_PORT>` 代表）。

evidence: `{"type":"slice_a_step","step":"ida_attached","port":<RAD2D_PORT>}`

## 步骤 3：切实例 + 摸底

```text
CallMcpTool user-ida-pro-mcp select_instance {"port": <RAD2D_PORT>}
CallMcpTool user-ida-pro-mcp survey_binary {"detail_level": "standard"}
```

记录：
- `metadata.module` / `arch` / `image_size`
- `statistics.{total_functions, named_functions}`
- `interesting_strings`（top 15）
- `imports_by_category.other`（找 `ole32` / `IStorage`)

evidence: `{"type":"slice_a_step","step":"survey","funcs":N,"strings":M}`

## 步骤 4：扫 Sheet stream 名字字符串

```text
CallMcpTool user-ida-pro-mcp find_regex {"pattern":"^Sheet[0-9]+$","limit":50}
CallMcpTool user-ida-pro-mcp find_regex {"pattern":"(?i)sheet[^a-z]","limit":50}
CallMcpTool user-ida-pro-mcp find_regex {"pattern":"(?i)(OpenStream|CreateStream|IStorage|IStream)","limit":50}
```

预期至少其中一个 hit：返回 `addr` 是字符串字面量的地址。把 hit 列
出，按地址排序。

evidence: `{"type":"slice_a_step","step":"sheet_strings","hits":[{addr,string}, ...]}`

## 步骤 5：对每个字符串 hit，找 xrefs 到它的函数

```text
CallMcpTool user-ida-pro-mcp xrefs_to {"addr":"<STRING_ADDR>"}
```

对每个 string hit 重复。每个 xref 对应一个函数地址 `<FN_ADDR>`。把
所有候选 fn 去重得到 `caller_set`。

evidence: `{"type":"slice_a_step","step":"sheet_string_xrefs","caller_set":[<addr>, ...]}`

## 步骤 6：对每个 caller 函数反编译看是否调 `OpenStream`

```text
CallMcpTool user-ida-pro-mcp decompile {"addr":"<FN_ADDR>"}
```

判定：反编译输出含 `OpenStream(` 或 `CreateStream(` 或
`IStorage->Open` 或类似模式 → 标为**确认 callsite**。否则丢弃。

evidence: `{"type":"slice_a_step","step":"decompile_callsite","addr":"<FN_ADDR>","confirmed":bool}`

## 步骤 7：把确认的 callsite 落到 docs/analysis/

新建文件 `docs/analysis/2026-05-XX-rad2d-sheet-callsites.md`，结构：

```markdown
# RAD2D Sheet stream callsites (Slice A 结果)

## 概要

- 反向 DLL：`rad2d.dll`（md5=...，size=...）
- IDA 实例端口：<RAD2D_PORT>
- 确认 callsite 数：N

## Callsite 表

| 函数地址 | 函数名 (IDA 显示) | 调用 stream | 反编译截取 |
|---|---|---|---|
| 0x... | `sub_...` | `Sheet0..N` | `OpenStream("Sheet%d", ...)` |
| ... | | | |

## 下一步

进入 Slice B：从这些 callsite 跟踪 `IStream::Read` → record kind dispatcher 函数。
```

evidence: `{"type":"slice_a_complete","file":"docs/analysis/2026-05-XX-rad2d-sheet-callsites.md","callsite_count":N}`

## 步骤 8：判断 AC1 + AC2 是否满足

- AC1（DLL 入 IDA + B1 resolved）：是否步骤 2 list_instances 返回
  含 `rad2d.dll`？是 → AC1 ✓
- AC2（至少 1 个 `OpenStream("SheetN")` 反编译可见）：是否步骤 6
  `confirmed` 数 ≥ 1？是 → AC2 ✓

两个都过 → commit `docs/analysis/2026-05-XX-rad2d-sheet-callsites.md`，
进入 Slice B。任何一个不过 → 在 `blockers.md` 加 entry，
stop-and-ask。

## Stop-and-ask 触发

- 步骤 2 找不到 `rad2d.dll` 实例 → 用户是否真的开了 IDA + 项目？
- 步骤 4 一个 Sheet 字符串 hit 都没有 → DLL 选错了？应该看
  `pidobjectmanager.dll` 而不是 `rad2d.dll`？
- 步骤 6 所有候选 caller 都不调 `OpenStream` → 间接调用？通过
  function pointer / vtable / COM `QueryInterface`？需要换搜法
  （搜 `IStorage` 类型用法 vs 字符串字面量）

## 时间预算

- 步骤 1–3：5 分钟
- 步骤 4–5：10 分钟（取决于字符串密度）
- 步骤 6：20–60 分钟（每个 callsite 反编译 + 阅读）
- 步骤 7：10 分钟落文档

整个 Slice A 预计 0.5–2 小时。超时立即写 stop-and-ask。
