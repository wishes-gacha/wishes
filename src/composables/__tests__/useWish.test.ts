import { invoke } from "@tauri-apps/api/core";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { useWish } from "../useWish";


vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

// 单测不处于组件 setup 上下文, mock 掉 i18n (仅测试文案拼装行为)
vi.mock("vue-i18n", () => ({
  useI18n: () => ({
    t: (key: string, params?: Record<string, unknown>) =>
      params ? `${key} ${Object.values(params).map(String).join(" ")}` : key,
  }),
}));

describe("useWish", () => {
  const bannerId = "0192a3c4-1234-7abc-8def-000000000001";
  const cardId = (index: number) =>
    `0192a3c4-1234-7abc-8def-0000000001${String(index).padStart(2, "0")}`;

  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("should handle successful single wish", async () => {
    const mockResponse = {
      global_id: cardId(1),
      content: { "zh-CN": "Test-Card" },
      tags: [],
      event_tags: [],
    };
    (invoke as any).mockResolvedValue(mockResponse);

    const { doWish, currentWish, loading, history } = useWish();
    const promise = doWish(bannerId);

    expect(loading.value).toBe(true);

    await promise;

    expect(loading.value).toBe(false);
    expect(currentWish.value).toEqual(mockResponse);
    expect(history.value).toContainEqual(mockResponse);
  });

  it("should handle error when wish fails", async () => {
    (invoke as any).mockRejectedValue(new Error("Test error"));

    const { doWish, loading, error } = useWish();

    await expect(doWish(bannerId)).rejects.toThrow("Test error");

    expect(loading.value).toBe(false);
    expect(error.value).toContain("Test error");
  });

  it("should handle successful ten wish via batch command", async () => {
    const mockResults = Array.from({ length: 10 }, (_, i) => ({
      global_id: cardId(i),
      content: { "zh-CN": `Test-Card-${i}` },
      tags: [],
      event_tags: [],
    }));
    (invoke as any).mockResolvedValue(mockResults);

    const { doWishTen, loading, history } = useWish();
    const results = await doWishTen(bannerId);

    expect(invoke).toHaveBeenCalledWith("wish_ten", { bannerId });
    expect(results).toHaveLength(10);
    expect(loading.value).toBe(false);
    expect(history.value[0]).toEqual(mockResults[0]);
  });

  it("should surface a localized error when ten wish fails", async () => {
    (invoke as any).mockRejectedValue(new Error("Test error"));

    const { doWishTen, loading, error } = useWish();

    await expect(doWishTen(bannerId)).rejects.toThrow("Test error");

    expect(loading.value).toBe(false);
    expect(error.value).toContain("wish.drawFailed");
  });
});
