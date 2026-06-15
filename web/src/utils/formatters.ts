/**
 * 格式化 JSON 字符串
 * @param value - 原始 JSON 字符串
 * @returns 格式化后的 JSON 字符串（2 空格缩进）
 * @throws 如果 JSON 格式无效
 */
export function formatJSON(value: string): string {
  const trimmed = value.trim();
  if (!trimmed) {
    return "";
  }
  const parsed = JSON.parse(trimmed);
  return JSON.stringify(parsed, null, 2);
}

const USD_TO_CNY = 7.2;

export function formatLocalizedCost(usd: number, language?: string): string {
  if (!Number.isFinite(usd)) return "—";

  if (language?.toLowerCase().startsWith("zh")) {
    return new Intl.NumberFormat("zh-CN", {
      style: "currency",
      currency: "CNY",
      minimumFractionDigits: 4,
      maximumFractionDigits: 4,
    }).format(usd * USD_TO_CNY);
  }

  return new Intl.NumberFormat("en-US", {
    style: "currency",
    currency: "USD",
    minimumFractionDigits: 4,
    maximumFractionDigits: 4,
  }).format(usd);
}
