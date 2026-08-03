# jblock

> 基于 React + Rust/WebAssembly 的单页面 Java 线程转储（thread dump）分析工具。

`jblock` 在浏览器本地解析并分析 Java 线程转储：选择本地 dump 文件后，由 Rust 编译成的
WASM 库完成解析与问题模式识别，结果直接在浏览器渲染，并可导出为 HTML。所有解析都在
本地完成，dump 内容不会上传到任何服务器。

## 功能特性

- **本地文件选择 / 拖拽上传**：选择或直接拖拽本地线程转储文件（`.txt` / `.log` / `.tdump` 等）到页面。
- **多格式适配**：
  - `jstack` 工具输出（状态位于独立的 `java.lang.Thread.State:` 行）。
  - `ThreadMXBean#dumpAllThreads` / `ThreadInfo#toString()` 输出（状态在线程头行）。
  - 已用 Temurin **Java 8 / 11 / 17 / 21** 实机 dump 回归（见 `tests/fixtures/java-versions/`）。
- **Rust/WASM 解析**：核心解析器用 Rust 编写，编译为 WebAssembly，兼顾性能与安全。
- **分析结果**：
  - 线程状态分组计数（RUNNABLE / BLOCKED / WAITING / TIMED_WAITING / …）。
  - 锁阻塞问题模式识别：找出被阻塞线程、其等待的锁，以及持有该锁的线程。
  - **死锁环检测**：基于 wait-for 图检测线程间的循环等待并可视化。
  - 每个线程的持有锁、栈深度等信息。
- **结果渲染**：问题优先 Findings、按锁聚合的竞争视图、可筛选/排序线程表、栈帧展开、JVM 噪音过滤、同栈聚类；死锁/竞争边可跳转到对应线程。
- **问题模式**：死锁环、热锁竞争、**线程池耗尽**、**同步 I/O / RPC 热点簇**、**危险热锁持有者（owner 阻塞调用）**、**连接池借出阻塞** 等 Findings。
- **Java reproducer**：右上角 **Generate Java…** 打开模态框，生成可运行的死锁 / 锁竞争 / 线程池耗尽 / 同步 I/O 热点 / 危险热锁 / 连接池借出阻塞样例代码（不经过 WASM）；测试可用 JDK 实机 `jstack` 捕获 dump。
- **导出**：
  - HTML 报告：复用 web app 自身的 CSS（`?inline`）与结构，样式与页面一致。

## 技术栈

| 层 | 技术 |
| --- | --- |
| 解析核心 | Rust（`wasm-bindgen` + `serde` + `regex`），编译为 `wasm32-unknown-unknown` |
| 打包 | `wasm-pack`（`--target web`） |
| 前端 | React 18 + TypeScript + Vite |

## 目录结构

```
jblock/
├── Cargo.toml           # Rust WASM crate + optional `cli` feature / bin
├── src/
│   ├── lib.rs           # wasm-bindgen 绑定层（analyzeDump 导出）
│   ├── parser.rs        # 纯 Rust 解析/分析逻辑（含单元测试）
│   ├── cli/             # 主机 CLI（feat-056，`--features cli`）
│   └── bin/jblock.rs    # `jblock` 二进制入口
└── web/                 # React + Vite 前端
```

## 环境要求

- Rust 工具链（`rustc` / `cargo`）与 `wasm32-unknown-unknown` target
- [`wasm-pack`](https://rustwasm.github.io/wasm-pack/)
- Node.js 18+ 与 `pnpm`

一次性安装：

```bash
rustup target add wasm32-unknown-unknown
curl https://rustwasm.github.io/wasm-pack/installer/init.sh -sSf | sh
cd web && pnpm install
```

## 开发

```bash
cd web
pnpm run dev        # 先用 wasm-pack 构建 WASM（dev 模式），再启动 Vite
```

打开 http://localhost:5173/ ，点击 **Load sample**（含死锁 / 热锁竞争 / JVM 噪音线程的示例 dump）或 **Choose thread dump…** 选择本地 dump。

> 说明：WASM 不会随前端热更新自动重建。修改了 `src/*.rs` 后，需要重新运行
> `pnpm run wasm`（或重启 `pnpm run dev`）以重建 `web/src/wasm/`。

## 常用命令（在 `web/` 目录下）

| 命令 | 作用 |
| --- | --- |
| `pnpm run wasm` | release 模式构建 WASM 到 `web/src/wasm/` |
| `pnpm run wasm:dev` | dev 模式构建 WASM（更快，未优化） |
| `pnpm run dev` | 构建 WASM 并启动 Vite 开发服务器 |
| `pnpm run build` | 构建 WASM + 类型检查 + 生产打包到 `web/dist/` |
| `pnpm run preview` | 预览生产构建 |
| `pnpm run typecheck` | TypeScript 类型检查 |
| `pnpm run lint` | ESLint 检查 |

Rust 侧：

```bash
cargo test --features cli   # 解析单元测试 + CLI（feat-056）
cargo run --features cli --bin jblock -- --help
cargo run --features cli --bin jblock -- tests/fixtures/deadlock_real_jstack.txt
jstack <pid> | cargo run --features cli --bin jblock
```

## CLI（feat-056）

主机端二进制包装同一套 `parser::analyze` / `analyze_series`：

| 输入 | 示例 |
| --- | --- |
| 文件 | `jblock dump.txt` |
| 管道 | `jstack $PID \| jblock` |
| 剪贴板 | `jblock -c` / `jblock --clipboard` |
| 多文件（跨 dump） | `jblock t1.txt t2.txt` |

常用参数：`-j` / `--output json`、`-s/--section`、`--state`、`--severity`、`--hide-jvm`、`-v`、`-n/--limit`、`-q`、`--color`。

退出码：`0` 干净或仅 info；`1` 有 warning/critical/死锁；`2` 用法/读入错误；`3` 无法识别为 dump。

> CLI 依赖可选 feature `cli`（`clap`；剪贴板通过 `pbpaste` / `wl-paste` / `xclip` / `xsel`），默认不编进 WASM。

## 本地部署

jblock 是纯静态单页应用（HTML / JS / CSS + WASM），解析在浏览器本地完成，**无需后端**。把 `web/dist/` 用任意静态文件服务器托管即可。

### 从源码构建并预览

先完成上方「环境要求」中的一次性安装，然后：

```bash
pnpm -C web install
pnpm -C web run build          # 默认 VITE_BASE=/ ，产物在 web/dist/
pnpm -C web run preview        # 本地预览生产构建，默认 http://localhost:4173/
```

也可用其它静态服务器，例如：

```bash
npx --yes serve web/dist
# 或将 web/dist/ 挂到 Nginx / Caddy / 内网对象存储的站点根目录
```

### 子路径部署

若站点不在域名根路径（例如 `https://example.com/jblock/`），构建时设置 `VITE_BASE`（须以 `/` 开头和结尾）：

```bash
VITE_BASE=/jblock/ pnpm -C web run build
```

> **注意**：GitHub Actions 发布到 GitHub Pages 的产物使用 `VITE_BASE=/jblock/`。该包适合挂在 `/jblock/` 子路径；若你要挂在站点根路径（`/`），请本地按默认 `VITE_BASE=/` 重新构建，不要直接复用 Pages 产物，否则静态资源会 404。

### 从 CI 产物部署（免本地编译）

每次 PR / push 的 Verify job 会上传 `web-dist` artifact（Pages 用的 `/jblock/` 包）。在 Actions 运行页下载解压后，用静态服务器托管即可；若需要根路径访问，仍建议按上一节在本地构建一份 `VITE_BASE=/` 的包。

## CI / CD

GitHub Actions workflow [`.github/workflows/ci.yml`](.github/workflows/ci.yml)：

- **CI**（`pull_request` + `push`）：`cargo test`，以及 `pnpm` 的 `wasm` / `lint` / `typecheck` / `build`（与 `./init.sh` 对齐）。
- **CD**（仅 `main` push）：将 `web/dist` 部署到 GitHub Pages（仓库需在 Settings → Pages 选择 **GitHub Actions** 作为源；`VITE_BASE=/jblock/`）。

本地验证仍推荐：

```bash
./init.sh
```

## 工作原理

1. 前端读取本地 dump 文件内容（纯文本）。
2. 调用 WASM 导出的 `analyzeDump(text)`（对应 Rust `src/lib.rs`）。
3. Rust 侧 `parser::analyze` 检测格式、按线程切块、提取名称/ID/状态/锁信息，
   统计状态分组，并根据"持锁-等锁"关系构建锁竞争边。
4. 结果以 JS 对象返回前端并渲染；可导出为 HTML。

## License

MIT
