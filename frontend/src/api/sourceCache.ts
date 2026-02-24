import type { SourcePreviewResponse } from "./types.generated";
import { apiClient } from "./index";
import { splitHighlightedHtml, isContextCutMarker } from "../utils/highlightedHtml";

/** In-flight / resolved promise cache: frameId → promise. Survives unmount/remount. */
const sourcePreviewCache = new Map<number, Promise<SourcePreviewResponse>>();

/** Resolved preview cache: frameId → response, populated when the promise settles. */
const resolvedPreviewCache = new Map<number, SourcePreviewResponse>();

/** Resolved single-line cache: frameId → extracted highlighted HTML line. */
const resolvedLineCache = new Map<number, string>();

function extractLineFromPreview(res: SourcePreviewResponse): string | undefined {
  if (res.context_html && res.context_range) {
    const ctxLines = splitHighlightedHtml(res.context_html);
    // target_line is 1-based, context_range.start is 1-based
    const targetIdx = res.target_line - res.context_range.start;
    if (targetIdx >= 0 && targetIdx < ctxLines.length) {
      const line = ctxLines[targetIdx]?.trim();
      if (line && !isContextCutMarker(line)) return line;
    }
    // If target line was cut or empty, scan nearby for content
    for (let d = 1; d <= 3; d++) {
      for (const offset of [targetIdx - d, targetIdx + d]) {
        if (offset >= 0 && offset < ctxLines.length) {
          const candidate = ctxLines[offset]?.trim();
          if (candidate && !isContextCutMarker(candidate)) return candidate;
        }
      }
    }
  }
  // Fallback to full file
  const lines = splitHighlightedHtml(res.html);
  const targetIdx = res.target_line - 1;
  return targetIdx >= 0 && targetIdx < lines.length ? lines[targetIdx]?.trim() : undefined;
}

export function cachedFetchSourcePreview(frameId: number): Promise<SourcePreviewResponse> {
  let cached = sourcePreviewCache.get(frameId);
  if (!cached) {
    cached = apiClient.fetchSourcePreview(frameId).then((res) => {
      resolvedPreviewCache.set(frameId, res);
      const line = extractLineFromPreview(res);
      if (line) resolvedLineCache.set(frameId, line);
      return res;
    });
    cached.catch(() => sourcePreviewCache.delete(frameId));
    sourcePreviewCache.set(frameId, cached);
  }
  return cached;
}

export function getSourcePreviewSync(frameId: number): SourcePreviewResponse | undefined {
  return resolvedPreviewCache.get(frameId);
}

export function getSourceLineSync(frameId: number): string | undefined {
  return resolvedLineCache.get(frameId);
}
