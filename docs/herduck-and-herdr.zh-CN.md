# Herduck 和 Herdr：各自做什么，怎么一起用

[English](herduck-and-herdr.md)

*作者本人写的。关于 Herdr 的说法来自它自己的 README 和文档（文末有链接），核对时间 2026-10-30。
如果有变化，以 Herdr 官方文档为准。*

## 一句话

- **Herdr** 是给编程 Agent 用的终端运行环境，回答的是：*我的 Agent 都在哪、哪个在等我？*
- **Herduck** 从 Herdr 的代码出发，在上面加了一层，回答的是：*这件事为了什么、做到哪了、下一步干什么？*
- 它们是**两个独立的程序**。Herduck 不是 Herdr 插件，也不需要先装 Herdr。两个都用的话，有一个可选的小桥接插件。

## Herdr 做什么

按它 README 的说法，Herdr 是"编程 Agent 运行在上面的那一层"。说白了：

- 后台服务让终端在你关掉窗口或断开 SSH 后继续跑，之后可以接回来。
- 每个面板会标出在干活、被卡住、还是空闲，一眼看出哪个 Agent 需要你。
- Agent 自己也能通过命令行和 socket API 操作它。
- 能在一个窗口里看多台机器，还有插件市场。
- 它不包装、不替换你用的 Agent——Claude Code、Codex 这些照常用。

Herdr 是更早、更成熟的项目，用户多得多，也在持续发布新版本。

## Herduck 多了什么

Herduck 是从 **Herdr v0.7.4** 派生出来的独立项目（见 [UPSTREAM.md](UPSTREAM.md)）。运行环境那部分——
面板、标签页、工作区、断开再接回、能识别 19 种 Agent CLI 的状态——都保留了，另外加了：

- **Work（工作）**：把不同 Agent、不同目录的对话归到同一件事里，带一个可编辑的目标、最多三条下一步、
  一条卡点备注。会话结束、服务重启都还在，自动更新不会覆盖你自己写的。
- **Sessions（会话）**：把本机 Claude Code、Codex、Pi、OpenCode、Grok 的历史对话放进一个能搜索的列表，
  包括不是在 Herduck 里启动的。（OpenCode 的历史能被收录，但暂时不能预览内容。）
- **Continue with this（接着做）**：把选中的下一步发给原来的 Agent，能复用正在运行的会话，或者恢复原来那段对话。
- 命令行和 JSON API（`herduck work list`、`herduck work plan get/update`、`herduck work overview get`），
  让 Agent 能读到和人看到的同一份计划。

自动归类成 Work、会话命名、进展描述默认关闭，需要你自己配置模型来源，见[配置文档](configuration.md)。

## 两者的关系，说清楚

- Herduck 有自己的程序（`herduck`）和自己的后台服务。装 Herduck 不会安装或改动 Herdr。
- 它**不是** Herdr 官方版本。Herdr 在 v0.7.4 之后又更新了很多，所以如果你只要运行环境这部分
  （断开再接回、Agent 状态、多台机器），现在 Herdr 本身功能更多。
- 许可证：AGPL-3.0-or-later，和 Herdr 一样，原有版权声明都保留。Herdr LICENSE 里的商业授权说明属于
  Herdr 的权利人，不是 Herduck 的。

## 两个都用：桥接插件

如果你每天用 Herdr，又想看 Herduck 的 Work 视图，可以装这个仓库里的小桥接插件：

```sh
herdr plugin install wenhanweime/herduck/herdr-plugin
```

需要 Herdr 0.7.0 或更新版本，并且 `herduck` 在你的 `PATH` 里。它加了两个动作：

| 动作 | 做什么 |
| --- | --- |
| Open Herduck | 在 Herdr 里用浮层面板打开 Herduck。 |
| Show Herduck work status | 打印本机 Herduck 服务里最多八件 Work。装了 `jq` 会显示成更整齐的表格。 |

可选快捷键（`prefix+h` 打开 Herduck）的写法见[桥接插件 README](../herdr-plugin/README.md)。

插件只运行 [herdr-plugin/](../herdr-plugin/) 里的两个小脚本，不会把你的 Herdr 会话搬进 Herduck。

## 该用哪个？

- **只用 Herdr**：你要的是一个靠谱的终端运行环境，能看出哪个 Agent 在等你；每件事在干嘛你自己记得住。
- **只用 Herduck**：你主要想要 Work 和 Sessions 视图，也能接受早期 Alpha（macOS 和 Linux；Work 功能需要
  [从源码编译](installation.md)）。
- **两个一起，加桥接插件**：Herdr 还是你的主力终端；需要看某件事的目标、卡点、下一步时，再打开 Herduck。

另见：[和 `claude --resume`、tmux、Herdr 及其他工具的对比](compare-resume-tmux.zh-CN.md)。

## 资料来源

- Herdr：<https://github.com/herdrdev/herdr> · <https://herdr.dev/docs/> ·
  [插件市场](https://herdr.dev/plugins/)
- Herduck：[README](../README.zh-CN.md) · [UPSTREAM.md](UPSTREAM.md) ·
  [桥接插件](../herdr-plugin/README.md)
