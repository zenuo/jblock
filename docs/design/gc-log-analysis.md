# GC 日志分析 — 产品设计

> 状态：**已实现**（2026-11-13）。功能拆分见 `feature_list.json` 中 `feat-068` – `feat-082`（均为 done，证据见各条 evidence）。实现中的调整：对比视图改为分面小图（§3.5）；GC02 / GC06 / GC11 的判定细化（§4.2）；WASM 体积通过 `opt-level = "z"`（正则引擎保持 `"s"`）回到预算内，代价是 GC 解析约慢 60%（§5.6）。

## 1. 目标与边界

在现有 thread dump 分析器的基础上，让 jblock 能**在浏览器本地**解析 JVM GC 日志，给出 KPI、问题诊断与可交互图表；并支持剪贴板、多文件批量导入。CLI 与 HTML 导出同步覆盖。

已确认的产品决策：

| 决策点 | 结论 |
| --- | --- |
| 格式 / 收集器 | JDK 9+ 统一日志（`-Xlog:gc*`）：G1 / Parallel / Serial / ZGC / Shenandoah；JDK 8（`-XX:+PrintGCDetails`）：G1 / Parallel / CMS。ZGC / Shenandoah 第一版只做停顿与堆曲线 |
| 多文件语义 | **自动判断**：同一 JVM 的滚动分片拼成一条时间线；不同 JVM 进入对比视图；用户可手动覆盖 |
| 图表库 | **uPlot**（Canvas、约 50KB，承载数十万点，支持缩放、框选和多图联动） |
| 入口 | **统一入口自动识别**：沿用现有的选择文件 / 拖拽 / 粘贴入口，按内容识别 thread dump 或 GC log；混合导入时分组展示 |
| 分析范围 | 核心 KPI + 停顿/堆图、分布与速率、问题诊断 Findings、事件明细表 + 时间范围联动（全部纳入） |
| CLI / 导出 | 都覆盖：CLI 输出文本或 JSON，退出码语义不变；HTML 导出内嵌静态 SVG 图表 |

**不做（v1）**：OpenJ9 verbose:gc XML；safepoint / TTSP 日志（`-Xlog:safepoint`、`PrintGCApplicationStoppedTime`）；tenuring 分布；`.zip` 归档；阈值可配置 UI；服务端上传。`.gz` 解压列为可选的 `feat-082`。

## 2. 用户场景

1. **线上停顿排查**：拿到某台机器的 `gc.log`，想知道最长停顿多久、什么时候发生、什么原因触发，以及是否出现过 Full GC。
2. **疑似内存泄漏**：导入同一 JVM 的一串滚动分片 `gc.log.0 … gc.log.9`（JDK 8 是 `gc.log.N.current`），观察 GC 后老年代基线是否持续上涨。
3. **集群对比**：同一服务 4 个节点各一份日志，横向比较吞吐率、P99 停顿和 Full GC 次数，找出异常节点。
4. **快速粘贴**：从终端或工单里复制一段 GC 日志，直接按 Ctrl+V 看结果。
5. **CI / 脚本**：`jblock gc.log -j` 拿到 JSON，或靠退出码在流水线里拦截有 Full GC 的压测结果。

## 3. 信息架构与交互

### 3.1 导入（统一入口）

- **首页文案**改为「拖入线程转储或 GC 日志」，按钮从 `Choose thread dump…` 改为 `Choose files…`，`Paste dump` 改为 `Paste`。
- **入口**：
  - 文件选择（`multiple`）；
  - **文件夹选择**（`webkitdirectory`）；
  - 拖拽多个文件或整个文件夹（`DataTransferItem.webkitGetAsEntry()` 递归遍历）；
  - 剪贴板：按钮 / Ctrl+V / 粘贴框兜底，复用现有 `clipboardImport.ts`；
  - 结果页提供 **Add files**，可向当前会话**追加**文件，比如补上漏掉的分片。
- **逐文件识别**（Rust `detect_input_kind`，只嗅探前 64KB）：`thread-dump` / `gc-log` / `unknown`。
  - 识别为 `unknown` 的文件以警告 chip 显示（「3 个文件无法识别」，可展开看文件名），不阻断其他文件。只有全部都无法识别时才报错。
  - 文件夹导入时，跳过隐藏文件和明显的二进制文件（前 4KB 含 NUL 字节）。
- **Load sample** 改成分裂按钮：`Thread dump sample` / `GC log sample`。

### 3.2 结果页顶层

当导入内容同时包含两类时，顶部出现分段切换：`线程转储 · 3 | GC 日志 · 2`。只有一类时不显示。线程转储一侧保持现有行为不变。

### 3.3 GC 结果页布局

```
┌ ReportNav ┐ ┌──────────────────────────────────────────────────────────────┐
│ 概览      │ │ [JVM 组: node-a (gc.log.0–3 · G1 · 6h12m) | node-b | 对比]    │
│ 问题      │ │ 分组: (自动) 合并 独立     时间轴: (uptime) 墙钟   ⓘ 2 个文件有间隙 │
│ JVM 信息  │ │ [范围 10:03:10 – 10:07:55 ×]                                   │
│ 停顿      │ ├──────────────────────────────────────────────────────────────┤
│ 堆        │ │ 吞吐率 97.8% │ P99 182ms │ Max 1.42s │ 总停顿 4m02s │ Full GC 3 │…│
│ 分布      │ ├──────────────────────────────────────────────────────────────┤
│ 原因      │ │ Findings（与现有 Findings 卡片同风格，带「定位」按钮）            │
│ 速率      │ ├──────────────────────────────────────────────────────────────┤
│ Metaspace │ │ 停顿时间线（散点，按 Young/Mixed/Full/Remark… 着色，并发周期为底色带）│
│ 事件      │ │ 堆使用（GC 前/后锯齿 + 容量线 + GC 后老年代线，Full GC 竖线）       │
│ 解析诊断  │ │ 停顿直方图 ┃ GC 原因（次数 / 总停顿，横向条形）                  │
└───────────┘ │ 分配 / 晋升速率 MB/s ┃ Metaspace used / committed               │
              │ 事件明细表（虚拟滚动，可筛选、排序）                             │
              │ 解析诊断（未识别行数 + 前 20 行样本、文件间隙/重叠）              │
              └──────────────────────────────────────────────────────────────┘
```

**联动规则**

- 所有时间轴图表**共享 x 轴与十字光标**（uPlot `cursor.sync`）。
- 在任一时间轴图上**框选** → 设置全局时间范围 `range`：所有图同步缩放；KPI、直方图、原因条形图、事件表**按范围重算**，KPI 标题标注「（所选范围）」；顶部出现范围 chip，点 × 复位，双击图表也复位。
- Findings 始终基于**全量日志**计算，与范围无交集的卡片变淡。每条 Finding 带时间锚点，点「定位」后缩放到 锚点 ±10% 跨度。
- 点击停顿散点，或在事件表中选中一行 → 其余图表画竖线标记该事件，事件表滚动到该行。
- 时间轴模式：`uptime`（默认）/ `墙钟`（仅当所有事件都带 `time` / `utctime` 或 JDK 8 date stamp 时可选）。

### 3.4 多文件分组（自动判断 + 手动覆盖）

对每个 GC 文件提取**指纹**：收集器、JVM 版本 / 启动参数（如有）、`pid` decorator（如有）、首末 `GC(id)`、首末 uptime、首末墙钟、JDK 8 滚动标记（`GC log file created …` / `GC log file has reached the maximum size`）。

判定为**同一 JVM 的分片**（按以下顺序判断）：

1. 有 `pid` decorator 且 pid 相同；
2. JDK 8 滚动标记互相衔接；
3. 收集器相同，**且** `GC(id)` 首尾相接（下一片首 id ≈ 上一片末 id + 1，允许少量缺口），**且** uptime 单调不回退。

不满足以上任一条件 → 视为不同 JVM（包括同一节点重启后 uptime 从 0 开始的情况）。分片按 `GC(id)` / uptime 排序后拼接；有缺口（id 跳号）或重叠（相同 id 重复）时去重，并在「解析诊断」里提示。

**手动覆盖**：分段控件 `自动 / 合并 / 独立`。
- 「合并」：把所有文件当作同一 JVM，按时间排序拼接；
- 「独立」：每个文件各成一组。

v1 不提供逐文件拖拽调整分组。

### 3.5 对比视图（≥2 个 JVM 组时出现）

入口是 JVM 分组栏末尾的「对比 N 个 JVM」。

- **KPI 对比表**：行是 JVM 组，列是 KPI。每列最差的值加粗并带「最差」标签（不只靠颜色）；框选范围后按范围重算。
- **GC 原因对比表**：原因 × JVM，单元格为总停顿和次数。
- **分面小图（small multiples）**：每个 JVM 一组停顿图 + 堆图，沿用单 JVM 页面的配色（颜色表示停顿类型，不表示 JVM）。所有小图共享 x 轴范围和十字光标，任一图上框选都会同步到全部。x 轴按 uptime 对齐，所有组都有墙钟时可切到墙钟。
  - 实现时由「叠加散点」调整为分面小图：散点图的分类色只有前 3 个槽位满足两两可辨（dataviz all-pairs 校验），叠加也会让颜色在同一页面上同时代表「停顿类型」和「JVM」两种含义。
- 最多同时对比 **8 组**，超出时显示勾选列表，由用户选择。

## 4. 分析内容

### 4.1 KPI

| KPI | 定义 |
| --- | --- |
| 吞吐率 | `1 − Σ停顿 / 日志跨度` |
| 停顿统计 | 次数、总时长、P50 / P90 / P95 / P99 / Max、平均停顿间隔 |
| 按类别计数 | Young / Mixed / Full / Remark / Cleanup / Concurrent cycles（ZGC / Shenandoah 为各 Pause 子类） |
| 分配速率 | 平均与峰值 MB/s：`本次 young_before − 上次 young_after`，除以间隔；没有分代数据时退化为 `heap_before − 上次 heap_after` |
| 晋升速率 | 平均 MB/s，仅在 young GC 且有 old 数据时计算 |
| 堆 | 最大容量；GC 后使用的 P50 / Max；GC 后占比峰值 |
| Metaspace | GC 后使用峰值 |
| 并发 GC 占比 | 并发周期总时长 / 跨度，适用于 G1 / CMS / ZGC / Shenandoah |

### 4.2 Findings（沿用 Findings 风格：severity + 标题 + 证据 + 建议 + 时间锚点）

**定级原则**：参照 JDK Mission Control（JMC）的 GC 自动分析规则。JMC 只有 OK / INFO / WARNING 三级，从不给「严重」，单次异常大多只是 INFO。jblock 保留 critical 一级，但**只用于有明确依据、意味着应用实际停摆或即将 OOM 的情况**：
- 连续 Full GC 且回收后老年代剩余可用空间不到 2%（接近 JVM 自己的 GC overhead limit）；
- GC 后可用空间低于 2%；
- G1 evacuation failure；
- CMS 的 promotion failed / concurrent mode failure。

其余规则对齐 JMC，没有外部依据的阈值标注为「自定」。JMC 的分数是连续映射（score 25 = INFO，75 = WARNING），这里取它在这两个点上对应的阈值。

| # | 规则 | 默认阈值 → severity | 依据 | 建议 |
| --- | --- | --- | --- | --- |
| GC01 | Full GC 发生（不含 System.gc 触发的） | G1 / CMS / Shenandoah 任意一次 → warning；Parallel / Serial 的 Full GC 属于正常老年代回收，本规则不适用 | JMC `FullGcRule`：G1 / CMS 出现 Full GC 即 WARNING，其他收集器不适用；Oracle G1 调优指南：Full GC「通常非常耗时」 | 列出原因分布；扩堆、调 IHOP、排查大对象 |
| GC02 | 连续 Full GC | 连续 ≥3 次 Full GC 且中间没有 young GC → warning；其中每次回收后老年代（无分代数据时用整堆）剩余可用空间都 < 容量的 2%，或回收量 < 2% → critical | GCeasy「Consecutive Full GC」模式；JVM `GCHeapFreeLimit=2`：Parallel 在 Full GC 后按老年代 / Eden 的剩余可用空间与容量之比判断，低于该值并叠加 `GCTimeLimit` 时抛 OOM；连续次数 3 为自定 | 区分内存不足（流量下降后自行恢复）与泄漏（一直持续到重启） |
| GC03 | 显式 `System.gc()` | ≥1 次 → info；占全部 GC 的比例 ≥ 50% → warning；约 1h 周期时提示 RMI DGC | JMC `SystemGcRule`（比例 0.5 → WARNING） | `-XX:+DisableExplicitGC` 或 `ExplicitGCInvokesConcurrent`；`sun.rmi.dgc.*.gcInterval` |
| GC04 | Evacuation failure / To-space exhausted（G1） | ≥1 次 → critical | Oracle G1 调优指南：Full GC 之前通常先出现 `(Evacuation Failure)` | 扩堆、`G1ReservePercent`、降低 IHOP |
| GC05 | Promotion failed / Concurrent mode failure（CMS） | ≥1 次 → critical | Oracle CMS 调优指南：concurrent mode failure 时停止所有应用线程完成回收 | `CMSInitiatingOccupancyFraction` 等；建议迁移 G1 |
| GC06 | 频繁 Humongous 分配（G1） | 原因为 Humongous 的 GC 占比 > 10%，或 humongous regions 峰值超过 old regions 的 50%（只计老年代 ≥ 堆容量 10% 的事件，排除启动初期的极小老年代）→ warning | Oracle G1 调优指南：Humongous regions 相对 old regions 偏高时应减少大对象 / 增大 `G1HeapRegionSize`；具体比例为自定 | `G1HeapRegionSize`，减少大数组 / 大缓冲 |
| GC07 | 长停顿 | 单次超过停顿目标 → info；P99 超过目标，或最长停顿 > 1s → warning；不设 critical。停顿目标取日志中的 `MaxGCPauseMillis`，否则取收集器默认值（G1 为 200ms）；没有目标的收集器（Parallel / Serial）只看 1s | JMC `LongGcPauseRule`（最长停顿 1s 仅 INFO）；JDK 21 默认 `MaxGCPauseMillis=200`（G1） | 按停顿类别给出建议 |
| GC08 | GC 时间占比过高 / GC 风暴 | 任一 60s 滑动窗口内停顿占比 > 5% → info，> 10% → warning，≥ 98% → critical（锚点为该窗口）；全局 GC 时间占比超过收集器自身目标 → info | JMC `GcPauseRatioRule`（60s 窗口，5% / 10%）；JVM `GCTimeLimit=98`；收集器目标 `1/(1+GCTimeRatio)`：G1 约 7.7%，Parallel 为 1% | 扩堆，排查分配热点 |
| GC09 | 疑似内存泄漏 | 见下方说明 | JMC `IncreasingLiveSetRule` | 用 heap dump 定位，建议做 jmap 对比 |
| GC10 | GC 后堆接近上限 | GC 后占比 > 85% → warning；≥ 98%（可用空间 < 2%）→ critical | 98% 对应 JVM `GCHeapFreeLimit=2`；85% 为自定 | OOM 风险，扩堆或排查泄漏 |
| GC11 | Metaspace 持续增长 | 预热期后 Metaspace（GC 后）的最小二乘斜率 ≥ 0.75 MiB/s → warning；预热期后原因为 `Metadata GC Threshold` 的回收 ≥ 3 次 → info（启动阶段类加载导致的不计入） | JMC `IncreasingMetaspaceLiveSetRule`（0.75 MiB/s → WARNING） | ClassLoader 泄漏、`MaxMetaspaceSize` |
| GC12 | GC 并行度异常 | `real > (user+sys) × 1.1` 且 real > 100ms → warning（IO / swap / 宿主争用）；`sys > user` → info | JMC `GcInvertedParallelismRule`（(user+sys)/real < 100% 即 WARNING）；1.1 倍与 100ms 是自定的噪声过滤 | 检查 swap、透明大页、容器 CPU 限额 |
| GC13 | Young GC 过于频繁 | 平均间隔 < 1s → info | 自定 | 增大新生代 / `G1NewSizePercent` |
| GC14 | ZGC / Shenandoah 退化 | ZGC Allocation Stall 每分钟 ≥10 次 → info，≥100 次 → warning；Shenandoah Degenerated GC ≥1 次 → warning（与 GC01 同级，属于 STW 回退）；Pacing → info | JMC `ZGCAllocationStallRule`（每分钟 10 / 100）；Shenandoah 部分为自定 | 扩堆、增加并发 GC 线程 |
| — | 无问题 | — | — | 复用现有的绿色 OK 空状态（feat-055） |

**GC09 疑似内存泄漏的判定**（以 JMC `IncreasingLiveSetRule` 为基础）：

1. **数据**：GC 后老年代占用；没有分代数据时用 GC 后整堆占用。
2. **排除预热期**：JMC 用「已加载 90% 的类」作为预热结束点，但 GC 日志里没有类加载信息，这里改为排除 uptime < 5 分钟以及日志前 10% 时长内的点（自定）。
3. **数据量**：预热期后至少 5 次 GC（JMC 要求 young GC 超过 4 次），且跨度 ≥ 30 分钟（自定）。不满足时不判断。
4. **斜率**：最小二乘拟合，按预热期后第一个点的占用归一化，得到「每秒上涨百分之几」。
5. **噪声过滤**：R² ≥ 0.7，确认是稳定上涨而不是业务波动（自定，JMC 没有这一条）。
6. **定级**：
   - 归一化斜率 ≥ 1%/s → warning（JMC 的 WARNING 点）；
   - 按当前斜率推算，GC 后占用将在 24 小时内达到堆上限 → warning（自定，用来覆盖几小时级日志里的慢泄漏，JMC 的 1%/s 是针对几分钟的 JFR 录制定的）；
   - 其余满足条件的上涨 → info；
   - 日志中**没有任何老年代 / Mixed / Full GC** 时，最多给 info（JMC 同样处理：没有老年代回收就无法区分泄漏和尚未回收）。
   - 即将占满堆的紧急情况由 GC10 给出 critical，本规则不重复定级。

阈值集中在 Rust 的 `GcThresholds::default()` 里，v1 不暴露 UI。所有阈值及其依据在本文档和 Pattern legend 中说明。

**参考资料**

- [JMC GC 规则源码（openjdk/jmc）](https://github.com/openjdk/jmc/tree/master/core/org.openjdk.jmc.flightrecorder.rules.jdk/src/main/java/org/openjdk/jmc/flightrecorder/rules/jdk/memory)
- [Oracle — G1 GC 调优](https://docs.oracle.com/en/java/javase/21/gctuning/garbage-first-garbage-collector-tuning.html)
- [Oracle — CMS（Java 8）](https://docs.oracle.com/javase/8/docs/technotes/guides/vm/gctuning/cms.html)
- [GCeasy — Repeated Full GCs](https://blog.gceasy.io/repeated-full-gc/)
- [GCeasy — 泄漏与连续 Full GC 的区别](https://blog.gceasy.io/interesting-garbage-collection-patterns/)
- JVM 默认参数取自 Temurin 21 的 `java -XX:+UseG1GC|UseParallelGC -XX:+PrintFlagsFinal -version`：`MaxGCPauseMillis=200`（G1）、`GCTimeRatio=12`（G1）/ `99`（Parallel）、`GCTimeLimit=98`、`GCHeapFreeLimit=2`

### 4.3 JVM 信息面板

数据来源：JDK 17+ 的 `gc,init` 行（Version、CPUs、Memory、Heap Min / Initial / Max、Region Size、Parallel / Concurrent Workers…），以及 JDK 8 头部的 `CommandLine flags:` 和 `Memory:`。提取不到时整块隐藏。

### 4.4 解析诊断

展示每个文件识别出的格式、收集器、事件数、未识别的行数和前 20 行样本、时间缺口与重叠，方便用户反馈不支持的格式。

## 5. 技术设计

### 5.1 Rust 模块（纯 Rust，可 `cargo test`）

```
src/
├── input_kind.rs     # detect_input_kind(&str) -> InputKind { ThreadDump, GcLog, Unknown }
└── gc/
    ├── mod.rs        # pub fn analyze_gc(inputs: &[GcInput], opts) -> GcReport
    ├── model.rs      # GcEvent / GcKind / Collector / GcFileInfo / GcReport
    ├── unified.rs    # JDK 9+ -Xlog 解析：decorators (time/utctime/uptime/uptimemillis/
    │                 #   timemillis/level/tags/pid/tid 任意组合与顺序)，按 GC(id) 聚合多行
    ├── legacy.rs     # JDK 8 PrintGCDetails：多行事件、CMS 并发行交错、date/time stamp 可选
    ├── stitch.rs     # 文件指纹 + 分组 + 拼接 + 去重 / 缺口诊断
    ├── stats.rs      # KPI / 百分位 / 速率 / 原因分布；支持 range: Option<(f64, f64)>
    └── findings.rs   # GC01–GC14 规则 + GcThresholds
```

- 解析按**事件聚合**：统一日志以 `GC(id)` 为键，汇总 start / heap / metaspace / cpu / 结束行；JDK 8 以 `[GC` / `[Full GC` 开头的块为单位（处理跨行和被并发行打断的情况）。
- `GcEvent` 的字段：`gc_id, uptime_ms, wall_ms?, collector, kind, is_pause, cause, duration_ms, heap_{before,after,cap}_kb, young_*, old_*, humongous_*, metaspace_*, cpu_{user,sys,real}_ms, flags`（`to_space_exhausted / evac_failure / promotion_failed / cmf / alloc_stall / degenerated`）。
- **图表数据用列式结构**（struct-of-arrays，`Vec<f64>`，缺失值用 `NaN`），方便直接交给 uPlot，也省掉对象开销。
- 解析不 panic：未识别的行计入 `unparsed`，不中断解析。

### 5.2 WASM API（`src/lib.rs`）

```rust
#[wasm_bindgen] pub fn detectInputKind(text: &str) -> String;

#[wasm_bindgen] pub struct GcSession { /* 持有全部事件与分组 */ }
#[wasm_bindgen] impl GcSession {
    #[wasm_bindgen(constructor)] pub fn new(files: JsValue /* [{name,text}] */, grouping: &str) -> Result<GcSession, JsValue>;
    pub fn report(&self) -> JsValue;                          // 分组、JVM 信息、全量 KPI、Findings、诊断（不含逐事件数据）
    pub fn series(&self, group: usize) -> JsValue;            // 列式图表数据（Float64Array）
    pub fn stats(&self, group: usize, t0: f64, t1: f64) -> JsValue;   // 范围 KPI / 直方图 / 原因
    pub fn events(&self, group: usize, query: JsValue) -> JsValue;    // 分页 + 筛选 + 排序
    pub fn regroup(&mut self, grouping: &str);                // 自动 / 合并 / 独立
}
```

- `GcSession` **常驻 worker**（扩展 `analyzeMessages.ts` 协议：`gcOpen` / `gcStats` / `gcEvents` / `gcRegroup` / `gcClose`）。几十万条事件留在 wasm 内存里，主线程只拿列式数组和分页结果，避免 structured clone 大对象。
- 新一轮导入或关闭页面时调用 `free()`。
- **范围统计只在 Rust 里实现一次**：Web 的范围重算和 CLI 共用 `stats.rs`。框选结束时（mouseup）才请求，不在拖动过程中逐帧请求。
- `web/src/gcTypes.ts` 与 `gc/model.rs` 保持同步（serde kebab-case，与 `types.ts` 约定一致）。

### 5.3 前端

| 文件 | 职责 |
| --- | --- |
| `importFiles.ts` | 文件、文件夹、拖拽遍历、二进制跳过；调用 `detectInputKind` 分流 |
| `App.tsx` | 顶层 `线程转储 / GC 日志` 切换；GC 会话生命周期 |
| `GcResults.tsx` | GC 结果页骨架，复用 `ReportNav` |
| `gc/charts/*.tsx` | uPlot 封装：`PauseTimeline`、`HeapChart`、`PauseHistogram`、`CauseBars`、`RateChart`、`MetaspaceChart`；统一 sync key 与框选回调 |
| `gc/GcEventTable.tsx` | 固定行高的虚拟滚动（不引入依赖），按页向 worker 拉数据 |
| `gc/GcCompare.tsx` | 对比视图 |
| `gcExport.ts` | HTML 导出中的 SVG 图表渲染（见 5.5） |

- 图表配色、标记规格和交互，在实现时以 `dataviz` skill 为准（分类色 ≤8、亮暗主题一致）。
- 所有新增文案写进 8 个 locale（`i18n/locales/*.ts`）。

### 5.4 CLI

- 自动识别输入类型；`--kind auto|thread|gc` 可强制指定。
- 多个文件里有 GC 日志时走 `analyze_gc`（同样自动分组）；thread dump 与 GC 日志混合输入时，分两个大节输出。
- 文本输出：头部（收集器、JVM、文件、跨度）→ KPI → Findings → 原因 Top N → 最长停顿 Top N → GC 后堆的 Unicode sparkline。
- `-s/--section` 新增 `summary,findings,causes,pauses,events`；`-j` 输出 `GcReport` JSON，`--events` 时附带逐事件数组。
- 退出码不变：`0` 干净或仅 info，`1` 有 warning / critical，`2` 用法错误，`3` 全部无法识别。

### 5.5 HTML 导出

- 结构与现有导出一致（复用 `?inline` CSS、ReportNav 锚点）。
- 图表另写一个轻量 **SVG 渲染器**：时间序列做 min/max 分桶降采样（每张图 ≤ 2000 个点），保证报告体积可控、离线可读。导出内容为当前 JVM 组（或对比视图）的全量范围。如果当前有框选范围，额外附一节「所选范围」的 KPI。

### 5.6 性能预算

| 场景 | 目标 |
| --- | --- |
| 50MB 统一日志（约 15 万次 GC）解析 + 分组 + 统计 | worker 内 < 2s（开发机） |
| 图表首屏 | < 300ms；20 万点框选缩放保持流畅 |
| 范围统计往返 | < 50ms |
| WASM 体积增量 | 记录前后数值；目标 < +150KB（gzip 前）。实测 1108 KiB → 1239 KiB（+131 KiB）|
| 单次导入总量 | 500MB 以上给出提示，但不拒绝 |

## 6. 测试与 Fixtures

- **真实采集**：新增 `scripts/capture-gc-logs.sh` 和 `GcWorkload.java`（分配抖动、humongous、`System.gc()`、泄漏模式、短时长、可复现）。
  - 用 JDK 21（本机 mise 已有）采集 G1 / Parallel / Serial / ZGC（分代与非分代）/ Shenandoah；
  - 通过 mise 安装 temurin-8 / 11 / 17，采集 JDK 8 的 G1 / Parallel / CMS，以及 decorator 变体（`time,uptime,level,tags`、`uptimemillis`、`utctime`、`pid`）；
  - 滚动分片用 `filecount=…,filesize=…` / `-XX:+UseGCLogFileRotation` 真实生成。
- Fixtures 放在 `tests/fixtures/gc/`，单个文件裁剪到 < 200KB。格式差异记录在 `tests/fixtures/gc/FORMAT_DIFFS.md`。
- Rust 单元测试：每种格式 × 收集器的事件数和关键字段、分组与拼接、每条 Finding 的正反例、范围统计。
- 每个 feature 在 `scripts/e2e-features.mjs` 里增加对应的 `FEATURE_CHECKS` 条目；Web 交互按现有惯例做浏览器验证，并把证据记入 `progress.md`。

## 7. 交付拆分（`feature_list.json`）

| id | 名称 | 依赖 |
| --- | --- | --- |
| feat-068 | GC 事件模型 + JDK 9+ 统一日志解析（G1 / Parallel / Serial）+ 真实 fixtures 采集脚本 | — |
| feat-069 | JDK 8 PrintGCDetails 解析（G1 / Parallel / CMS） | 068 |
| feat-070 | ZGC / Shenandoah 统一日志解析（停顿 + 堆） | 068 |
| feat-071 | GC 统计引擎（KPI / 百分位 / 速率 / 原因 / 范围统计） | 068 |
| feat-072 | GC Findings 规则 GC01–GC14 | 071 |
| feat-073 | 多文件指纹、分组与拼接（自动 / 合并 / 独立） | 068 |
| feat-074 | 输入类型识别 + `GcSession` WASM/worker + 统一导入（文件夹 / 拖拽目录 / 追加 / 剪贴板）+ 顶层切换 | 071, 073 |
| feat-075 | GC 结果页：KPI、JVM 信息、Findings、停顿时间线 + 堆图（uPlot）、联动与框选范围 | 072, 074 |
| feat-076 | 分布与速率图：直方图、原因条形、分配 / 晋升速率、Metaspace | 075 |
| feat-077 | GC 事件明细表（虚拟滚动、筛选排序、与图表联动）+ 解析诊断面板 | 075 |
| feat-078 | 多 JVM 对比视图 | 075 |
| feat-079 | GC 报告 HTML 导出（SVG 图表） | 076 |
| feat-080 | CLI 支持 GC 日志 | 072, 073 |
| feat-081 | GC 样例、首页文案、Help / Pattern legend、README | 075 |
| feat-082 | （可选）`.gz` 解压导入（Web `DecompressionStream` / CLI） | 074 |

建议实现顺序：068 → 071 → 073 → 074 → 075（先打通 G1 统一日志的端到端）→ 072 → 076 / 077 → 069 / 070 → 080 → 078 → 079 → 081 → 082。

## 8. 待评审的开放问题

1. ~~Findings 阈值~~：已按 JMC / JVM 默认参数 / Oracle 调优指南 / GCeasy 修订（2026-11-13），见第 4.2 节。标注「自定」的阈值仍可在实现阶段用真实日志校准。
2. 首页按钮改名（`Choose files…` / `Paste`）会影响已有用户的认知，是否接受？
3. 对比视图上限 8 组是否够用？
