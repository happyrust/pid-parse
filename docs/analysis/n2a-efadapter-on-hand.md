# N2a：EFAdapter.dll 已在手头（2026-10-12）

本页只记录文件从哪来、拷到哪、SHA-256，以及伴随文件为什么没一起拷。DLL 字节未改，`src` 未改。

## 文件

| 项 | 值 |
| --- | --- |
| 来源 | `D:\work\cad\bin\efadapter.dll` |
| 目标 | `D:\work\plant-code\cad\pid-parse\dlls\EFAdapter.dll` |
| 大小 | 1 519 616 字节 |
| 文件时间 | 2017-08-30 11:11:26 |
| SHA-256 | `6C77CC79A3CA92256F3C168C6183B03BAA67CF4545D9330E7E854C3F8C487891` |

来源与目标的大小、文件时间、SHA-256 一致（本会话对两边分别做了 `Get-FileHash -Algorithm SHA256`）。

## 伴随文件

计划要一起拷的两类都没有进 `dlls\`，因为来源侧找不到：

- `D:\work\cad\bin`（与 `efadapter.dll` 同目录）没有 `EFCommon*`。
- `D:\work\cad` 深度 3 没有 `EFCommon*`。
- `D:\work\cad` 深度 3 没有 `*.cmf`（同目录或相邻的 Schema Component 都没有）。

因此本次拷入 `dlls\` 的只有 `EFAdapter.dll`。没有覆盖 `dlls\` 里已有的其他文件。
