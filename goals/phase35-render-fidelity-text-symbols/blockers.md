# Blockers: Phase 35 渲染保真

> 2026-07-26 创建。当前无硬阻塞。

## Active

（无）

## Resolved / Demoted

| 项 | 状态 | 说明 |
|---|---|---|
| 符号库在远程 `\\WIN-SPID\qsmcqtaz13\Plant\Ref\Symbols` | **降级为非阻塞**（2026-07-26） | `test-file/backup-test/DWG-0202GP06-01_p/RefData~4~681.zip` 含完整参考库 616 个 `.sym`（含 `Flanged Nozzle.sym` 等实际引用项），本地提取即可用 |
| `0x0059` 语义门（controlled-diff / native-reader） | 开放，有两条可行路径 | `dlls/` 内 IDA 资产（`ugeom2d1.dll`、`radsrvitem.dll.asm` 等）支持 native-reader 路线；616 记录语料支持统计交叉验证兜底（grill Q3 授权启发式） |

## Watch

- `igSymbol2d` ↔ `JSite` 若无稳定实例级连接件，标签退化为启发式挂接
  （计划 §6 已定默认）。
- `trailing_double_3` 若无区分度，字高维持 2.5mm 回退，只做旋转。
- 备份库 `.sym` 的许可边界以 `test-file/symbols/PROVENANCE.md` 为准，
  不全量入库。
