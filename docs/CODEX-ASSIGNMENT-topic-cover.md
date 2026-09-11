# Codex 开发指派：Topic 封面

## 仓库

https://github.com/wenhanweime/herduck\
从 `main` 拉分支：`feat/topic-cover-alpha.3`

## 先读

1. `docs/SPEC-2026-09-11-topic-cover.md`（本功能唯一需求；冲突以它为准）\
2. `docs/contributor-agents.md`\
3. 现有 Topics / catalog / projects UI 相关代码（自行定位，不要臆造路径）

## 任务

按 SPEC 实现 **Topic 封面**：目标、最多 3 条下一步、卡住说明；可编辑；持久化；CLI 或 socket API 可读可写；版本升到 **0.1.0-alpha.3**。

## 版本管理（必须做）

1. 只在本功能分支上改，不要直接推 `main`（除非仓库惯例不同且文档写明）。\
2. 同步 bump：`Cargo.toml`、`npm/package.json`，以及任何写死 `0.1.0-alpha.2` 的发布相关文件。\
3. 提交信息小写 conventional，无 emoji、无 AI co-author。建议拆成：\
   - `docs: add topic cover spec`\
   - `feat: persist and render topic cover`\
   - `feat: expose topic cover via cli or api`\
   - `chore: bump version to 0.1.0-alpha.3`\
4. 开 PR 到 `main`，PR 描述用人话写：用户能看到什么、怎么验收、本轮明确没做啥。\
5. 跑通 `just test-public` 与 `just check`（或 CONTRIBUTING 等价命令），把实际跑过的命令写进 PR。

## 完成标准

SPEC「Done when」五条全部满足。做不了「跳到卡住窗格」时，在 PR 里写明并留 TODO，不要假装做了。

## 不要做

大重构、多人权限、自动从聊天生成目标、产出物列表（那是下一张单）。
