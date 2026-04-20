# SPPID 完全解析路线图

> 日期：2026-04-21
> 范围：`pid-parse` 项目内 SPPID/SmartPlant P&ID 文件数据解析能力升级
> 目标：从“部分结构化解析”推进到“尽可能完整、可验证、可维护的全文件解析”

## 1. 背景与目标

当前项目已经具备 SPPID 文件的基础解析框架，能够读取 CFB 容器、识别部分顶层流、提取元数据、建立对象图、交叉引用图与近似布局模型。但从代码现状看，解析能力仍处于“部分完成”阶段：

- 已知顶层流虽然大多可识别，但仍存在 `unknown_streams`
- `inspect` 模块仍保留“unidentified top-level streams”视角，说明覆盖面尚未闭环
- 多个 parser 仍依赖 `probe`、`raw`、`heuristic`、`audit` 风格输出，而不是稳定结构模型
- `PSM` 系列表、`Sheet*`、`Relationship`、`Dynamic Attributes` 等关键二进制结构还未完全统一到一个严格的规范化语义层

本路线图的目标不是只“多解析几个字段”，而是把项目升级为一个可以系统回答以下问题的解析器：

1. 一个 SPPID 文件包含哪些流、存储、记录结构？
2. 每类字节数据被解释到了什么程度？
3. 每个业务对象、关系、图元、符号、端点分别来自哪些底层记录？
4. 哪些区域仍未知、为何未知、是否稳定复现？

最终交付应满足：结构覆盖清晰、模型统一、验证可追溯、后续 writer/diff/inspect 能直接消费同一套可靠数据层。

## 2. 当前状态评估

从现有模型与 parser 能力看，项目已经具备如下基础：

- 容器与原始流层：
  - CFB 树与流列表已可读取
  - 顶层已识别流包括 `PSMroots`、`PSMclustertable`、`PSMsegmenttable`、`DocVersion2`、`DocVersion3`、`AppObject`、`JTaggedTxtStgList`
  - 存储前缀已识别 `Sheet*`、`TaggedTxtData`、`JSite*`
- 结构化模型层：
  - `PidDocument` 已能承载 summary、drawing/general XML、JSite、cluster、dynamic attributes、sheet streams、PSM tables、DocVersion2、object inventory、object graph、cross reference、layout
- 推导层：
  - 已能从 dynamic attrs 构建 `object_graph`
  - 已能建立 `cross_reference`
  - 已能生成面向可视化的 `layout`

但仍存在明显缺口：

- 顶层识别与“完全解码”不是一回事；当前更多是“入口识别 + 部分提取”
- `DocVersion3` 尚未形成成熟、稳定、可依赖的结构化模型
- `PSMclustertable` / `PSMsegmenttable` 仍未完全解释 header/flags/索引关系
- `Sheet*` 与 `relationship`、`endpoint`、`object_graph` 之间仍有割裂
- 不同 parser 的“已消费字节 vs 未解释字节”没有统一验证框架

因此，下一阶段不应直接做零散 parser 修补，而应建立一个覆盖清单驱动的持续推进机制。

## 3. 总体策略

建议采用“先盘点、再收口、后统一、再验证”的四段式策略：

1. 先建立 **覆盖清单（coverage inventory）**
   - 明确每个顶层流、关键子流、关键记录的解析状态
   - 把“已识别”和“已完全解析”区分开
2. 再补齐 **关键顶层结构化 parser**
   - 优先处理能显著降低未知区域且与全局模型强相关的流
3. 再统一 **规范化语义层**
   - 把 object / relationship / sheet / psm / symbol / cluster 的来源统一入图
4. 最后建设 **字节级验证体系**
   - 让每个 parser 都能说明：解释了哪些字节，剩余哪些字节

这个顺序的核心价值在于：避免陷入“看见哪块像哪块就解哪块”的局部优化，确保每一轮工作都能量化提升全局覆盖率。

## 4. 分阶段实施路线

### Phase 1：建立 SPPID 解析覆盖清单

#### 目标

构建一个统一的 coverage report，回答“当前项目对 SPPID 文件解析到了哪里”。

#### 具体任务

- 新增 coverage inventory/report 生成逻辑
- 针对每类顶层流输出以下状态：
  - `fully_decoded`
  - `partially_decoded`
  - `identified_only`
  - `unknown`
- 将以下模块纳入清单：
  - summary
  - tagged text
  - jsite
  - dynamic attrs
  - sheet streams
  - psm tables
  - doc version 2 / 3
  - app object
- 对真实样本批量输出覆盖报告，统计：
  - 未覆盖顶层流
  - 部分覆盖流
  - 未解释字节集中区

#### 建议修改点

- `src/inspect/`：新增 coverage 报告生成入口
- `src/model.rs`：新增解析状态枚举或覆盖统计模型
- 视情况新增 `src/inspect/coverage.rs`

#### 验收标准

- 任意样本文件都能输出一份覆盖报告
- 报告可明确区分“识别”与“完全解析”
- 报告结果可作为后续阶段优先级依据

### Phase 2：补齐关键顶层流的结构化解析

#### 目标

把目前仍停留在 raw/probe/partial 状态的关键流升级为稳定结构模型。

#### 优先顺序

1. `DocVersion3`
2. `PSMclustertable`
3. `PSMsegmenttable`
4. `JTaggedTxtStgList`
5. `Sheet*` 深层结构

#### 执行原则

每个流统一提供三层表达：

- `raw`：原始字节保留
- `decoded`：已确认语义的结构化字段
- `audit/probe`：尚不能完全命名但可稳定提取的字段

#### 重点说明

##### 2.1 `DocVersion3`

- 明确 header、record 布局、计数字段、版本差异
- 与 `DocVersion2` 做字段对照
- 增加 `DocVersion3Decoded` 类模型，而不是仅保留原始或半解释结构

##### 2.2 `PSMclustertable`

- 结构化提取 cluster id、索引、flags、类型、声明关系
- 建立“声明的 cluster”与 `doc.clusters` 实际扫描结果之间的映射
- 对缺失项与冗余项做显式报告

##### 2.3 `PSMsegmenttable`

- 结构化提取 segment record
- 与 `layout.segments`、relationship、sheet endpoint 的来源链建立映射
- 明确 segment 是几何实体、连接实体还是引用实体

#### 验收标准

- 每个目标流都有单独 parser + model + tests
- 结构字段具备稳定命名，不再主要依赖“raw preview”
- 对真实样本可输出可比对的结构化摘要

### Phase 3：统一规范化语义图层

#### 目标

把当前分散在多个 parser/派生模块中的对象、关系、端点、符号、cluster、sheet 信息统一到一套规范化语义图层。

#### 具体任务

- 统一以下来源：
  - `Dynamic Attributes Metadata`
  - `Unclustered Dynamic Attributes`
  - `Sheet*`
  - `PSMroots`
  - `PSMclustertable`
  - `JSite*`
- 扩展或重构规范化模型，建议至少包含：
  - object
  - relationship
  - endpoint
  - symbol reference
  - cluster reference
  - source provenance

#### 核心要求

每个规范化实体都应携带 provenance：

- 来源 stream path
- record id / field_x / cluster index
- 原始 drawing id / model id / guid
- 解析来源层级（raw / decoded / inferred）

#### 结果

后续 `inspect`、`report`、`import_view`、`layout` 都改为消费这套统一图层，而不是各自再做临时拼装。

### Phase 4：建设字节级验证与回归体系

#### 目标

避免“看起来解析对了，但实际上只覆盖了部分字节”的假象。

#### 具体任务

- 为 parser 增加 consumed-range / leftover-range 报告
- 为每种结构建立：
  - 单元测试
  - fixture 测试
  - 真实文件 golden 测试
  - 跨流一致性检查
- 增加关键一致性断言：
  - record count 与实际遍历数量一致
  - 交叉引用无不可解释悬空
  - stream size 与已消费区间一致
  - 未解析区间明确可见

#### 验收标准

- 每个关键 parser 都能回答“解释了哪些字节”
- 真实样本中的未知区能被定位与量化
- 回归测试能阻止解析覆盖率倒退

### Phase 5：达成“接近完整解析”的验收门槛

#### 满足以下条件时可视为阶段性完成

- 所有顶层已知流都有明确 parser 和结构模型
- `unknown_streams` 仅剩少量样本特异内容
- `inspect` 输出以结构化信息为主，不再依赖 raw/probe 描述
- `object_graph`、`cross_reference`、`layout` 全部基于统一规范化图层
- 多个代表性真实样本上的未解释字节比例显著下降并可量化

## 5. 建议的近期执行顺序

建议接下来严格按如下顺序推进：

1. 实现 coverage inventory / coverage report
2. 完成 `DocVersion3` 结构化解析
3. 完成 `PSMclustertable` 结构化解析
4. 完成 `PSMsegmenttable` 结构化解析
5. 统一 dynamic attrs + sheet + relationship 规范化语义图
6. 引入 consumed-bytes / leftover-bytes 验证框架

不建议直接从某个局部 parser 开始“深挖到底”，否则很容易再次产生覆盖不透明的问题。

## 6. 第一阶段的可执行实施计划

### 6.1 目标

在不大规模改动现有 parser 的前提下，先把“项目当前解析覆盖面”显式化。

### 6.2 任务拆分

#### W1：定义覆盖状态模型

- 在 `src/model.rs` 或独立模块中新增：
  - `ParseCoverageStatus`
  - `CoverageEntry`
  - `CoverageReport`
- 字段建议包括：
  - stream/storage 名称
  - 类型（top-level stream / storage / nested stream）
  - 当前状态
  - 对应 parser 名称
  - 输出到 `PidDocument` 的目标字段
  - 备注（partial/raw/probe/inferred）

#### W2：实现 coverage report 生成器

- 新增 `src/inspect/coverage.rs`
- 输入 `&PidDocument`
- 输出统一 `CoverageReport`
- 与现有 `unidentified_top_level_streams` 逻辑打通，但不再只返回“未知”，而是返回全量状态表

#### W3：补充 inspect 展示层

- 在 `inspect` 报告里增加 coverage section
- 优先支持：
  - 顶层流覆盖摘要
  - 部分解析项列表
  - 未知项列表

#### W4：补测

- 使用构造文档测试 coverage 分类
- 对真实样本做至少一条 golden/assertion 测试
- 校验已有已知流均能出现在报告中

### 6.3 第一阶段验收标准

- 可以从任意 `PidDocument` 生成 coverage report
- `KNOWN_TOP_LEVEL_STREAM_NAMES` / storage prefixes 会映射到明确状态，而不是仅做布尔过滤
- 输出中能看出下一步最值得投入的目标流

## 7. 风险与应对

### 风险 1：逆向范围持续膨胀

表现：越解越多，始终没有“完成”的感觉。

应对：

- 强制采用 coverage 驱动
- 每阶段必须有量化验收项
- 先建立状态地图，再决定深挖目标

### 风险 2：模型增长过快导致 `PidDocument` 膨胀

表现：字段越来越多，且原始结构、推导结构、审计结构混杂。

应对：

- 明确区分 raw / decoded / inferred / normalized
- 新模型优先按模块分组，不直接把所有细节塞平到顶层

### 风险 3：真实样本差异过大

表现：某些 parser 对个别样本成立，对其他样本失效。

应对：

- 每次 parser 升级必须用多样本回归
- 不确定语义先放 audit/probe 层，不强行命名
- 未解释区显式保留，不做危险猜测

## 8. 结论

这个项目距离“完全解析 SPPID 文件数据”已经有了良好骨架，但当前最缺的不是再多写一个 parser，而是把整体覆盖面、结构边界、验证边界先显式化。

因此，推荐的下一步不是直接进入某个复杂流的逆向，而是优先实现 **Phase 1：coverage inventory/report**。这一步完成后，后续每一轮解析工作都将变得可衡量、可排序、可验证，也更适合作为长期稳定迭代的工程主线。
