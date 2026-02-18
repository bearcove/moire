export const SCOPE_COLOR_HUES = [208, 158, 34, 276, 18, 124, 332, 248, 54, 188, 14, 300] as const;

export function hashString(value: string): number {
  let h = 0;
  for (let i = 0; i < value.length; i++) {
    h = (h * 31 + value.charCodeAt(i)) >>> 0;
  }
  return h;
}

export function scopeHueForKey(scopeKey: string): number {
  return SCOPE_COLOR_HUES[hashString(scopeKey) % SCOPE_COLOR_HUES.length];
}
