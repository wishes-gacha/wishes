import { invoke } from "@tauri-apps/api/core";
import { CardAssets, LocalizedString, Origin, Tag } from "../types";

/**
 * 卡片信息
 */
export interface CardSummary {
  global_id: string,
  origin: Origin,
  forked_from?: string,
  forked_from_name?: LocalizedString,
  derived_id?: string,
  shadowed: boolean,
  content: LocalizedString,
  title?: LocalizedString,
  assets?: CardAssets,
  tags: Tag[],
}

/**
 * 卡片创建请求
 */
export interface CardCreateRequest {
  content: LocalizedString,
  title?: LocalizedString,
  tags: Tag[],
}

/**
 * 卡片更新请求
 */
export interface CardUpdateRequest {
  global_id: string,
  content: LocalizedString,
  title?: LocalizedString,
  tags: Tag[],
  shadow?: boolean,
}

/**
 * 显式引用某张卡片的卡组 (用于删除前提示)
 */
export interface DeckReference {
  global_id: string,
  name: LocalizedString,
}

export async function listCards(): Promise<CardSummary[]> {
  return await invoke<CardSummary[]>("list_cards");
}

export async function listCardsByTags(tags: Tag[]): Promise<CardSummary[]> {
  return await invoke<CardSummary[]>("list_cards_by_tags", { tags });
}

/**
 * 新建卡片
 */
export async function createCard(req: CardCreateRequest): Promise<CardSummary> {
  return await invoke<CardSummary>("create_card", { req });
}

/**
 * 更新卡片
 *
 * 若目标卡片来自只读来源 (官方 / 拓展包), 后端会**分叉**出一个本地副本:
 * 返回值的 `global_id` 与传入的不同, 且 `forked_from` 指向原卡片.
 */
export async function updateCard(req: CardUpdateRequest): Promise<CardSummary> {
  return await invoke<CardSummary>("update_card", { req });
}

export async function deleteCard(globalId: string): Promise<void> {
  return await invoke("delete_card", { globalId });
}

export async function setCardShadow(globalId: string, shadowed: boolean): Promise<CardSummary> {
  return await invoke("set_card_shadow", { globalId, shadowed });
}

export async function getCardReferencingDecks(globalId: string): Promise<DeckReference[]> {
  return await invoke<DeckReference[]>("get_card_referencing_decks", { globalId });
}