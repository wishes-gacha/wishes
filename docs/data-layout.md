# Wishes 数据目录布局

> [!NOTE]
>
> 本文档描述**数据格式 v2** 的目录结构与文件格式。
>
> 概念层面的说明（标签、来源、分叉、抽卡流程）见 [核心概念](concepts.md)。

`Wishes` 把全部业务数据保存在独立的 `data/` 目录中，运行时数据库与日志分别放在同级的 `db/`、`logs/` 目录。三者互不混用：

```text
<应用数据根目录>/
├── data/     # JSON 数据文件（版本化、可备份、可手工编辑）
├── db/       # SQLite 运行时数据库（状态与引用覆盖）
└── logs/     # 应用日志
```

## 一、目录位置

| 环境 | 数据目录 | 数据库目录 | 日志目录 |
| :--- | :--- | :--- | :--- |
| 开发（`debug_assertions`） | `<项目根>/data` | `<项目根>/db` | `<项目根>/logs` |
| 生产 | `<app_data_dir>/data` | `<app_data_dir>/db` | `<app_data_dir>/logs` |

`app_data_dir` 由 Tauri 按应用标识 `com.gaze.wishes` 生成，常见平台路径为：

| 平台 | 路径 |
| :--- | :--- |
| Linux | `~/.local/share/com.gaze.wishes/` |
| macOS | `~/Library/Application Support/com.gaze.wishes/` |
| Windows | `%APPDATA%\com.gaze.wishes\` |

开发环境下，只要项目根目录的 `data/` 存在且通过校验，就会直接使用它，不会读取用户数据目录。

首次启动（生产环境）时，如果 `<app_data_dir>/data` 不存在，程序会从安装包资源目录复制默认数据；`version.json` 缺失时会自动补写。

## 二、data/ 顶层结构

```text
data/
├── version.json          # 数据格式版本
├── official/             # 官方数据（只读）
├── local/                # 用户数据（可写）
└── packs/                # 拓展包（只读）
    └── <pack-name>/
        └── pack.zip      # 原始压缩包备份，不参与运行时
```

`version.json` 记录整数格式版本：

```json
{
  "format_version": 2
}
```

程序启动时会读取并校验该文件。版本号与当前实现不一致时**直接拒绝启动**，不会尝试猜测或自动迁移。迁移脚本位于 `scripts/`。

## 三、来源目录

`official/`、`local/` 与每个拓展包目录拥有相同的内部结构：

```text
<source>/
├── cards/                # 卡片
├── decks/                # 卡组
├── logics/               # 抽卡逻辑定义（无资产）
├── banners/              # 卡池
└── assets/
    ├── cards/
    ├── decks/
    ├── banners/
    └── logics/           # 预留，当前未使用
```

| 来源 | 目录 | 运行时是否可写 |
| :--- | :--- | :---: |
| `official` | `data/official/` | 否 |
| `local` | `data/local/` | 是 |
| `pack:<name>` | `data/packs/<name>/` | 否 |

- 官方数据与拓展包数据**永远不会被原地修改**。
- 用户对只读对象的编辑会产生一个 `local` 派生对象，详见 [来源与分叉](concepts.md#三来源与分叉)。
- 对象内部的 `origin` 字段必须与所处目录一致。

## 四、对象文件

### 4.1 命名规则

- 对象文件：`{global_id}.json`
- 资产文件：`{global_id}.{ext}`；同一对象的多个资产用后缀区分，如 `{global_id}_thumb.webp`
- **不按标签值划分子目录**，避免标签值参与路径拼接带来的穿越风险。

`global_id` 是 UUID v7 字符串，也是跨实例的唯一标识；文件名与文件内的 `global_id` 必须一致。

### 4.2 通用字段

四类对象共享以下外层字段：

| 字段 | 类型 | 说明 |
| :--- | :--- | :--- |
| `global_id` | string | UUID v7，与文件名一致 |
| `origin` | string | `official` / `local` / `pack:<name>` |
| `forked_from` | string? | 派生对象指向的原对象 `global_id`，非派生对象省略 |
| `shadowed` | bool? | 派生对象是否遮蔽原对象，默认 `true`；非派生对象省略 |
| `tags` | array | 标签数组，见下 |

v2 的标签仍为 `namespace` + `value` 结构：

```json
{ "namespace": "game", "value": "genshin" }
```

> 标签 Id 化会在后续数据格式 v3 中实施，届时标签引用将改为 `TagId`。

### 4.3 卡片 `cards/{global_id}.json`

```json
{
  "global_id": "01a11038-b4a1-7414-aa0a-70e3452001c5",
  "origin": "official",
  "content": { "zh-CN": "乐圮" },
  "title": { "zh-CN": "乐圮" },
  "assets": {
    "portrait": "cards/01a11038-b4a1-7414-aa0a-70e3452001c5.png",
    "thumbnail": "cards/01a11038-b4a1-7414-aa0a-70e3452001c5_thumb.webp",
    "animation": null
  },
  "tags": [
    { "namespace": "game", "value": "starrail" },
    { "namespace": "rarity", "value": "3" }
  ]
}
```

| 字段 | 必填 | 说明 |
| :--- | :---: | :--- |
| `content` | ✓ | 卡片名称，多语言映射 |
| `title` | | 称号 / 副标题，多语言映射 |
| `assets` | | `portrait` / `thumbnail` / `animation` |
| `tags` | ✓ | 描述卡片属性的标签 |

### 4.4 卡组 `decks/{global_id}.json`

```json
{
  "global_id": "01a0fbfa-c5c7-74ef-86f0-461f625d55c1",
  "origin": "official",
  "name": { "zh-CN": "原神胡桃 UP 卡组" },
  "assets": { "cover": "decks/01a0fbfa-c5c7-74ef-86f0-461f625d55c1.png" },
  "members": {
    "conditions": [
      { "type": "tag_all", "tags": [{ "namespace": "game", "value": "genshin" }] },
      { "type": "include_ids", "ids": ["01a11038-b616-7566-ac7d-cf01ed7f7fd2"] }
    ]
  },
  "event_groups": {
    "up": {
      "conditions": [{ "type": "tag_all", "tags": [{ "namespace": "fate", "value": "up" }] }]
    }
  },
  "tags": [{ "namespace": "game", "value": "genshin" }]
}
```

| 字段 | 必填 | 说明 |
| :--- | :---: | :--- |
| `name` | ✓ | 卡组名称，多语言映射 |
| `assets` | | `cover` |
| `members` | ✓ | 成员规则，按条件递归筛选卡片 |
| `event_groups` | | 活动标签分组：`EventTag` → 该分组的成员规则 |
| `tags` | | 卡组自身的描述标签 |

成员条件与活动组的完整语义见 [核心概念 · 卡组](concepts.md#五卡组)。

### 4.5 逻辑 `logics/{global_id}.json`

```json
{
  "global_id": "01a0fbfa-c5ca-7cb2-992a-71ef114f7390",
  "origin": "official",
  "name": { "zh-CN": "原神角色 UP 池逻辑" },
  "variant": {
    "variant": "Hardcoded",
    "config": { "executor_name": "genshin_character_up" }
  }
}
```

| 字段 | 必填 | 说明 |
| :--- | :---: | :--- |
| `name` | ✓ | 逻辑名称，多语言映射 |
| `variant` | ✓ | 逻辑实现方式；当前内置逻辑为 `Hardcoded` |

逻辑只描述"这一抽命中什么标签"，不直接引用卡片。

### 4.6 卡池 `banners/{global_id}.json`

```json
{
  "global_id": "01a0fbfa-c5cc-725f-b912-de327d361861",
  "origin": "official",
  "name": { "zh-CN": "赤团开时" },
  "assets": {
    "background": "banners/01a0fbfa-c5cc-725f-b912-de327d361861.png",
    "icon": "banners/01a0fbfa-c5cc-725f-b912-de327d361861_icon.png",
    "carousel_cards": ["01a11038-b4a1-7414-aa0a-70e3452001c5"]
  },
  "deck_id": "01a0fbfa-c5c7-74ef-86f0-461f625d55c1",
  "logic_id": "01a0fbfa-c5ca-7cb2-992a-71ef114f7390",
  "state_scope": "banner",
  "tags": [
    { "namespace": "game", "value": "genshin" },
    { "namespace": "event", "value": "up" }
  ]
}
```

| 字段 | 必填 | 说明 |
| :--- | :---: | :--- |
| `name` | ✓ | 卡池名称，多语言映射 |
| `assets` | | `background` / `icon` / `carousel_cards` |
| `deck_id` | ✓ | 关联卡组 `global_id` |
| `logic_id` | ✓ | 关联逻辑 `global_id` |
| `state_scope` | ✓ | `banner`（独立状态）或 `group:<name>`（共享状态） |
| `tags` | | 卡池描述标签 |

## 五、资产

- `assets` 中的路径**相对于所属来源的 `assets/` 目录**。
- 例如 `"cards/xxx.png"` 对应：
  - 官方卡片：`data/official/assets/cards/xxx.png`
  - 用户卡片：`data/local/assets/cards/xxx.png`
  - 拓展包卡片：`data/packs/<name>/assets/cards/xxx.png`
- 加载器**不检查**资产文件是否存在，避免启动时产生大量文件 I/O；资产缺失只在界面显示时暴露。
- 资产目录会在启动时由 `ensure_layout` 自动创建。

## 六、运行时数据库 `db/`

运行时数据不写入 `data/`。`db/` 下有两个独立的 SQLite 文件：

### 6.1 `states.db` — 卡池运行时状态

表 `instance_states`：

| 列 | 类型 | 说明 |
| :--- | :--- | :--- |
| `scope_key` | TEXT | 主键，`banner:<global_id>` 或 `group:<name>` |
| `user_id` | INTEGER | 当前固定为 `0`（单用户模式） |
| `total_counter` | INTEGER | 该作用域的总抽数 |
| `logic_state` | TEXT? | 逻辑引擎的持久化状态（JSON 字符串） |

状态以 `scope_key` 为键，因此使用同一 `group:<name>` 的多个卡池会共享同一份进度。首次抽卡时若不存在记录，会以默认状态（计数器 0、逻辑状态为空）开始。

### 6.2 `overrides.db` — 引用覆盖层

遮蔽产生的引用重定向不会写回 JSON，而是保存在此数据库中：

| 表 | 状态 | 说明 |
| :--- | :--- | :--- |
| `card_ref_overrides` | 启用 | `from_id -> to_id`，卡组引用解析时重定向 |
| `deck_ref_overrides` | 预留 | 卡池对卡组的引用覆盖（随 Banner CRUD 启用） |
| `logic_ref_overrides` | 预留 | 卡池对逻辑的引用覆盖（随 Logic CRUD 启用） |

每张表只有 `from_id`（主键）与 `to_id` 两列。

### 6.3 重置运行时数据

删除 `db/` 目录会同时清空所有卡池进度与引用覆盖，但不会影响 `data/` 中的卡片、卡组与卡池定义。程序下次启动时会重新创建数据库。

## 七、启动校验与目录创建

程序启动时按以下顺序准备目录：

1. 确定数据目录（开发环境优先使用项目 `data/`，生产环境使用 `app_data_dir/data`）。
2. 复制默认数据（仅生产环境首次启动）。
3. 补写缺失的 `version.json`。
4. `DataPaths::verify` 校验：
   - 数据目录存在；
   - `version.json` 可读且版本受支持；
   - `official/` 与 `local/` 存在。
5. `ensure_layout` 创建缺失的子目录（`official` / `local` / `packs` 及各类对象、资产目录）。

注意：`verify` 只要求 `official/` 与 `local/` 存在，`packs/` 由 `ensure_layout` 补齐。

## 八、备份与迁移

- **备份**：直接复制整个 `data/` 目录即可；`db/` 保存的是运行时进度，可按需一并备份。
- **手工编辑**：`local/` 下的 JSON 可以手工修改，修改后重启程序生效。官方目录与拓展包目录不建议手工改动。
- **格式升级**：数据格式变更时版本号自增，旧数据不会被自动迁移。使用 `scripts/` 下的迁移脚本，或按新格式重写数据。
- **重置用户数据**：退出程序后删除 `data/local/`（保留 `official/`）可清除全部用户创作；删除整个 `data/` 并重启会重新复制默认官方数据。

## 九、相关源码

| 文件 | 职责 |
| :--- | :--- |
| `src-tauri/src/utils/data_paths.rs` | 目录布局、路径拼装、布局校验与创建 |
| `src-tauri/src/utils/path.rs` | 开发 / 生产环境目录选择与默认数据复制 |
| `src-tauri/src/domain/version.rs` | `version.json` 的读取与版本校验 |
| `src-tauri/src/app/loader.rs` | 从来源目录加载四类对象 |
| `src-tauri/src/infrastructure/repository/` | `states.db` 与 `overrides.db` |
