<div align="center">
  <h1> Wishes · 众愿 </h1>
  
  <img src="src/assets/logos/wishes.svg" alt="Wishes Logo" width="128">

  <p>
    <img src="https://img.shields.io/badge/framework-Tauri_2-blue?logo=tauri" />
    <img src="https://img.shields.io/badge/frontend-Vue_3-4FC08D?logo=vuedotjs" />
    <img src="https://img.shields.io/badge/NaiveUI-18A058?logo=naiveui&logoColor=white" />
    <img src="https://img.shields.io/badge/storage-SQLite-003B57?logo=sqlite" />
    <img src="https://img.shields.io/badge/LICENSE-GPL_v3.0_or_later-green" />
  </p>

  <p>
    <span>语言: 简体中文</span> |
    <a href="docs/README.en.md">English</a> |
    <a href="docs/README.ja.md">日本語</a>
  </p>
</div>

## 简介

> [!IMPORTANT]
>
> 当前的 `Wishes` 仍处于测试阶段，部分功能尚不完善。

`Wishes` 是一款通用的、可高度自定义的模拟抽卡工具。
通过 *标签系统* 统一管理所有卡片、卡组、卡池与抽卡逻辑，并利用 *组合式的逻辑引擎*，让你能够自由模拟 **几乎任何** 游戏的抽卡体验。

> 其实不止游戏：如果你愿意的话，`Wishes` 可以模拟 **几乎任何** 随机系统。

所有数据均存储于 **本地** ; 卡池的逻辑状态 (如保底计数) 会持久化至 `SQLite`, 每个卡池相互独立。

## v0.2.0 重要更新

> [!WARNING]
>
> **本次更新包含数据格式的不兼容变更**。加载器启动时会读取数据目录根部的 `version.json` 并校验版本；版本不符时直接失败，**没有自动迁移**。升级时请按 v2 格式重新组织数据目录。
>
> 若需要保留 v1 数据，请使用 [`scripts/migrate_v1_to_v2.py`](scripts/migrate_v1_to_v2.py) 迁移（用法见 [`scripts/README.md`](scripts/README.md)）。脚本不会修改原始数据目录。

- **数据格式 v2**
  - 所有核心对象（卡片 / 卡组 / 逻辑 / 卡池）统一使用 `GlobalId`（Uuid v7）作为唯一标识，取代旧版本的自增整数 Id。
  - 面向用户的文本字段（卡片内容、卡组名、卡池名、逻辑名）改为 **多语言映射**（语言标签 → 文本），由前端按当前界面语言回退解析。
  - 每个对象携带 **来源** 标记：`official`（官方）、`local`（本地）、`pack:<name>`（拓展包）。
  - 数据按 **来源物理分离**：`data/official/` 与 `data/packs/<name>/` 只读，`data/local/` 可写。

- **数据分叉模型**
  - 编辑只读来源的对象时，不再原地修改，而是 **派生** 出一个本地副本（新 `GlobalId`、`origin = local`、`forked_from` 指向原对象）。
  - 派生对象可以选择是否 **遮蔽** 原对象：遮蔽时原对象在列表与抽卡查询中被隐藏，卡组引用重写至派生对象；撤销遮蔽则反向重写回原对象。当前卡片已支持手动切换遮蔽，卡组的相应 UI 将在后续版本加入。
  - 删除派生对象后，原对象自动恢复可见。

- **状态作用域**
  - 卡池运行时状态的存储键从「卡池 Id」改为「作用域键」：
    - `banner:<uuid>`（默认）：每个卡池独立维护状态。
    - `group:<name>`：同组的多个卡池共享同一份状态（如跨池继承保底）。
  - **不兼容旧数据库**：若检测到 v1 时代的旧表结构，启动时会重建，旧的保底进度会丢失。

- **其他**
  - 加载器新增跨来源 `GlobalId` 唯一性校验、卡池引用的卡组 / 逻辑存在性校验。

## 功能清单

- [x] 标签系统
- [x] 卡池加载
- [x] 完整的抽卡流程
- [x] 数据格式 v2 (GlobalId / 多语言文本 / 来源分离)
- [x] 数据分叉模型 (只读来源编辑时派生本地副本)
- [ ] 数据管理
  - [x] 卡片管理 (含分叉与遮蔽切换)
  - [x] 卡组管理 (分叉已支持, 遮蔽切换待加入)
  - [ ] 抽卡逻辑管理
  - [ ] 卡池管理
- [ ] 卡组声明规则
  - [x] 成员规则
  - [ ] 活动组规则 (*不完善*)
- [ ] 内置抽卡逻辑 (*硬编码*)
  - [ ] *原神*
    - [x] *角色 · UP* (含 *捕获明光*)
    - [ ] *武器 · UP*
    - [ ] *常驻*
    - [ ] *新手*
  - [ ] *崩坏星穹铁道*
    - [x] *角色 · UP*
    - [ ] *光锥 · UP*
    - [ ] *常驻*
    - [ ] *新手*
  - [ ] *绝区零*
    - [ ] *角色 · UP*
    - [ ] *音擎 · UP*
    - [ ] *邦布*
    - [ ] *常驻*
    - [ ] *新手*
  - [ ] *明日方舟*
  - [ ] *鸣潮*
  - [ ] *蔚蓝档案*
    - [ ] *角色 · UP*
    - [ ] *角色 · FES*
- [ ] 自定义抽卡逻辑
  - [ ] 组合式抽卡规则
  - [ ] 自定义规则蓝图/脚本 (*预想*)
- [ ] 本地多用户隔离
- [ ] 配置全英文化
- [ ] 卡片立绘管理
- [ ] 更完善的抽卡动画
- [ ] 国际化
  - [x] 界面层多语言
  - [ ] 数据层多语言 (数据文件翻译待完善)
    - [x] 简体中文
    - [ ] English
    - [ ] 日本語
- [ ] 初始化引导

### 预想

- [ ] 建立远程数据仓库 (`wishes-data`)，自动拉取更新数据，并通过设置 `追踪的游戏`，仅拉取需要的内容

## 快速开始

### 下载安装包

普通用户可直接通过预编译的安装包快速体验 `Wishes`。

前往 [**Releases**](https://github.com/wishes-gacha/wishes/releases) 页面，下载对应平台的最新安装包。

下载后按常规方式安装并启动即可，首次启动时会把内置的默认卡片、卡组、逻辑与卡池数据复制到用户数据目录。

### 本地构建

面向希望自行编译、修改源码或参与开发的用户。

#### 环境要求

- [**Node.js**](https://nodejs.org/) ≥ 18
- [**Rust**](https://www.rust-lang.org/tools/install) (*stable*)
- 对应平台的 [**Tauri 2 系统依赖**](https://tauri.app/start/prerequisites/)

#### 开发模式

```bash
# 安装前端依赖
npm install

# 使用 npm 启动开发服务器
npm run tauri dev
# 或使用 cargo
# cargo run tauri
```

> [!NOTE]
>
> 开发模式下，应用会优先读取项目根目录中的 `data/` 与 `db/`，修改数据文件后重启即可生效。

### 构建发布版

```bash
# 使用 npm
npm run tauri build
# 或使用 cargo
# cargo tauri build
```

构建产物位于 `src-tauri/target/release/bundle`，按平台生成对应产物。

## 内置支持

`Wishes` 目前有以下受支持游戏清单:

- [x] 原神
- [x] 崩坏星穹铁道
- [ ] 绝区零
- [ ] 明日方舟
- [ ] 鸣潮
- [ ] 蔚蓝档案

> [!IMPORTANT]
>
> 游戏排名 **不分先后** !
>
> 此清单 **不固定**，未来可能有 **更多** 游戏加入到计划清单中。

`Wishes` 默认包含受支持游戏的 **所有卡片** 、提供受支持游戏的 **所有抽卡逻辑** 的实现。并随游戏更新而 **同步更新**。

## 更多文档

- [**`Wishes` 核心概念**](docs/concepts.md)：`Wishes` 的灵活性来自一些小巧而通用的抽象概念，理解这些概念后即可自定义 **任意** 卡池。
- **`data/` 数据目录结构说明** (待补充)
- [**English README** 说明文档](docs/README.en.md)
- [**日本語 README** 说明文档](docs/README.ja.md)

## 图片版权

### Wishes Logo

`Wishes` Logo 及其变体 (位于 `src/assets/logos/`) 的版权归 `Wishes` 项目及作者所有，采用与项目一致的 `GPL-3.0-or-later` 协议授权。在保留原始版权声明的前提下，可自由用于本项目的衍生项目。

### 卡片立绘与游戏素材

未来 `data/assets/` 目录中若包含来自各游戏的卡片立绘、图标或其他美术资源，其版权均归 **相应的游戏公司** 所有。这些素材仅用于 `Wishes` 的本地模拟展示，**未经版权方许可，不得用于任何其他用途**，包括但不限于商业用途、二次分发或再创作。

若版权方对相关素材的使用存在异议，请联系作者 (通过邮箱)，我们将在第一时间移除相关内容。

## 开源协议

`Wishes` 基于 **GNU General Public License v3.0 or later** (`GPL-3.0-or-later`) 协议开源。

> [!NOTE]
>
> `Wishes` 是自由软件: 你可以按照由自由软件基金会发布的 **GNU 通用公共许可证来** 再发布该软件或者修改该软件; 你可以使用该许可证的第 3 版, 或者 (作为可选项) 使用该许可证的任何更新版本。
>
> `Wishes` 的发布是希望它能发挥作用, 但是 **并无担保** ; 甚至也 **不担保** 其可销售性或适用于某个特殊的目的。请参看 GNU 通用公共许可证来了解详情。
>
> 完整的许可证文本可见仓库根目录的 [`LICENSE`](LICENSE) 文件。

任何基于 `Wishes` 的衍生项目 **必须** 开放其源代码，并保留原始的版权声明与许可证声明。

<div align="center">
  <sub>Copyright (C) 2026 G-GAZE and contributors · <a href="LICENSE">GPL-3.0-or-later</a></sub>
</div>
