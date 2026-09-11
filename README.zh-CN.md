<p align="center">
  <img src="assets/herduck-logo.png" alt="Herduck — Make AI work for you. 白色兜帽小鸭坐在电脑前。" width="880">
</p>

<h1 align="center">Agent 窗口管理与项目进度看板。</h1>

<p align="center">管窗口，看进展，让人和 Agent 共同掌握项目全局。</p>

<p align="center">
  <a href="#安装">安装</a> ·
  <a href="#功能特点">功能特点</a> ·
  <a href="docs/configuration.md">配置文档</a> ·
  <a href="README.md">English</a>
</p>

Herduck 基于 [Herdr](https://github.com/ogulcancelik/herdr) 发展而来。Herdr 是面向 Agent、
类似 **tmux** 的终端窗口管理与运行环境；Herduck 保留多工作区、标签页、分屏和持久会话能力，
再把 **Agent 状态看板、项目视图和本机会话库** 放到同一个工作台。

人和 Agent 可以共同管理项目中的工作：看清谁在运行、哪里需要接手，追踪相关对话，再回到正确的上下文继续。
人通过终端界面操作，Agent 通过 CLI 和 JSON API 查看状态、管理窗口与工作区。

## 功能特点

| 能力 | 能帮你管理什么 |
| --- | --- |
| **Agents** | 管理终端窗口、分屏和工作区，全局查看 Agent 的运行状态。 |
| **Topics** | 开启语义整理后，把不同目录、不同 Agent 的相关对话自动归类。 |
| **Projects** | 沿用文件夹管理方式，为项目建立工作区，并在独立的项目视图中查看会话。 |
| **Sessions** | 自动汇集本机支持的 Agent 历史，查找、预览并继续对话。 |
| **人和 Agent 共用的控制接口** | 通过界面、CLI 和 API 查看状态、管理窗口、协调工作。 |
| **模型来源与 fallback** | 为主题整理和会话命名配置 Agent CLI、API 及模型的尝试顺序。 |
| **会话与资源管理** | 识别不活跃的 Agent，按需恢复历史；自动回收后台进程为规划中能力。 |

## 安装

macOS 或 Linux，安装 **Node.js 20+** 后运行：

```sh
npm install -g herduck@alpha
herduck
```

不想全局安装，可以直接体验：

```sh
npx --yes herduck@alpha
```

当前是 **alpha 预览版**。首次启动会下载对应系统的原生程序并校验 SHA-256，无需安装 Rust 或 Zig。
各个 Agent CLI 需要自行安装和登录。

预编译程序支持 Apple silicon / Intel Mac，以及 glibc 2.39+ 的 x64 / arm64 Linux
（例如 Ubuntu 24.04）。暂不支持 Windows 和 Alpine/musl。
[安装文档](docs/installation.md)包含固定版本安装、GitHub 下载、源码安装、升级和常见问题。

## Agents：窗口管理与全局状态看板

并排运行不同的 Agent CLI，用鼠标分屏、调整大小，在标签页和工作区之间切换。
退出客户端后，后台服务继续维持终端；重新打开 `herduck` 即可接回。

Agent 面板把不同工作区的运行情况放到一起，区分正在工作、等待输入、已完成和不活跃状态。
点击条目即可回到对应终端；状态变化、视觉提示和可配置通知，让你及时发现需要关注的工作。

![Herduck 原生 Agents 截图：终端分屏与 Agent 运行状态总览](assets/screenshots/agents.png)

## Topics：按主题自动归类

开启语义整理后，**Topics** 会根据对话内容，把跨目录、跨 Agent 的相关会话归到同一个主题下。
你可以同时跟进多个主题，展开分组，查看组成它的对话；最近活跃的主题会排在前面。

主题生成默认关闭。关闭生成或模型来源暂时不可用时，已有分组仍可查看；新会话可在来源恢复后继续整理。

**0.1.0-alpha.3 源码预览**新增 **Topic 封面**：点开主题，在对话列表上方填写这周目标、最多三条下一步、
以及卡住说明。人可以在界面编辑保存，Agent 可以通过 `herduck topic` 命令或 socket API 读写同一份封面；
重启后仍然保留。[使用说明](docs/topic-covers.md)。

![Herduck 原生 Topics 截图：跨目录、跨 Agent 汇集相关会话](assets/screenshots/topics-zh.png)

## Projects：按目录组织，保留独立项目视图

**Projects** 沿用熟悉的文件夹管理方式。你可以主动创建项目目录，并为它新建工作区；
该目录下支持的 Agent 对话会自动归入项目分组。每个分组都有自己的展开视图，可查看近期会话和正在打开的会话。

展开项目、预览对话、返回对应终端，项目上下文始终与实际工作的目录关联。

![Herduck 原生 Projects 截图：按工作目录分组，查看项目内会话](assets/screenshots/projects-zh.png)

## Sessions：本机 Agent 对话的全局会话库

**Sessions** 自动发现本机支持的 Agent 历史，把在不同终端中开始的对话汇集到同一个可搜索列表。
当前历史适配器支持 **Claude Code、Codex、Pi、OpenCode 和 Grok**，也可以配置额外的历史目录。

查看历史时先打开预览；选择继续后，再由原来的 Agent 恢复支持续接的会话。
如果对应会话已经在 Herduck 中运行，就直接切回它。Projects 和 Topics 为这份会话库提供另外两种查找方式。

![Herduck 原生 Sessions 截图：会话列表、历史预览与继续对话入口](assets/screenshots/sessions-zh.png)

*图片为 Herduck v0.1.0-alpha.2 在 Ghostty 中运行的原生窗口截图，历史对话和主题使用预设示例。
Agent 页面展示原生 CLI、演示会话及本地测试结果；示例内容仅用于展示界面能力。
[截图说明](assets/screenshots/README.md)。*

## 人和 Agent 共用的项目工作台

Agent 和脚本也能操作同一个工作台。CLI 与 JSON API 支持列出 Agent、读取终端输出、发送输入、
等待状态变化，以及创建和管理工作区、标签页与窗格。Agent 还可以上报工作区元数据，和正在进行的工作一起展示。

例如，为已有项目目录新建工作区，再查看 Agent 列表：

```sh
herduck workspace create --cwd /path/to/project --label my-project
herduck agent list
```

这样，项目组织、Agent 进度总览和具体操作就能连接起来，由人和自动化共同管理。
完整入口见 `herduck agent --help` 和 [API schema](docs/api/herduck-api.schema.json)。

## 多 Agent 支持与模型 fallback

内置 **19 种 Agent CLI 的界面状态识别规则**，包括 Claude Code、Codex、OpenCode、Pi、Gemini CLI、
Cursor、GitHub Copilot、Kimi、Grok 和 Hermes 等。状态识别与历史导入的支持范围不同；历史适配器见上方 Sessions 说明。

主题整理和会话命名可以使用 **OpenCode、Pi、Codex、Hermes CLI**，也可以接入 **OpenAI 兼容 API**。
你可以配置来源顺序，以及每个来源内部的模型顺序：模型被拒绝时尝试下一个模型，启动或传输失败时切换来源。
所有来源均不可用时，已有 Topics 继续保留，会话名称回退到本地文本生成；手动名称不会被覆盖。
[配置来源与 fallback](docs/configuration.md#summary-sources-and-fallback-order)。

## 会话可追踪，资源按需使用

浏览已保存的历史不会启动 Agent；选择继续时才按需启动，并复用已运行的对应会话。
你也可以手动结束不再需要的 Agent 进程，保留其 CLI 已保存的对话，之后查看或按原 Agent 的能力恢复。

当前版本默认在 Agent 连续一小时没有输入、输出或状态变化后，将空闲 Agent 标记为 **inactive**。
正在工作或等待用户响应、操作确认的 Agent 不会被这样标记。标记后进程和终端仍然保留，阈值可以配置。
[不活跃 Agent 设置](docs/configuration.md#inactive-agents)。

**规划中：自动回收后台进程。** 按配置的闲置时长结束不活跃的后台进程，释放内存，保留已保存的历史记录，
需要时再恢复。当前 alpha 尚未实现自动回收。

## 用自己的工具，按自己的方式配置

欢迎页可以打开配置，也可以跳过后直接使用 shell。选择 **herduck · menu → settings** 打开设置，
也可以在终端中按 **Ctrl+B，再按 S**。Sessions、Summaries 和 Session names 展示当前配置、来源优先级和模型顺序。

点击 **Open config file** 修改配置文件，保存后关闭并重新打开设置即可加载。
`herduck config check` 可以检查文件。Agent 的登录继续由各自 CLI 管理。

模型摘要、命名和主题整理**默认关闭**。启用后，所选对话内容会发送给你配置的 Agent 或 API 来源；
也可以只使用本地命名。完整示例和数据位置见[配置文档](docs/configuration.md)。

## 开发与许可

按照[源码构建说明](docs/installation.md#build-from-source)安装固定版本的 Rust 和 Zig，
使用 `just build` 构建，使用 `just check` 运行完整检查。

Herduck 是基于 Herdr v0.7.4 的独立项目，采用 **AGPL-3.0-or-later**，保留上游版权和许可声明。
安装后直接运行 `herduck`，无需额外安装 Herdr。
参见[上游来源](docs/UPSTREAM.md)、[贡献说明](CONTRIBUTING.md)和[安全报告](SECURITY.md)。
