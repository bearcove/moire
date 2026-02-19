import React from "react";
import { FileRs } from "@phosphor-icons/react";
import { NodeChip } from "../../ui/primitives/NodeChip";

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
    <NodeChip
      icon={<FileRs size={12} weight="bold" />}
      label={shortSource(source)}
      href={`zed://file${source}`}
      title={`Open ${source} in Zed`}
    />
  );
}
