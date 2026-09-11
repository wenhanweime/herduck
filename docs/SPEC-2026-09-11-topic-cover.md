# SPEC：Topic 封面（大事摘要）

> 状态：v0.1 · 待验收\
> 日期：2026-09-11\
> 版本计划：随本功能发布 **0.1.0-alpha.3**（Cargo.toml + npm/package.json + 相关 manifest 同步）\
> 分支建议：`feat/topic-cover-alpha.3`

## 0. 一句话说清楚

点开一个 Topic（例如「首批 100 用户」）时，对话列表**上面**多一块固定区域，能看、能改三样东西：

1. **这周目标**（一两句话）\
2. **下一步**（最多 3 条）\
3. **现在卡在哪**（一句；能关联到某个会话/窗格时，点一下就跳过去——本轮若跳窗做不完，至少先能手写「卡在哪」）

**Done when：**

1. Topics 详情里能看到封面；空封面不挡浏览对话。\
2. 用户能编辑并保存目标 / 下一步 / 卡住说明；重启 Herduck 后还在。\
3. 有 CLI 或 socket API 能读、能改封面（方便以后 Codex 等 Agent 当入口用）。\
4. 版本号升到 `0.1.0-alpha.3`，changelog/release notes 有一条人话说明。\
5. `just check`（或仓库等价检查）通过。

## 1. 为什么做

现在 Topic 主要是「把对话拢在一起」。创始人要用它管事，还缺一张「封面」：不用翻聊天就能知道这件大事要干啥、下一步啥、卡哪了。

这是「创始人 OS」的第一步，**不是**整套多人项目管理（不做审批流、不做多人权限、不做云端代跑）。

## 2. 功能

### F1. 数据模型 · P0

每个 Topic 增加封面字段（名字可按现有代码风格微调，语义别改）：

- `goal: string` — 这周目标\
- `next_steps: string[0..=3]` — 下一步\
- `blocked_note: string` — 现在卡在哪（人写）\
- `blocked_session_ref`（可选）— 若已有稳定会话 id，可挂上；本轮没有可靠映射就先空着\
- `updated_at`

持久化进现有 Topic/catalog 存储；缺字段当空字符串/空列表，**不要**弄坏旧数据。

### F2. UI · P0

在 Topic 详情顶部画封面：

- 显示目标、下一步（编号 1–3）、卡住说明\
- 提供编辑入口（对话框或行内编辑，跟现有 Settings/对话框风格一致）\
- 没填时显示短提示，例如「点这里写下这周目标」\
- 不要挡住下面的会话列表

### F3. Agent / CLI 可读可写 · P0

至少提供一种机器入口（优先跟现有 `herduck` CLI / socket API 风格）：

- 读：某个 topic 的封面\
- 写：更新 goal / next_steps / blocked_note\

这样以后可以把 Herduck 当成「项目管理入口」：人在 UI 改，Agent 用命令改。

### F4. 版本与发布说明 · P0

- `Cargo.toml` 与 `npm/package.json`（及任何锁死同版本处）→ `0.1.0-alpha.3`\
- 在 release notes / 产品公告习惯位置加一条：**Topic 封面：目标、下一步、卡住说明**\
- 提交用小写 conventional commits，例如 `feat: add topic cover for goals and next steps`

## 3. 边界

**Always**

- 本地、可持久化、空值安全\
- UI 文案用简单中文或跟现有 UI 语言一致\
- 改动集中在 Topic/catalog/UI/API，别顺手大重构\

**Never（本轮不做）**

- 多人协作、权限、审批\
- 完整任务看板 / Linear 替代品\
- 自动从聊天「猜」目标（可后做；本轮只人手写）\
- 产出物列表（文件/链接清单）— 下一张 SPEC\
- 强制跳窗（有 `blocked_session_ref` 且现成 focus API 才接；没有就只做文字）

## 4. 验收

```bash
just test-public   # 或仓库文档要求的等价命令
just check
```

手动：

1. 打开一个 Topic → 看到封面区\
2. 填目标 + 两条下一步 + 卡住说明 → 保存 → 重启仍在\
3. 用 CLI/API 读出同样内容，再改 goal，UI 刷新后一致\

## 5. 和「全部项目管理入口」的关系

- **本轮**：Topic 变成「一件大事的封面 + 对话」，Herduck 开始像个人项目入口。\
- **下一轮**：产出列表、卡住一键跳窗。\
- **再往后**：默认有一个「工作台 Agent」专门读写封面/下一步——那时才更像「Herduck 本身是一个管全部项目的 Agent」。\
本 SPEC 只完成第一级。
