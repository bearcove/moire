import React from "react";

function shortSource(source: string): string {
  const match = source.match(/^(.*):(\d+)$/);
  if (!match) {
    return source.split("/").pop() ?? source;
  }

  const [, path, line] = match;
  const file = path.split("/").pop() ?? path;
  return `${file}:${line}`;
}

export function Source({ source }: { source: string }) {
  return (
    <a className="inspector-source-link" href={`zed://file${source}`} title="Open in Zed">
      {shortSource(source)}
    </a>
  );
}
