import React from "react";
import type { SourcePreviewResponse } from "../../api/types.generated";
import { splitHighlightedHtml, collapseContextLines } from "../../utils/highlightedHtml";
import "./SourcePreview.css";

const CONTEXT_LINES = 4;

export function SourcePreview({ preview }: { preview: SourcePreviewResponse }) {
  const { target_line } = preview;

  // Prefer context_html (scope excerpt with cuts) when available
  const useContext = preview.context_html != null && preview.context_range != null;

  let entries: Array<{ lineNum: number; html: string; isSeparator: boolean }>;

  if (useContext) {
    const ctxLines = splitHighlightedHtml(preview.context_html!);
    entries = collapseContextLines(ctxLines, preview.context_range!.start);
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
