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
