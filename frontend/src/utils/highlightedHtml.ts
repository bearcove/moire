/** Strip all HTML tags to get plain text. */
export function stripHtmlTags(html: string): string {
  return html.replace(/<[^>]*>/g, "");
}

/** Check if a highlighted HTML line is a context cut marker (the "slash-star ... star-slash" placeholder). */
export function isContextCutMarker(htmlLine: string): boolean {
  const plain = stripHtmlTags(htmlLine).trim();
  return plain === "/* ... */";
}

export interface CollapsedLine {
  lineNum: number;
  html: string;
  isSeparator: boolean;
}

/**
 * Collapse cut marker lines (and their following empty lines) into single separator entries.
 * Input: per-line HTML strings from splitHighlightedHtml applied to context_html.
 * startLineNum: the 1-based line number of the first line in `lines`.
 */
export function collapseContextLines(
  lines: string[],
  startLineNum: number,
): CollapsedLine[] {
  const result: CollapsedLine[] = [];
  let i = 0;
  while (i < lines.length) {
    const lineNum = startLineNum + i;
    if (isContextCutMarker(lines[i])) {
      result.push({ lineNum, html: "", isSeparator: true });
      i++;
      // Skip following empty lines (part of the same cut region)
      while (i < lines.length && stripHtmlTags(lines[i]).trim() === "") {
        i++;
      }
    } else {
      result.push({ lineNum, html: lines[i], isSeparator: false });
      i++;
    }
  }
  return result;
}

/**
 * Split arborium-highlighted HTML into per-line strings while preserving
 * tag nesting balance across line boundaries.
 *
 * Arborium produces a flat HTML string where inline elements (`<a-k>`,
 * `<a-f>`, etc.) can span multiple lines. This function splits at `\n`
 * characters and reopens/closes any tags that straddle a line break.
 */
export function splitHighlightedHtml(html: string): string[] {
  const parser = new DOMParser();
  const doc = parser.parseFromString(`<div>${html}</div>`, "text/html");
  const container = doc.body.firstChild;
  const lines: string[] = [];
  let currentLine = "";
  const openTags: { tag: string; attrs: string }[] = [];

  function processNode(node: Node) {
    if (node.nodeType === Node.TEXT_NODE) {
      const text = node.textContent ?? "";
      for (const char of text) {
        if (char === "\n") {
          for (let j = openTags.length - 1; j >= 0; j--) {
            currentLine += `</${openTags[j].tag}>`;
          }
          lines.push(currentLine);
          currentLine = "";
          for (const t of openTags) {
            currentLine += `<${t.tag}${t.attrs}>`;
          }
        } else {
          currentLine +=
            char === "<" ? "&lt;" : char === ">" ? "&gt;" : char === "&" ? "&amp;" : char;
        }
      }
    } else if (node.nodeType === Node.ELEMENT_NODE) {
      const el = node as Element;
      const tag = el.tagName.toLowerCase();
      let attrs = "";
      for (const attr of el.attributes) {
        attrs += ` ${attr.name}="${attr.value.replace(/"/g, "&quot;")}"`;
      }
      currentLine += `<${tag}${attrs}>`;
      openTags.push({ tag, attrs });
      for (const child of el.childNodes) {
        processNode(child);
      }
      openTags.pop();
      currentLine += `</${tag}>`;
    }
  }

  if (container) {
    for (const child of container.childNodes) {
      processNode(child);
    }
  }
  if (currentLine) {
    lines.push(currentLine);
  }
  return lines;
}
