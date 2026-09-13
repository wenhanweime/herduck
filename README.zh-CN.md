<p align="center">
  <img src="assets/herduck-logo.png" alt="Herduck — Make AI work for you. 白色兜帽小鸭坐在电脑前。" width="880">
</p>

<h1 align="center">会话会结束，工作要接得上。</h1>

<p align="center">Agent 窗口管理，以及人和 Agent 共用的项目工作台。</p>

<p align="center">
  <a href="#安装">安装</a> ·
  <a href="#功能特点">功能特点</a> ·
  <a href="#持续的工作状态">产品方向</a> ·
  <a href="docs/configuration.md">配置文档</a> ·
  <a href="README.md">English</a>
</p>

Herduck 把 **Agent 终端、会话历史和项目视图** 放到同一个终端工作台。
你可以同时运行多个 Agent，看清谁需要关注，找到相关对话，再回到正确的上下文继续工作。

它基于 [Herdr](https://github.com/ogulcancelik/herdr) 发展而来。Herdr 是面向 Agent、类似 **tmux**
的终端窗口管理与运行环境，支持工作区、标签页、分屏，以及退出客户端后仍然运行的终端会话。
Herduck 在这个基础上增加跨 Agent、跨会话的工作组织：人使用终端界面，Agent 使用 CLI 和 JSON API。

我们希望你每次回到一件工作时，都能知道：**要完成什么、现在做到哪里、什么需要决定、下一步做什么。**
Herduck 正在向一份持续存在的工作状态发展，让人和 Agent 都能读取、维护，并据此行动。

| 可用范围 | 目前包含什么 |
| --- | --- |
| **npm alpha · 0.1.0-alpha.2** | Agent 窗口与状态、本机会话库、按目录组织的 Projects、语义归类的 Topics，以及 CLI/API 控制。 |
| **源码预览 · 0.1.0-alpha.4** | 保存 Topic 计划、描述近期进展、把跟进建议交给原 Agent 执行。见[预览 PR](https://github.com/wenhanweime/herduck/pull/1)。 |
| **规划中** | 跨会话维护工作全貌，记录决策和失败尝试，按优先级与依赖安排行动，将执行结果写回工作状态。 |

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

npm alpha 当前安装的是 **0.1.0-alpha.2**。首次启动会下载原生程序并校验 SHA-256，无需安装 Rust 或 Zig。
各个 Agent CLI 需要自行安装和登录。源码预览功能需要使用[预览分支](https://github.com/wenhanweime/herduck/pull/1)。

预编译程序支持 Apple silicon / Intel Mac，以及 glibc 2.39+ 的 x64 / arm64 Linux
（例如 Ubuntu 24.04）。暂不支持 Windows 和 Alpine/musl。
[安装文档](docs/installation.md)包含固定版本安装、原生程序下载、源码安装、升级和常见问题。

## 功能特点

四个视图把执行环境、会话历史和项目上下文连接起来：

| 视图 | 能帮你做什么 |
| --- | --- |
| **Agents** | 管理终端窗口，全局查看 Herduck 各个工作区中的 Agent 活动。 |
| **Sessions** | 查找本机支持的 Agent 历史，包括在 Herduck 外开始的工作，并继续支持恢复的对话。 |
| **Projects** | 按目录跟进工作，主动创建项目目录和工作区，查看各个项目的对话。 |
| **Topics** | 开启语义整理后，把跨目录、跨 Agent 的相关对话自动归到一起。 |

### 看清正在运行的 Agent

并排运行不同的 Agent CLI，用鼠标分屏、调整大小，在标签页和工作区之间切换。
退出客户端后，终端继续运行；重新打开 `herduck` 即可接回。

状态提示和可配置通知帮助你发现 Agent 正在工作、等待输入、已经空闲或不活跃。
点击 Agent 即可回到对应终端。运行状态描述的是 Agent；工作是否完成，还需要查看结果来确认。

![Herduck 原生 Agents 截图：终端分屏与 Agent 运行状态总览](assets/screenshots/agents.png)

### 跨会话找到同一件工作

**Topics** 按对话含义整理相关会话，可以跨越目录和 Agent。你可以同时跟进多个主题，展开分组查看历史。
主题生成默认关闭；关闭生成或模型来源暂时不可用时，已有 Topics 仍然保留。

![Herduck 原生 Topics 截图：跨目录、跨 Agent 汇集相关会话](assets/screenshots/topics-zh.png)

**Projects** 保留熟悉的目录视图。你可以创建项目目录并为它新建工作区，该目录下支持的会话会归入项目分组。
每个项目都有自己的视图，可查看近期会话和正在打开的会话。

![Herduck 原生 Projects 截图：按工作目录分组，查看项目内会话](assets/screenshots/projects-zh.png)

### 回到正确的对话继续

**Sessions** 自动发现本机 **Claude Code、Codex、Pi、OpenCode 和 Grok** 的历史，也支持配置额外历史目录。
你可以搜索会话库、预览支持读取的对话，浏览时无需启动 Agent。OpenCode 已支持历史索引，暂不支持对话正文预览。

选择继续后，Herduck 会使用原来的 Agent 恢复支持续接的会话；如果对应会话已经运行，就直接切回它。
Projects 和 Topics 为同一份会话历史提供另外两种查找方式。

![Herduck 原生 Sessions 截图：会话列表、历史预览与继续对话入口](assets/screenshots/sessions-zh.png)

*图片为 Herduck v0.1.0-alpha.2 在 Ghostty 中运行的原生窗口截图，历史对话和主题使用预设示例。
Agent 页面展示原生 CLI、演示会话及本地测试结果，用于说明现有界面能力。
[截图说明](assets/screenshots/README.md)。*

### 看近期进展，直接推进建议——源码预览

在 **0.1.0-alpha.4** 中，打开 Topic 或 Project，可看到根据近期请求和 Agent 回应整理的进展描述与下一步建议。
Topic 还可以编辑目标、最多三条下一步和卡点，保存的计划跨服务重启保留。

点击 **推进此建议**，将选定的跟进交给原 Agent。Herduck 会复用对应的运行中会话，或恢复原始对话；
Agent 忙碌时先排队，准备好后再发送。**查看会话** 可以追溯上下文；遇到等待你答复的 Agent，则打开它进行回应。
控件和自动生成的描述跟随配置的中文或英文，左侧四个导航标签保持英文。

当前预览仍从各个会话提取进展和建议，跨会话综合工作状态是后续方向。跟进记录目前仅在本次服务运行期间保留；
“已发送”表示指令送达，执行结果仍需验证。
详见[预览版使用说明与 API](https://github.com/wenhanweime/herduck/blob/041f35fc68f8a055f0fcfe427073496433112639/docs/project-overview.md)。

### 人和 Agent 操作同一个工作台

CLI 与 JSON API 支持列出 Agent、读取终端输出、发送输入、等待状态变化，以及创建和管理工作区、标签页与窗格。
例如，为已有目录创建工作区，再查看 Agent：

```sh
herduck workspace create --cwd /path/to/project --label my-project
herduck agent list
```

源码预览还通过公共 API 提供 Topic 计划更新、项目概览和跟进控制。
当前检出版本的接口见 `herduck agent --help` 和 [API schema](docs/api/herduck-api.schema.json)，预览接口见上方说明。

## 持续的工作状态

一件工作经常横跨多个会话和 Agent。Herduck 接下来要维护这件工作的共同上下文，并让结论都能追溯到证据：

| 工作信息 | 人和 Agent 应该能看懂什么 |
| --- | --- |
| **目标与完成条件** | 最终要做成什么，怎样才算完成？ |
| **当前进展与阻塞** | 什么已确认，什么尚不确定，哪里需要关注？ |
| **决策与尝试** | 为什么选择这条路，哪些失败方案不该重试？ |
| **下一步行动** | 现在最值得做什么，为什么，需要人决定还是 Agent 执行？ |
| **成果与最近变化** | 离开后发生了什么，哪些文件、产出或对话支持这个结论？ |

目标是形成 **观察进展 → 维护状态 → 采取行动 → 写回结果** 的闭环。
事实、决定和待验证判断应始终有清楚的区别，摘要和交接也要保留这些区别。
新的 Agent 接手时，应能直接获得目标、约束、已有尝试和下一步。
这份工作记忆与行动闭环是产品的发展方向，目前尚未完整实现；跨设备连续性也属于未来能力。

## 使用自己的 Agent，选择模型来源

Herduck 内置 **19 种 Agent CLI 的界面状态识别规则**，包括 Claude Code、Codex、OpenCode、Pi、Gemini CLI、
Cursor、GitHub Copilot、Kimi、Grok 和 Hermes 等。状态识别、历史索引、正文读取和恢复会话的支持范围各不相同，
历史支持见上方说明。

主题整理和会话命名可以使用 **OpenCode、Pi、Codex、Hermes CLI**，也可以接入 **OpenAI 兼容 API**。
你可以配置来源顺序和每个来源内部的模型顺序：模型被拒绝时尝试下一个模型，启动或传输失败时切换来源。
所有来源均不可用时，已有 Topics 继续保留，会话名称回退到本地文本生成；手动名称不会被覆盖。
[配置来源与备用顺序](docs/configuration.md#summary-sources-and-fallback-order)。

### 本地保存，按需启用模型

会话索引和已保存计划存储在本地。模型摘要、命名和主题整理**默认关闭**。
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
