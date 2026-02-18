import { useCallback, useRef, useState } from "react";
import type React from "react";
import type { RefObject } from "react";
import type { GraphGeometry } from "../geometry";
import { type Camera, screenToWorld } from "../canvas/camera";
import { hitTestNodes, hitTestEdges } from "./hitTest";

// Matches GraphSelection in App.tsx: { kind: "entity" | "edge"; id: string } | null
export type GraphSelection =
  | { kind: "entity"; id: string }
  | { kind: "edge"; id: string }
  | null;

type InteractionMode =
  | { kind: "idle" }
  | { kind: "panning"; startX: number; startY: number; startCamera: Camera }
  | { kind: "dragging"; nodeId: string; startX: number; startY: number };

// Future feature flag: enable node dragging when ready
const ENABLE_NODE_DRAGGING = false;

export interface GraphInteraction {
  selection: GraphSelection;
  hoveredNodeId: string | null;
  hoveredEdgeId: string | null;
  mode: InteractionMode;

  // Attach these to the SVG element
  onPointerDown: (e: React.PointerEvent) => void;
  onPointerMove: (e: React.PointerEvent) => void;
  onPointerUp: (e: React.PointerEvent) => void;
  onClick: (e: React.MouseEvent) => void;
}

export function useGraphInteraction(
  geometry: GraphGeometry | null,
  camera: Camera,
  svgRef: RefObject<SVGSVGElement | null>,
  callbacks: {
    setCamera?: (c: Camera) => void;
    onNodeClick?: (id: string) => void;
    onEdgeClick?: (id: string) => void;
    onBackgroundClick?: () => void;
  },
): GraphInteraction {
  const [selection, setSelection] = useState<GraphSelection>(null);
  const [hoveredNodeId, setHoveredNodeId] = useState<string | null>(null);
  const [hoveredEdgeId, setHoveredEdgeId] = useState<string | null>(null);
  const [mode, setMode] = useState<InteractionMode>({ kind: "idle" });

  // Refs for use inside pointer callbacks to avoid stale closures
  const modeRef = useRef<InteractionMode>({ kind: "idle" });
  const cameraRef = useRef<Camera>(camera);
  cameraRef.current = camera;
  const callbacksRef = useRef(callbacks);
  callbacksRef.current = callbacks;
  const geometryRef = useRef(geometry);
  geometryRef.current = geometry;

  // Pending move coords for RAF-throttled hover updates
  const rafIdRef = useRef<number | null>(null);
  const pendingMoveRef = useRef<{ clientX: number; clientY: number } | null>(null);

  const setModeAndRef = useCallback((m: InteractionMode) => {
    modeRef.current = m;
    setMode(m);
  }, []);

  const getWorldPoint = useCallback(
    (clientX: number, clientY: number) => {
      const svg = svgRef.current;
      if (!svg) return null;
      const svgRect = svg.getBoundingClientRect();
      return screenToWorld(cameraRef.current, svgRect.width, svgRect.height, {
        x: clientX - svgRect.left,
        y: clientY - svgRect.top,
      });
    },
    [svgRef],
  );

  const onPointerDown = useCallback(
    (e: React.PointerEvent) => {
      if (e.button !== 0) return;
      const worldPoint = getWorldPoint(e.clientX, e.clientY);
      if (!worldPoint) return;
      const geo = geometryRef.current;

      if (geo && ENABLE_NODE_DRAGGING) {
        const nodeId = hitTestNodes(worldPoint, geo.nodes);
        if (nodeId) {
          e.preventDefault();
          (e.currentTarget as Element).setPointerCapture(e.pointerId);
          setModeAndRef({ kind: "dragging", nodeId, startX: e.clientX, startY: e.clientY });
          return;
        }
      }

      // Pan when pointer goes down on empty space (no node or edge hit)
      const hitNode = geo ? hitTestNodes(worldPoint, geo.nodes) : null;
      const hitEdge = !hitNode && geo ? hitTestEdges(worldPoint, geo.edges) : null;
      if (!hitNode && !hitEdge) {
        e.preventDefault();
        (e.currentTarget as Element).setPointerCapture(e.pointerId);
        setModeAndRef({
          kind: "panning",
          startX: e.clientX,
          startY: e.clientY,
          startCamera: cameraRef.current,
        });
      }
    },
    [getWorldPoint, setModeAndRef],
  );

  const onPointerMove = useCallback(
    (e: React.PointerEvent) => {
      const currentMode = modeRef.current;

      if (currentMode.kind === "panning") {
        const dx = e.clientX - currentMode.startX;
        const dy = e.clientY - currentMode.startY;
        const sc = currentMode.startCamera;
        callbacksRef.current.setCamera?.({
          ...sc,
          x: sc.x - dx / sc.zoom,
          y: sc.y - dy / sc.zoom,
        });
        return;
      }

      if (currentMode.kind === "dragging") {
        // Future: update draft node position
        return;
      }

      // Idle: throttle hover hit tests to one per animation frame
      pendingMoveRef.current = { clientX: e.clientX, clientY: e.clientY };
      if (rafIdRef.current !== null) return;
      rafIdRef.current = requestAnimationFrame(() => {
        rafIdRef.current = null;
        const coords = pendingMoveRef.current;
        if (!coords) return;
        pendingMoveRef.current = null;

        const geo = geometryRef.current;
        if (!geo) {
          setHoveredNodeId(null);
          setHoveredEdgeId(null);
          return;
        }
        const worldPoint = getWorldPoint(coords.clientX, coords.clientY);
        if (!worldPoint) return;

        const nodeId = hitTestNodes(worldPoint, geo.nodes);
        setHoveredNodeId(nodeId);
        if (nodeId) {
          setHoveredEdgeId(null);
        } else {
          setHoveredEdgeId(hitTestEdges(worldPoint, geo.edges));
        }
      });
    },
    [getWorldPoint],
  );

  const onPointerUp = useCallback(
    (e: React.PointerEvent) => {
      if (modeRef.current.kind === "idle") return;
      (e.currentTarget as Element).releasePointerCapture(e.pointerId);
      setModeAndRef({ kind: "idle" });
    },
    [setModeAndRef],
  );

  const onClick = useCallback(
    (e: React.MouseEvent) => {
      const worldPoint = getWorldPoint(e.clientX, e.clientY);
      if (!worldPoint) return;
      const geo = geometryRef.current;
      const cbs = callbacksRef.current;

      if (geo) {
        const nodeId = hitTestNodes(worldPoint, geo.nodes);
        if (nodeId) {
          setSelection({ kind: "entity", id: nodeId });
          cbs.onNodeClick?.(nodeId);
          return;
        }
        const edgeId = hitTestEdges(worldPoint, geo.edges);
        if (edgeId) {
          setSelection({ kind: "edge", id: edgeId });
          cbs.onEdgeClick?.(edgeId);
          return;
        }
      }

      setSelection(null);
      cbs.onBackgroundClick?.();
    },
    [getWorldPoint],
  );

  return { selection, hoveredNodeId, hoveredEdgeId, mode, onPointerDown, onPointerMove, onPointerUp, onClick };
}
