# v1 → v2 数据迁移脚本

`migrate_v1_to_v2.py` 将 Wishes v1 的数据目录与状态数据库迁移为 v2 格式。脚本只读取输入目录，所有结果写入新的输出目录，**不会修改原始数据**。

支持以下 v1 数据：

- `v0.1.0` / `v0.1.1`：卡组使用静态成员 Id 列表，卡池状态内联在 `logic_instance.state` 中；
- `v0.1.2`：卡组使用 `conditions` 规则，卡池状态内联；
- v1 `main`（v0.2.0 之前）：卡组使用 `conditions` 规则，状态存放在 SQLite `instance_states` 中。

## 环境要求

- Python 3.10+
- 仅使用标准库，无需安装依赖
- v2 官方数据（`data/official/`），用于匹配官方对象与复制到输出目录

## 快速开始

```bash
python scripts/migrate_v1_to_v2.py \
  --data-dir /path/to/v1/data \
  --db /path/to/v1/db/states.db \
  --official-data /path/to/v2/data/official \
  --out-data /path/to/data_v2 \
  --out-db /path/to/db_v2/states.db
```

参数说明：

| 参数 | 默认值 | 说明 |
| :--- | :--- | :--- |
| `--data-dir` | 必填 | v1 数据目录 |
| `--db` | `<data-dir>/../db/states.db`（若存在） | v1 状态数据库；早期版本没有数据库时可省略 |
| `--official-data` | 仓库根目录的 `data/official` | v2 官方数据目录，或包含 `official/` 的数据根目录 |
| `--out-data` | `<data-dir>/../data_v2` | 输出 v2 数据目录 |
| `--out-db` | `<data-dir>/../db_v2/states.db` | 输出 v2 状态数据库 |
| `--report` | `<out-data>/migration-report.json` | 迁移报告路径 |
| `--tag-map` | 无 | 额外的标签映射 JSON，例如 `{"game:星穹铁道": "game:starrail"}` |
| `--v1-official-manifest` | `scripts/v1_official_manifest.json` | v1 官方默认对象清单 |
| `--no-v1-manifest` | 关闭 | 忽略官方清单，把与 v2 官方数据不同的对象一律视为用户修改 |
| `--dry-run` | 关闭 | 只分析并打印摘要，不写任何文件 |
| `--force` | 关闭 | 允许覆盖已存在的输出目录/数据库 |
| `--strict` | 关闭 | 存在警告时返回退出码 2 |

## 迁移行为

1. **官方对象匹配**
   - `v1_official_manifest.json` 记录了 v0.1.0 / v0.1.1 / v0.1.2 / v1 main 四个版本的官方默认对象签名。
   - 与清单完全一致的对象视为「未修改的官方数据」，映射到当前 v2 官方对象。即使 v2 官方数据更新了内容（例如某个卡组的 UP 名单变化），也采用新版官方数据。
   - 不在清单中、但与 v2 官方对象同名/同内容的，视为用户修改：生成 `local` 派生对象，`forked_from` 指向官方对象，默认遮蔽原对象。
   - 完全匹配不到官方对象的，作为用户自建对象写入 `data/local/`。
2. **标签映射**
   - 已知的中文标签值（游戏、属性、命途、职业、活动标签）会映射为 v2 的英文标识符。
   - 映射会从 v2 官方数据自动补充；未识别的自定义标签会原样保留并记录警告。
   - 内置映射优先，避免早期 v1 默认数据中的历史错误覆盖正确映射（例如 v0.1.0 把流萤的属性写成了「风」）。
3. **卡组格式**
   - v0.1.0/v0.1.1 的静态成员列表会转换为 v2 的 `include_ids` 条件；
   - v0.1.2/main 的 `conditions` 规则直接转换，并重写 `include_ids` / `exclude_ids` 中的卡片 Id。
4. **卡池状态**
   - v1 SQLite `instance_states`（`user_id + banner_id`）迁移为 v2 的 `scope_key` 结构（`banner:<uuid>`），保留 `total_counter` 与 `logic_state`。
   - 早期版本内联在 `Banner.logic_instance.state` 中的状态会被提取并写入 v2 数据库；`total_counter` 单独存放，其余字段作为 `logic_state`。
   - 只迁移 `user_id = 0` 的状态；v2 目前是单用户模式。
5. **输出**
   - 复制 v2 `official/` 到输出目录；
   - 新建 `local/{cards,decks,logics,banners}` 与空 `packs/`；
   - 写入 `version.json`（`format_version: 2`）；
   - 写入迁移报告，包含每个对象的旧 Id → 新 `global_id`、状态与警告。

## 使用迁移结果

迁移完成后会得到：

```text
data_v2/
├── version.json
├── official/          # 从 v2 官方数据复制
├── local/             # 迁移后的用户数据 / 派生对象
└── packs/
db_v2/
└── states.db          # v2 状态数据库
```

可以将 `data_v2` 作为 v2 应用的数据目录；或把 `data_v2/local/` 合并到应用数据目录的 `local/`，并把 `db_v2/states.db` 放到应用的 `db/states.db` 位置。请确保 `official/` 与所运行的 v2 版本一致。

> [!IMPORTANT]
>
> 迁移前仍建议备份 `data/` 与 `db/`。脚本不会修改输入，但覆盖旧数据、切换应用数据目录等操作不可逆。

## 测试

```bash
python -m unittest discover -s scripts/tests -v
```

CI 会在每次 PR 中运行该测试。

## 已知限制

- 只迁移 `user_id = 0` 的状态；多用户隔离尚未实现。
- 未识别的自定义标签不会自动翻译，会保留原值并产生警告。
- v1 的 `data/assets/` 若存在会被复制到 `local/assets/`，但 v1 卡片没有资产引用，需要手动接入。
- 官方默认对象依赖 `v1_official_manifest.json`；如果清单缺失或使用了 `--no-v1-manifest`，与 v2 官方数据不同的对象都会保留为本地派生。
