import React, { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { Camera, CircleNotch } from "@phosphor-icons/react";
import type { EntityDef } from "../../snapshot";
import { quoteFilterValue } from "../../graphFilter";
import { canonicalNodeKind, kindDisplayName } from "../../nodeKindSpec";
import { formatProcessLabel } from "../../processLabel";
import { GraphCanvas, useCameraContext } from "../../graph/canvas/GraphCanvas";
import { GroupLayer } from "../../graph/render/GroupLayer";
import { EdgeLayer } from "../../graph/render/EdgeLayer";
import { NodeLayer } from "../../graph/render/NodeLayer";
import type { GraphGeometry, GeometryGroup, GeometryNode, Point } from "../../graph/geometry";

export function GraphViewport({
  entityDefs,
  snapPhase,
  waitingForProcesses,
  geometry,
  groups,
  nodes,
  selection,
  onSelect,
  unionModeSuppressAutoFit,
  entityById,
  onHideNodeFilter,
  onHideLocationFilter,
  onFocusConnected,
  onAppendFilterToken,
  ghostNodeIds,
  ghostEdgeIds,
}: {
  entityDefs: EntityDef[];
  snapPhase: "idle" | "cutting" | "loading" | "ready" | "error";
  waitingForProcesses: boolean;
  geometry: GraphGeometry | null;
  groups: GeometryGroup[];
  nodes: GeometryNode[];
  selection: { kind: "entity"; id: string } | { kind: "edge"; id: string } | null;
  onSelect: (next: { kind: "entity"; id: string } | { kind: "edge"; id: string } | null) => void;
  unionModeSuppressAutoFit: boolean;
  entityById: Map<string, EntityDef>;
  onHideNodeFilter: (entityId: string) => void;
  onHideLocationFilter: (location: string) => void;
  onFocusConnected: (entityId: string) => void;
  onAppendFilterToken: (token: string) => void;
  ghostNodeIds?: Set<string>;
  ghostEdgeIds?: Set<string>;
}) {
  const effectiveGhostNodeIds = useMemo(() => {
    if (!geometry || selection?.kind !== "entity") return ghostNodeIds;
    const selected = selection.id;
    const s = new Set<string>(ghostNodeIds);
    for (const node of geometry.nodes) {
      if (node.id !== selected) s.add(node.id);
    }
    return s;
  }, [geometry, selection, ghostNodeIds]);

  const effectiveGhostEdgeIds = useMemo(() => {
    if (!geometry || selection?.kind !== "entity") return ghostEdgeIds;
    const selected = selection.id;
    const s = new Set<string>(ghostEdgeIds);
    for (const edge of geometry.edges) {
      if (edge.sourceId !== selected && edge.targetId !== selected) s.add(edge.id);
    }
    return s;
  }, [geometry, selection, ghostEdgeIds]);

  const [portAnchors, setPortAnchors] = useState<Map<string, Point>>(new Map());
  const [hasFitted, setHasFitted] = useState(false);
  const graphFlowRef = useRef<HTMLDivElement | null>(null);
  const [nodeContextMenu, setNodeContextMenu] = useState<{
    nodeId: string;
    x: number;
    y: number;
  } | null>(null);

  const geometryKey = geometry ? geometry.nodes.map((n) => n.id).join(",") : "";
  const isBusy = snapPhase === "cutting" || snapPhase === "loading";
  const closeNodeContextMenu = useCallback(() => setNodeContextMenu(null), []);

  useEffect(() => {
    if (!nodeContextMenu) return;
    const onPointerDown = (event: PointerEvent) => {
      const target = event.target as HTMLElement | null;
      if (target?.closest(".graph-node-context-menu")) return;
      setNodeContextMenu(null);
    };
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") setNodeContextMenu(null);
    };
    const onResize = () => setNodeContextMenu(null);
    window.addEventListener("pointerdown", onPointerDown, true);
    window.addEventListener("keydown", onKeyDown);
    window.addEventListener("resize", onResize);
    return () => {
      window.removeEventListener("pointerdown", onPointerDown, true);
      window.removeEventListener("keydown", onKeyDown);
      window.removeEventListener("resize", onResize);
    };
  }, [nodeContextMenu]);

  useEffect(() => {
    setHasFitted(false);
  }, [geometryKey]);

  if (entityDefs.length === 0) {
    return (
      <div className="graph-empty">
        {isBusy ? (
          <>
            <CircleNotch size={24} weight="bold" className="spinning graph-empty-icon" />{" "}
            {GRAPH_EMPTY_MESSAGES[snapPhase]}
          </>
        ) : snapPhase === "idle" && waitingForProcesses ? (
          <>
            <CircleNotch size={24} weight="bold" className="spinning graph-empty-icon" />
            <span>Waiting for a process to connect…</span>
          </>
        ) : snapPhase === "idle" ? (
          <>
            <Camera size={32} weight="thin" className="graph-empty-icon" />
            <span>{GRAPH_EMPTY_MESSAGES[snapPhase]}</span>
            <span className="graph-empty-hint">
              Press "Take Snapshot" to capture the current state of all connected processes
            </span>
          </>
        ) : (
          GRAPH_EMPTY_MESSAGES[snapPhase]
        )}
      </div>
    );
  }

  return (
    <div className="graph-flow" ref={graphFlowRef}>
      {nodeContextMenu && (() => {
        const entity = entityById.get(nodeContextMenu.nodeId);
        const location = entity?.source?.trim() ?? "";
        const krate = entity?.krate;
        const processId = entity?.processId ?? "";
        const processLabel = entity
          ? formatProcessLabel(entity.processName, entity.processPid)
          : processId;
        const kind = entity ? canonicalNodeKind(entity.kind) : "";
        const kindLabel = kind ? kindDisplayName(kind) : "";
        const close = () => setNodeContextMenu(null);
        return (
          <div
            className="graph-node-context-menu"
            style={{ left: nodeContextMenu.x, top: nodeContextMenu.y }}
          >
            <button
              type="button"
              className="graph-node-context-menu-item"
              onClick={() => {
                onFocusConnected(nodeContextMenu.nodeId);
                close();
              }}
            >
              Show only connected
            </button>
            <div className="graph-node-context-menu-separator" />
            <button
              type="button"
              className="graph-node-context-menu-item"
              onClick={() => {
                onHideNodeFilter(nodeContextMenu.nodeId);
                close();
              }}
            >
              Hide this node
            </button>
            {location && (
              <button
                type="button"
                className="graph-node-context-menu-item"
                onClick={() => {
                  onHideLocationFilter(location);
                  close();
                }}
              >
                Hide this location
              </button>
            )}
            {krate && (
              <>
                <div className="graph-node-context-menu-separator" />
                <button
                  type="button"
                  className="graph-node-context-menu-item"
                  onClick={() => {
                    onAppendFilterToken(`-crate:${quoteFilterValue(krate)}`);
                    close();
                  }}
                >
                  Hide this crate
                </button>
                <button
                  type="button"
                  className="graph-node-context-menu-item"
                  onClick={() => {
                    onAppendFilterToken(`+crate:${quoteFilterValue(krate)}`);
                    close();
                  }}
                >
                  Show only this crate
                </button>
              </>
            )}
            {processId && (
              <>
                <div className="graph-node-context-menu-separator" />
                <button
                  type="button"
                  className="graph-node-context-menu-item"
                  onClick={() => {
                    onAppendFilterToken(`-process:${quoteFilterValue(processId)}`);
                    close();
                  }}
                >
                  Hide process: {processLabel}
                </button>
                <button
                  type="button"
                  className="graph-node-context-menu-item"
                  onClick={() => {
                    onAppendFilterToken(`+process:${quoteFilterValue(processId)}`);
                    close();
                  }}
                >
                  Show only process: {processLabel}
                </button>
              </>
            )}
            {kind && (
              <>
                <div className="graph-node-context-menu-separator" />
                <button
                  type="button"
                  className="graph-node-context-menu-item"
                  onClick={() => {
                    onAppendFilterToken(`-kind:${quoteFilterValue(kind)}`);
                    close();
                  }}
                >
                  Hide kind: {kindLabel}
                </button>
                <button
                  type="button"
                  className="graph-node-context-menu-item"
                  onClick={() => {
                    onAppendFilterToken(`+kind:${quoteFilterValue(kind)}`);
                    close();
                  }}
                >
                  Show only kind: {kindLabel}
                </button>
              </>
            )}
          </div>
        );
      })()}
      <GraphCanvas
        geometry={geometry}
        onBackgroundClick={() => {
          closeNodeContextMenu();
          onSelect(null);
        }}
      >
        <GraphAutoFit
          geometryKey={geometryKey}
          hasFitted={hasFitted}
          onFitted={() => setHasFitted(true)}
          suppressAutoFit={unionModeSuppressAutoFit && hasFitted}
        />
        {geometry && (
          <>
            <GroupLayer groups={groups} />
            <GraphPortAnchors
              geometryKey={geometryKey}
              onAnchorsChange={setPortAnchors}
            />
            <EdgeLayer
              edges={geometry.edges}
              selectedEdgeId={selection?.kind === "edge" ? selection.id : null}
              ghostEdgeIds={effectiveGhostEdgeIds}
              portAnchors={portAnchors}
              onEdgeClick={(id) => {
                closeNodeContextMenu();
                onSelect({ kind: "edge", id });
              }}
            />
            <NodeLayer
              nodes={nodes}
              selectedNodeId={selection?.kind === "entity" ? selection.id : null}
              ghostNodeIds={effectiveGhostNodeIds}
              onNodeClick={(id) => {
                closeNodeContextMenu();
                onSelect({ kind: "entity", id });
              }}
              onNodeContextMenu={(id, clientX, clientY) => {
                onSelect({ kind: "entity", id });
                const graphFlow = graphFlowRef.current;
                if (!graphFlow) return;
                const rect = graphFlow.getBoundingClientRect();
                const x = Math.max(8, Math.min(clientX - rect.left, Math.max(8, rect.width - 8)));
                const y = Math.max(8, Math.min(clientY - rect.top, Math.max(8, rect.height - 8)));
                setNodeContextMenu({ nodeId: id, x, y });
              }}
            />
          </>
        )}
      </GraphCanvas>
    </div>
  );
}

const GRAPH_EMPTY_MESSAGES: Record<"idle" | "cutting" | "loading" | "ready" | "error", string> = {
  idle: "Take a snapshot to see the current state",
  cutting: "Waiting for all processes to sync…",
  loading: "Loading snapshot data…",
  ready: "No entities in snapshot",
  error: "Snapshot failed",
};

function GraphAutoFit({
  geometryKey,
  hasFitted,
  onFitted,
  suppressAutoFit,
}: {
  geometryKey: string;
  hasFitted: boolean;
  onFitted: () => void;
  suppressAutoFit: boolean;
}) {
  const { fitView } = useCameraContext();

  useEffect(() => {
    if (suppressAutoFit) return;
    if (!geometryKey) return;
    fitView();
    onFitted();
  }, [geometryKey]); // eslint-disable-line react-hooks/exhaustive-deps

  useEffect(() => {
    const onKeyDown = (e: KeyboardEvent) => {
      if (e.key === "f" && !e.metaKey && !e.ctrlKey && !e.altKey) {
        const tag = (e.target as HTMLElement).tagName;
        if (tag === "INPUT" || tag === "TEXTAREA") return;
        fitView();
      }
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [fitView]);

  return null;
}

function GraphPortAnchors({
  geometryKey,
  onAnchorsChange,
}: {
  geometryKey: string;
  onAnchorsChange: (anchors: Map<string, Point>) => void;
}) {
  const { clientToGraph } = useCameraContext();

  useEffect(() => {
    if (!geometryKey) {
      onAnchorsChange(new Map());
      return;
    }
    const raf = window.requestAnimationFrame(() => {
      const anchors = new Map<string, Point>();
      const nodes = document.querySelectorAll<HTMLElement>(".graph-port-anchor[data-port-id]");
      nodes.forEach((node) => {
        const portId = node.dataset.portId;
        if (!portId) return;
        const rect = node.getBoundingClientRect();
        const centerX = rect.left + rect.width / 2;
        const centerY = rect.top + rect.height / 2;
        const world = clientToGraph(centerX, centerY);
        if (!world) return;
        anchors.set(portId, world);
      });
      onAnchorsChange(anchors);
    });
    return () => window.cancelAnimationFrame(raf);
  }, [clientToGraph, geometryKey, onAnchorsChange]);

  return null;
}
