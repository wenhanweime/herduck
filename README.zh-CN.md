<p align="center">
  <img src="assets/herduck-logo.png" alt="Herduck — Make AI work for you. 白色兜帽小鸭坐在电脑前。" width="880">
</p>

<h1 align="center">多个 AI 编程助手，一个终端工作台。</h1>

<p align="center">并排工作，找回对话，接着上次的进度继续。</p>

<p align="center">
  <a href="#安装">安装</a> ·
  <a href="docs/configuration.md">配置文档</a> ·
  <a href="README.md">English</a>
</p>

Herduck 把编程 Agent 的终端和历史对话放到一起。继续使用你熟悉的 Claude Code、Codex、
OpenCode、Pi 等 CLI，同时看清哪些任务在运行、之前的对话在哪里，以及该从哪里继续。

## 安装

macOS 或 Linux，安装 **Node.js 20+** 后运行：

```sh
npm install -g https://github.com/wenhanweime/herduck/releases/download/v0.1.0-alpha.2/herduck-0.1.0-alpha.2.tgz
herduck
```

不想全局安装，可以直接体验：

```sh
npx --yes --package=https://github.com/wenhanweime/herduck/releases/download/v0.1.0-alpha.2/herduck-0.1.0-alpha.2.tgz herduck
```

目前通过 GitHub 发布包安装，npm registry 上的包尚未发布。
当前是 **alpha 预览版**。首次启动会下载对应系统的原生程序并校验 SHA-256，无需安装 Rust 或 Zig。
各个 Agent CLI 需要自行安装和登录。

预编译程序支持 Apple silicon / Intel Mac，以及 glibc 2.39+ 的 x64 / arm64 Linux
（例如 Ubuntu 24.04）。暂不支持 Windows 和 Alpine/musl。
[安装文档](docs/installation.md)包含 GitHub 下载、源码安装、升级和常见问题。

## 正在做的事，一眼看清

让一个 Agent 写代码，另一个检查边界情况。并排打开终端，拖动调整大小，在项目之间切换。
关闭客户端后，后台服务和终端继续运行；重新打开 `herduck` 即可接回。

![Herduck 中并排运行的演示 Agent 终端](assets/screenshots/agents.png)

## 找到上次那段对话

**Sessions** 汇集支持的本地 Agent 历史，最近的工作排在前面。先预览内容，再用原来的 Agent
恢复支持继续的会话。如果它已经在运行，就直接切回对应终端。

![Sessions 中按最近活动排列的演示对话](assets/screenshots/sessions.png)

## 按项目找，也能按主题找

**Projects** 按工作目录整理会话。启用模型整理后，**Topics** 可以把不同项目里的相关工作放到一起。
关闭生成后，已有的名称和分组会保留。

![Projects 按文件夹整理演示会话](assets/screenshots/projects.png)

*以上图片来自实际运行的终端界面，对话和 Agent 输出均为演示内容。*

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
