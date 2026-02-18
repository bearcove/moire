import React, { useEffect, useMemo, useRef } from "react";
import {
  ReactFlow,
  useReactFlow,
  Background,
  BackgroundVariant,
  Controls,
  type Node,
  type Edge,
} from "@xyflow/react";
import { nodeTypes, edgeTypes } from "./nodeTypes";

export type GraphSelection =
  | { kind: "entity"; id: string }
  | { kind: "edge"; id: string }
  | null;

export function GraphFlow({
  nodes,
  edges,
  onSelect,
  suppressAutoFit,
}: {
  nodes: Node[];
  edges: Edge[];
  onSelect: (sel: GraphSelection) => void;
  /** When true, skip automatic fitView on structure changes (used during scrubbing). */
  suppressAutoFit?: boolean;
}) {
  const { fitView } = useReactFlow();
  const hasFittedRef = useRef(false);

  // Only refit when the graph structure changes (nodes/edges added or removed),
  // not on selection changes which also mutate the nodes array.
  const layoutKey = useMemo(
    () => nodes.map((n) => n.id).join(",") + "|" + edges.map((e) => e.id).join(","),
    [nodes, edges],
  );
  useEffect(() => {
    if (suppressAutoFit && hasFittedRef.current) return;
    fitView({ padding: 0.3, maxZoom: 1.2, duration: 0 });
    hasFittedRef.current = true;
  }, [layoutKey, fitView, suppressAutoFit]);

  // Press F to fit the view.
  useEffect(() => {
    const onKeyDown = (e: KeyboardEvent) => {
      if (e.key === "f" && !e.metaKey && !e.ctrlKey && !e.altKey) {
        const tag = (e.target as HTMLElement).tagName;
        if (tag === "INPUT" || tag === "TEXTAREA") return;
        fitView({ padding: 0.3, maxZoom: 1.2, duration: 300 });
      }
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [fitView]);

  return (
    <ReactFlow
      nodes={nodes}
      edges={edges}
      nodeTypes={nodeTypes}
      edgeTypes={edgeTypes}
      onNodeClick={(_event, node) => {
        if ((node.data as { isScopeGroup?: boolean } | undefined)?.isScopeGroup) return;
        onSelect({ kind: "entity", id: node.id });
      }}
      onEdgeClick={(_event, edge) => onSelect({ kind: "edge", id: edge.id })}
      onPaneClick={() => onSelect(null)}
      proOptions={{ hideAttribution: true }}
      minZoom={0.3}
      maxZoom={3}
      panOnDrag
      nodesDraggable={false}
      nodesConnectable={false}
      elementsSelectable
    >
      <Background variant={BackgroundVariant.Dots} gap={16} size={1} />
      <Controls showInteractive={false} />
    </ReactFlow>
  );
}
