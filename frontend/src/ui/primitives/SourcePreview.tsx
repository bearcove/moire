import React from "react";
import type { SourceContextLine, SourcePreviewResponse } from "../../api/types.generated";
import { splitHighlightedHtml } from "../../utils/highlightedHtml";
import "./SourcePreview.css";

const CONTEXT_LINES = 4;

type Entry = { lineNum: number; html: string; isSeparator: boolean; separatorIndentCols?: number };

function contextLinesToEntries(lines: SourceContextLine[]): Entry[] {
  return lines.map((line) => {
    if ("separator" in line) {
      return { lineNum: 0, html: "", isSeparator: true, separatorIndentCols: line.separator.indent_cols };
    }
    return { lineNum: line.line.line_num, html: line.line.html, isSeparator: false };
  });
}

export function SourcePreview({ preview }: { preview: SourcePreviewResponse }) {
  const { target_line } = preview;

  let entries: Entry[];

  if (preview.context_lines != null) {
    entries = contextLinesToEntries(preview.context_lines);
  } else {
    // Fallback: window into full file HTML
    const lines = splitHighlightedHtml(preview.html);
    const start = Math.max(0, target_line - 1 - CONTEXT_LINES);
    const end = Math.min(lines.length, target_line + CONTEXT_LINES);
    const slice = lines.slice(start, end);
    const firstLineNum = start + 1;
    entries = slice.map((html, i) => ({
      lineNum: firstLineNum + i,
      html,
      isSeparator: false,
    }));
  }

  return (
    <div className="ui-source-preview arborium-hl">
      <pre className="ui-source-preview__code">
        {entries.map((entry) => {
          if (entry.isSeparator) {
            return (
              <div key={`sep-${entry.lineNum}`} className="ui-source-preview__sep">
                <span className="ui-source-preview__gutter">
                  <span className="ui-source-preview__ln" />
                  <span className="ui-source-preview__ribbon" />
                </span>
                <span className="ui-source-preview__sep-label">⋯</span>
              </div>
            );
          }
          const isTarget = entry.lineNum === target_line;
          return (
            <div
              key={entry.lineNum}
              className={`ui-source-preview__line${isTarget ? " ui-source-preview__line--target" : ""}`}
            >
              <span className="ui-source-preview__gutter">
                <span className="ui-source-preview__ln">{entry.lineNum}</span>
                <span className="ui-source-preview__ribbon" />
              </span>
              {/* eslint-disable-next-line react/no-danger */}
              <span className="ui-source-preview__text" dangerouslySetInnerHTML={{ __html: entry.html }} />
            </div>
          );
        })}
      </pre>
    </div>
  );
}
