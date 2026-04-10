export function formatTrendLabelByWindow(unixSeconds: number, windowDays: number): string {
  const date = new Date(unixSeconds * 1000);
  if (windowDays <= 7) {
    return date.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });
  }
  return date.toLocaleDateString([], { month: "numeric", day: "numeric" });
}

export function getTrendTickLimit(windowDays: number): number {
  return windowDays <= 7 ? 12 : 10;
}

export function buildWindowedDailyLabel(dayLabel: string, windowDays: number): string {
  if (windowDays <= 7) {
    return dayLabel;
  }
  return dayLabel;
}
