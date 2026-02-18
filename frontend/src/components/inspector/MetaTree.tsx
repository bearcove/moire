import React, { useState } from "react";
import { CaretDown, CopySimple } from "@phosphor-icons/react";
import { ActionButton } from "../../ui/primitives/ActionButton";
import type { MetaValue } from "../../snapshot";
import "./MetaTree.css";
import "./InspectorPanel.css";

export function MetaTreeNode({
  name,
  value,
  depth = 0,
}: {
  name: string;
  value: MetaValue;
  depth?: number;
}) {
  const [expanded, setExpanded] = useState(depth < 1);
  const isObject = value !== null && typeof value === "object" && !Array.isArray(value);
  const isArray = Array.isArray(value);
  const isExpandable = isObject || isArray;

  if (!isExpandable) {
    return (
      <div className="meta-leaf" style={{ paddingLeft: depth * 14 }}>
        <span className="meta-key">{name}</span>
        <span className={`meta-value meta-value--${typeof value}`}>
          {value === null ? "null" : typeof value === "string" ? `"${value}"` : String(value)}
        </span>
      </div>
    );
  }

  const entries = isArray
    ? (value as MetaValue[]).map((v, i) => [String(i), v] as const)
    : Object.entries(value as Record<string, MetaValue>);

  return (
    <div className="meta-branch">
      <button
        className="meta-toggle"
        style={{ paddingLeft: depth * 14 }}
        onClick={() => setExpanded((v) => !v)}
      >
        <CaretDown
          size={10}
          weight="bold"
          style={{
            transform: expanded ? undefined : "rotate(-90deg)",
            transition: "transform 0.15s",
          }}
        />
        <span className="meta-key">{name}</span>
        <span className="meta-hint">
          {isArray ? `[${entries.length}]` : `{${entries.length}}`}
        </span>
      </button>
      {expanded &&
        entries.map(([k, v]) => <MetaTreeNode key={k} name={k} value={v} depth={depth + 1} />)}
    </div>
  );
}

export function MetaSection({ meta }: { meta: Record<string, MetaValue> | null }) {
  if (!meta || Object.keys(meta).length === 0) return null;
  return (
    <div className="inspector-section">
      <div className="inspector-raw-head">
        <span>Metadata</span>
        <ActionButton size="sm">
          <CopySimple size={12} weight="bold" />
        </ActionButton>
      </div>
      <div className="meta-tree">
        {Object.entries(meta).map(([k, v]) => (
          <MetaTreeNode key={k} name={k} value={v} />
        ))}
      </div>
    </div>
  );
}
