import { invoke } from "@tauri-apps/api/core";
import { DeckAssets, EventTag, LocalizedString, Origin, Tag } from "../types";

/**
 * 卡片引用一律使用目标 Card 的 `global_id` (Uuid 字符串)
 */
export type MembershipCondition =
  | { type: "tag_all", tags: Tag[] }
  | { type: "tag_any", tags: Tag[] }
  | { type: "filter_tag_all", tags: Tag[] }
  | { type: "filter_tag_any", tags: Tag[] }
  | { type: "include_ids", ids: string[] }
  | { type: "exclude_ids", ids: string[] };

export interface Membership {
  conditions: MembershipCondition[],
}

export type EventGroupCondition =
  | { type: "all" }
  | { type: "tag_all", tags: Tag[] }
  | { type: "tag_any", tags: Tag[] }
  | { type: "filter_tag_all", tags: Tag[] }
  | { type: "filter_tag_any", tags: Tag[] }
  | { type: "include_ids", ids: string[] }
  | { type: "exclude_ids", ids: string[] }
  // 以下规则未正式实现, 但保留声明
  | { type: "include_groups", groups: EventTag[] }
  | { type: "exclude_groups", groups: EventTag[] };

export interface EventGroup {
  conditions: EventGroupCondition[],
}

export interface DeckSummary {
  global_id: string,
  origin: Origin,
  forked_from?: string,
  forked_from_name?: LocalizedString,
  name: LocalizedString,
  assets?: DeckAssets,
  members: Membership,
  event_groups: Record<EventTag, EventGroup>
  tags: Tag[],
}

export interface CreateDeckRequest {
  name: LocalizedString,
  members: Membership,
  event_groups: Record<EventTag, EventGroup>,
  tags: Tag[],
}

export interface UpdateDeckRequest {
  global_id: string,
  name?: LocalizedString,
  members?: Membership,
  event_groups?: Record<EventTag, EventGroup>,
  tags?: Tag[],
}

export type EditableCondition = MembershipCondition | EventGroupCondition;

export async function listDecks(): Promise<DeckSummary[]> {
  return await invoke("list_decks");
}

/**
 * 新建卡组
 */
export async function createDeck(req: CreateDeckRequest): Promise<DeckSummary> {
  return await invoke("create_deck", { req })
}

/**
 * 更新卡组
 * 
 * 若目标卡组来自只读来源 (官方 / 拓展包), 后端会**分叉**出一个本地副本;
 * 引用该卡组的卡池不会被自动切换, 需要提示用户.
 */
export async function updateDeck(req: UpdateDeckRequest): Promise<DeckSummary> {
  return await invoke('update_deck', { req });
}

export async function deleteDeck(globalId: string): Promise<void> {
  return await invoke('delete_deck', { globalId });
}