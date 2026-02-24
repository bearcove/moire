import React from "react";
import { createRoot } from "react-dom/client";
import { flushSync } from "react-dom";
import type { GeometryNode } from "../geometry";
import type { EntityDef } from "../../snapshot";
import { GraphNode } from "../../components/graph/GraphNode";
import { graphNodeDataFromEntity, computeNodeSublabel, type GraphNodeData } from "../../components/graph/graphNodeData";
import { scopeKindIcon } from "../../scopeKindSpec";
import type { GraphFilterLabelMode } from "../../graphFilter";
import "../../components/graph/ScopeGroupNode.css";
import "./NodeLayer.css";

export interface NodeLayerProps {
  nodes: GeometryNode[];
  selectedNodeId?: string | null;
  hoveredNodeId?: string | null;
  expandedNodeId?: string | null;
  onExpandNode?: (id: string | null) => void;
  onNodeClick?: (id: string) => void;
  onNodeContextMenu?: (id: string, clientX: number, clientY: number) => void;
  onNodeHover?: (id: string | null) => void;
  ghostNodeIds?: Set<string>;
}

type SubgraphScopeMode = "none" | "process" | "crate";

export type GraphMeasureResult = {
  nodeSizes: Map<string, { width: number; height: number }>;
  subgraphHeaderHeight: number;
};

// ── Measurement ───────────────────────────────────────────────

/** Render each entity's card in a hidden off-screen container and return measured sizes. */
export async function measureEntityDefs(
  defs: EntityDef[],
): Promise<Map<string, { width: number; height: number }>> {
  const measurements = await measureGraphLayout(defs, "none");
  return measurements.nodeSizes;
}

/** Measure node cards plus subgraph header height (for ELK top padding). */
export async function measureGraphLayout(
  defs: EntityDef[],
  subgraphScopeMode: SubgraphScopeMode = "none",
  labelBy?: GraphFilterLabelMode,
  showSource?: boolean,
): Promise<GraphMeasureResult> {
  // Escape React's useEffect lifecycle so flushSync works on our measurement roots.
  await Promise.resolve();
  // Ensure text measurement uses final webfont metrics.
  if (typeof document !== "undefined" && "fonts" in document) {
    try {
      await (document as Document & { fonts?: { ready?: Promise<unknown> } }).fonts?.ready;
    } catch {
      // Non-fatal: fallback metrics are still better than blocking.
    }
  }

  const container = document.createElement("div");
  container.style.cssText =
    "position:fixed;top:-9999px;left:-9999px;visibility:hidden;pointer-events:none;display:flex;flex-direction:column;align-items:flex-start;gap:4px;";
  document.body.appendChild(container);

  const sizes = new Map<string, { width: number; height: number }>();

  for (const def of defs) {
    const el = document.createElement("div");
    container.appendChild(el);
    const root = createRoot(el);

    const sublabel = labelBy ? computeNodeSublabel(def, labelBy) : undefined;
    // During measurement, useSourceLine hooks won't fire (sync render),
    // so frame lines show fn·file:line fallback text — same height as final.
    flushSync(() => root.render(<GraphNode data={{ ...graphNodeDataFromEntity(def), sublabel, showSource }} />));
    sizes.set(def.id, { width: el.offsetWidth, height: el.offsetHeight });
    root.unmount();
  }

  let subgraphHeaderHeight = 0;
  if (subgraphScopeMode !== "none") {
    const el = document.createElement("div");
    container.appendChild(el);
    const root = createRoot(el);
    const sampleLabel = subgraphScopeMode === "process" ? "moire-examples(27139)" : "moire-example";

    flushSync(() =>
      root.render(
        <div className="scope-group" style={{ width: 320 }}>
          <div className="scope-group-header">
            <span className="scope-group-label">
              <span className="scope-group-icon">
                {scopeKindIcon(subgraphScopeMode, 12)}
              </span>
              <span>{sampleLabel}</span>
            </span>
          </div>
        </div>,
      ),
    );

    const headerEl = el.querySelector(".scope-group-header");
    if (headerEl instanceof HTMLElement) subgraphHeaderHeight = headerEl.offsetHeight;
    root.unmount();
  }

  document.body.removeChild(container);
  return { nodeSizes: sizes, subgraphHeaderHeight };
}

// ── NodeLayer ──────────────────────────────────────────────────

export function NodeLayer({
  nodes,
  selectedNodeId,
  hoveredNodeId: _hoveredNodeId,
  expandedNodeId,
  onExpandNode,
  onNodeClick,
  onNodeContextMenu,
  onNodeHover,
  ghostNodeIds,
}: NodeLayerProps) {

  if (nodes.length === 0) return null;

  // Render expanded node last so it paints on top (SVG has no z-index).
  const ordered = expandedNodeId
    ? [...nodes].sort((a, b) =>
        a.id === expandedNodeId ? 1 : b.id === expandedNodeId ? -1 : 0,
      )
    : nodes;

  return (
    <>
      {ordered.map((node) => {
        const { x, y, width, height } = node.worldRect;
        const selected = node.id === selectedNodeId;
        const isGhost = !!(node.data?.ghost as boolean | undefined) || !!ghostNodeIds?.has(node.id);
        const isExpanded = expandedNodeId === node.id;
        const cardContent = (
          <GraphNode
            data={{ ...(node.data as GraphNodeData), selected, ghost: isGhost }}
            expanded={isExpanded}
            onToggleExpand={() => onExpandNode?.(isExpanded ? null : node.id)}
          />
        );

        return (
          <foreignObject
            key={node.id}
            x={x}
            y={y}
            width={width}
            height={height}
            data-pan-block="true"
            style={{ overflow: "visible" }}
            onClick={() => onNodeClick?.(node.id)}
            onContextMenu={(event) => {
              event.preventDefault();
              event.stopPropagation();
              onNodeContextMenu?.(node.id, event.clientX, event.clientY);
            }}
            onMouseEnter={() => onNodeHover?.(node.id)}
            onMouseLeave={() => onNodeHover?.(null)}
          >
            {/* xmlns required for HTML content inside SVG foreignObject */}
            <div
              // @ts-expect-error xmlns is valid in SVG foreignObject context
              xmlns="http://www.w3.org/1999/xhtml"
              className="nl-fo-wrapper"
            >
              {cardContent}
            </div>
          </foreignObject>
        );
      })}
    </>
  );
}
