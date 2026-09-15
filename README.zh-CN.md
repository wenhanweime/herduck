<p align="center">
  <img src="assets/herduck-logo.png" alt="Herduck — Make AI work for you. 白色兜帽小鸭坐在电脑前。" width="880">
</p>

<h1 align="center">面向 Agent 的持续工作层。</h1>

<p align="center">Agent 负责执行，Herduck 让工作持续推进。</p>

<p align="center">
  <a href="#安装">安装</a> ·
  <a href="#功能特点">功能特点</a> ·
  <a href="#人和-agent-共同管理工作">人和 Agent</a> ·
  <a href="#work-接下来要做到什么">产品方向</a> ·
  <a href="README.md">English</a>
</p>

Herduck 是让**人和 Agent 一起组织、跟进和继续工作的终端工作台**。
它融合**类似 tmux 的 Agent 窗口管理**、相关会话和项目上下文：运行自己的 Agent，
找到它们参与的工作，再回到正确的会话继续推进。

它基于 [Herdr](https://github.com/ogulcancelik/herdr)，保留熟悉的工作区、标签页、分屏，
以及退出客户端后仍然运行的终端。**人使用终端界面，Agent 使用 CLI 和 JSON API。**
源码预览进一步提供双方都能更新的目标与下一步，以及近期进展和可以直接执行的跟进建议。

我们希望每次回到一件工作时，你都能知道：**要完成什么、现在做到哪里、哪里需要关注、下一步做什么。**

## 安装

macOS 或 Linux，安装 **Node.js 20+** 后运行：

```sh
npm install -g herduck@alpha
herduck
```

也可以直接体验：

```sh
npx --yes herduck@alpha
```

**GitHub 源码中的 Topics 已更名为 Work。** npm alpha 当前仍安装 **0.1.0-alpha.2**，
使用旧名称。请根据所需能力选择版本：

| 版本 | 可用能力 |
| --- | --- |
| **npm alpha · 0.1.0-alpha.2** | Agent 窗口与状态、本机会话库、按目录组织的 Projects、以 Topics 命名的相关会话分组，以及 CLI/API 控制。 |
| **源码预览 · 0.1.0-alpha.4** | Work 新名称、保存目标和下一步、描述近期进展、把跟进建议交给原 Agent 执行。[预览分支与使用说明](https://github.com/wenhanweime/herduck/pull/1)。 |

首次启动会下载原生程序并校验 SHA-256，无需安装 Rust 或 Zig。各个 Agent CLI 需要自行安装和登录。
预编译程序支持 Apple silicon / Intel Mac，以及 glibc 2.39+ 的 x64 / arm64 Linux
（例如 Ubuntu 24.04）。暂不支持 Windows 和 Alpine/musl。
[安装文档](docs/installation.md)包含源码构建、原生程序下载和升级说明。

## 功能特点

四个视图把工作与执行过程、目录和历史连接起来：

| 视图 | 能帮你做什么 |
| --- | --- |
| **Agents** | 管理终端窗口，看清哪些 Agent 正在工作、哪些需要关注。 |
| **Sessions** | 查找本机支持的 Agent 历史，回到正确的对话。 |
| **Projects** | 按工作目录组织，主动创建项目目录和工作区，查看其中的会话。 |
| **Work** | 把跨目录、跨 Agent 的相关会话归到一起；源码预览进一步提供共用的目标和下一步。 |

### 看清工作在哪里执行

并排运行不同的 Agent CLI，用鼠标分屏、调整大小，在标签页和工作区之间切换。
退出客户端后，终端继续运行；重新打开 `herduck` 即可接回。

状态提示和可配置通知帮助你发现 Agent 正在工作、等待输入、已经空闲或不活跃。
点击 Agent 即可回到对应终端。空闲表示 Agent 可以接收下一条指令；确认工作完成仍需检查结果。

![Herduck v0.1.0-alpha.2 原生 Agents 截图：终端分屏与 Agent 活动](assets/screenshots/agents.png)

*图片为 v0.1.0-alpha.2 在 Ghostty 中运行的原生窗口截图，使用预设示例历史。
该版本仍将 Work 标为 Topics。[截图说明](assets/screenshots/README.md)。*

### 把相关会话组织在一起

**Work** 按含义整理相关会话，可以跨越目录和 Agent。你可以同时跟进多件工作，展开分组查看历史。
自动整理默认关闭；关闭生成或模型来源暂时不可用时，已有分组仍然保留。

![Herduck v0.1.0-alpha.2 相关会话视图：该版本名称仍为 Topics](assets/screenshots/topics-zh.png)

**Projects** 保留熟悉的目录视图，目录中可以是代码、文档或其他工作材料。
你可以创建项目目录并为它新建工作区，该目录下支持的会话会归入项目分组。

![Herduck v0.1.0-alpha.2 原生 Projects 截图：按工作目录组织会话](assets/screenshots/projects-zh.png)

### 回到正确的对话继续

**Sessions** 自动发现本机 **Claude Code、Codex、Pi、OpenCode 和 Grok** 的历史，也支持配置额外历史目录。
你可以搜索会话库、预览支持读取的对话，浏览时无需启动 Agent。OpenCode 已支持历史索引，暂不支持对话正文预览。

选择继续后，Herduck 会使用原来的 Agent 恢复支持续接的会话；如果对应会话已经运行，就直接切回它。
Work 和 Projects 为同一份会话历史提供另外两种查找方式。

![Herduck v0.1.0-alpha.2 原生 Sessions 截图：会话历史与继续对话入口](assets/screenshots/sessions-zh.png)

### 看近期进展，直接推进建议——源码预览

打开一项 Work 或 Project，可看到根据近期请求和 Agent 回应整理的进展描述与下一步建议。
Work 还可以编辑目标、最多三条下一步和卡点，保存的计划跨服务重启保留。

点击 **推进此建议**，将选定的跟进交给原 Agent。Herduck 会复用对应的运行中会话，或恢复原始对话；
Agent 忙碌时先排队，准备好后再发送。**查看会话** 可以追溯上下文；遇到等待你答复的 Agent，则打开它进行回应。
控件和自动生成的描述跟随配置的中文或英文，导航标签保持英文。

当前预览从各个会话提取进展和建议；综合整件工作的进展、组装跨会话上下文属于后续能力。
跟进记录仅在本次服务运行期间保留；“已发送”表示指令送达，执行结果仍需验证。
[预览版使用说明与 API](https://github.com/wenhanweime/herduck/blob/feat/topic-cover-alpha.3/docs/project-overview.md)。

## 人和 Agent 共同管理工作

**Agent 也能管理工作。** CLI 与 JSON API 支持列出 Agent、读取终端输出、发送输入、等待状态变化，
以及创建和管理工作区、标签页与窗格：

```sh
herduck workspace create --cwd /path/to/project --label my-project
herduck agent list
```

在**源码预览**中，Agent 还可以读取进展，更新已保存的目标、下一步和卡点，发起跟进并查询投递状态。
界面读取同一份已保存计划。例如，在匹配版本的服务运行时：

```sh
herduck topic list
herduck topic cover get TOPIC_KEY
herduck topic overview get TOPIC_KEY
herduck topic cover update TOPIC_KEY \
  --next-step "审核最新结果" \
  --blocked-note "等待反馈"
```

这些命令返回 JSON。将 `TOPIC_KEY` 替换为列表中的标识。
**Work 继续兼容现有的 `herduck topic` 命令和 `topic.*` API 名称。**
详见[计划接口与并发修改保护](https://github.com/wenhanweime/herduck/blob/feat/topic-cover-alpha.3/docs/topic-covers.md)
及[跟进控制](https://github.com/wenhanweime/herduck/blob/feat/topic-cover-alpha.3/docs/project-overview.md#cli-and-socket-api)。

## Work 接下来要做到什么

一件工作应该在最初的会话结束后继续存在。Herduck 正在向人或其他 Agent 都能接手的共同工作上下文发展：

- **保留少量明确状态：**目标、生命周期、阻塞和下一步，由人和 Agent 共同读取和更新。
- **根据证据理解进展：**把相关历史与当前工作连接起来，保留来源，区分已确认事实和待验证判断。
- **带着上下文继续：**把选定的下一步、目标、约束和相关历史带入执行，让结果可供下次接手时使用。

完整流程仍在规划中，当前源码预览以已保存计划和会话跟进为基础。
重新归类后仍稳定的工作身份、整件工作的综合进展、生命周期更新和上下文组装是接下来的方向；跨设备连续性属于未来能力。

## 使用自己的 Agent，选择模型来源

Herduck 内置 **19 种 Agent CLI 的界面状态识别规则**，包括 Claude Code、Codex、OpenCode、Pi、Gemini CLI、
Cursor、GitHub Copilot、Kimi、Grok 和 Hermes 等。状态识别、历史索引、正文读取和恢复会话的支持范围各不相同，
历史支持见上方说明。

工作整理和会话命名可以使用 **OpenCode、Pi、Codex、Hermes CLI**，也可以接入 **OpenAI 兼容 API**。
你可以配置来源顺序和每个来源内部的模型顺序：模型被拒绝时尝试下一个模型，启动或传输失败时切换来源。
所有来源均不可用时，已有工作分组继续保留，会话名称回退到本地文本生成；手动名称不会被覆盖。
[配置来源与备用顺序](docs/configuration.md#summary-sources-and-fallback-order)。

### 本地保存，按需启用模型

会话索引和已保存计划存储在本地。模型摘要、命名和工作整理**默认关闭**。
启用后，所选对话内容会发送给你配置的 Agent 或 API 来源；使用 Agent CLI 作为来源，也可能调用它的远程模型。
各个 Agent 的登录仍由它自己的工具管理。

选择 **herduck · menu → settings**，或在终端中按 **Ctrl+B，再按 S** 打开设置。
Sessions、Summaries 和 Session names 展示当前配置。点击 **Open config file** 修改并保存，关闭后重新打开设置即可加载。
运行 `herduck config check` 检查文件。完整示例和数据位置见[配置文档](docs/configuration.md)。

### 保留工作痕迹，按需使用资源

浏览历史不会启动 Agent；选择继续时才按需启动，并复用已运行的对应会话。
你可以结束不再需要的进程，保留其 CLI 已保存的历史，之后查看或按原 Agent 的能力恢复。

空闲 Agent 在一段时间没有活动后会被标为 **inactive**，默认一小时，可配置。
正在工作或等待用户响应、操作确认的 Agent 不参与此标记。标记后终端与进程仍然保留。
**规划中：自动结束不活跃的后台进程，释放内存并保留已保存的历史。**
[不活跃 Agent 设置](docs/configuration.md#inactive-agents)。

## 开发

按照[源码构建说明](docs/installation.md#build-from-source)安装固定版本的 Rust 和 Zig，然后运行：

```sh
just build
just check
```

`just check` 包含格式、公共源码检查、npm 安装器测试、Clippy 和 Rust 测试。
仓库命令还需要 `just`、`zsh`、Python 3 和 Node.js 20+。
参见[贡献说明](CONTRIBUTING.md)和[安全报告](SECURITY.md)。

## 许可与来源

Herduck 是基于 **Herdr v0.7.4** 的独立项目，采用 **AGPL-3.0-or-later**，保留上游版权和许可声明。
安装后直接运行 `herduck`，无需额外安装 Herdr。
参见 [LICENSE](LICENSE) 和[上游来源](docs/UPSTREAM.md)。
