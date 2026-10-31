/**
 * 全局共享的接口类型, 与后端 `domain` / `interface` 层对应.
 *
 * 数据格式 v2 起:
 * - 所有对象的标识字段为 `global_id` (Uuid 字符串), 不再有数字 Id.
 * - 所有面向用户的文本为 `LocalizedString` (语言标签 -> 文本), 由前端按当前语言回退解析.
 */

/**
 * 标签类型, 与后端 `Tag` 结构对应
 */
export interface Tag {
  namespace: string,
  value: string,
}

/**
 * EventTag 为 string 的活动标签语义化别名
 */
export type EventTag = string;

/**
 * 多语言字符串: BCP 47 语言标签 -> 文本.
 * 
 * 后端保证不存储空字符串, 且至少有一种语言非空.
 */
export type LocalizedString = Record<string, string>;

/**
 * 对象来源
 * 
 * - `official`: 官方数据 (只读)
 * - `local`: 用户本地数据
 * - `pack:<name>`: 拓展包数据 (只读)
 */
export type Origin = string;

/**
 * 卡片资产引用 (相对于所属来源的 `assets/` 目录)
 */
export interface CardAssets {
  portrait?: string,
  thumbnail?: string,
  animation?: string,
}

/**
 * 卡组资产引用
 */
export interface DeckAssets {
  cover?: string,
}

/**
 * 卡池资产引用
 * 
 * `carousel_cards` 为卡片轮播列表, 存储卡片 `Uuid`
 */
export interface BannerAssets {
  background?: string,
  icon?: string,
  carousel_cards?: string[],
}

/**
 * 单次抽卡响应, 包含卡片内容和标签信息
 */
export interface WishResponse {
  global_id: string
  content: LocalizedString,
  title?: LocalizedString,
  tags: Tag[],
  event_tags: EventTag[],
}

// TODO[2026-10-05]: 此处的卡池接口类型在 Banner CRUD 实现时需要移动至 api/banners.ts

/**
 * 卡池摘要信息, 用于列表展示
 */
export interface BannerSummary {
  global_id: string,
  origin: Origin,
  forked_from?: string,
  name: LocalizedString,
  tags: Tag[],
}

/**
 * 卡池详细信息, 包含关联资源和抽卡统计
 */
export interface BannerInfo {
  global_id: string,
  name: LocalizedString,
  tags: Tag[],
  deck_id: string,
  logic_id: string,
  deck_name: LocalizedString,
  logic_name: LocalizedString,
  /** 运行时状态作用域: `"banner"` 或 `"group:<name>"` */
  state_scope: string,
  total_counter: number,
}

/**
 * 图鉴统计信息
 */
export interface CatalogStats {
  cards: number,
  decks: number,
  logics: number,
}