import type { Node, Edge } from "@xyflow/react";
import type { FrameSummary } from "../api/types";
import type { ApiClient } from "../api/client";
import {
  convertSnapshot,
  getConnectedSubgraph,
  type EntityDef,
  type EdgeDef,
} from "../snapshot";
import {
  measureNodeDefs,
  layoutGraph,
  type LayoutResult,
  type RenderNodeForMeasure,
} from "../layout";

// ── Types ─────────────────────────────────────────────────────

export interface UnionLayout {
  /** Full ELK layout result (nodes with positions, edges with waypoints). */
  nodes: Node[];
  edges: Edge[];
  /** Per-frame converted data: frameIndex → { entities, edges }. */
  frameCache: Map<number, { entities: EntityDef[]; edges: EdgeDef[] }>;
  /** Which node IDs exist at each frame index. */
  nodePresence: Map<string, Set<number>>;
  /** Which edge IDs exist at each frame index. */
  edgePresence: Map<string, Set<number>>;
}

// ── Build ─────────────────────────────────────────────────────

const BATCH_SIZE = 20;

export async function buildUnionLayout(
  frames: FrameSummary[],
  apiClient: ApiClient,
  renderNode: RenderNodeForMeasure,
  onProgress?: (loaded: number, total: number) => void,
): Promise<UnionLayout> {
  const total = frames.length;
  const frameCache = new Map<number, { entities: EntityDef[]; edges: EdgeDef[] }>();

  // Fetch all frames in parallel batches.
  for (let batchStart = 0; batchStart < total; batchStart += BATCH_SIZE) {
    const batchEnd = Math.min(batchStart + BATCH_SIZE, total);
    const promises: Promise<void>[] = [];
    for (let i = batchStart; i < batchEnd; i++) {
      const frameIndex = frames[i].frame_index;
      promises.push(
        apiClient.fetchRecordingFrame(frameIndex).then((snapshot) => {
          const converted = convertSnapshot(snapshot);
          frameCache.set(frameIndex, converted);
        }),
      );
    }
    await Promise.all(promises);
    onProgress?.(batchEnd, total);
  }

  // Build union: collect all unique EntityDefs by ID (latest version wins),
  // all unique EdgeDefs by ID.
  const unionEntitiesById = new Map<string, EntityDef>();
  const unionEdgesById = new Map<string, EdgeDef>();
  const nodePresence = new Map<string, Set<number>>();
  const edgePresence = new Map<string, Set<number>>();

  for (const [frameIndex, { entities, edges }] of frameCache) {
    for (const entity of entities) {
      unionEntitiesById.set(entity.id, entity);
      if (!nodePresence.has(entity.id)) nodePresence.set(entity.id, new Set());
      nodePresence.get(entity.id)!.add(frameIndex);
    }
    for (const edge of edges) {
      unionEdgesById.set(edge.id, edge);
      if (!edgePresence.has(edge.id)) edgePresence.set(edge.id, new Set());
      edgePresence.get(edge.id)!.add(frameIndex);
    }
  }

  const unionEntities = Array.from(unionEntitiesById.values());
  const unionEdges = Array.from(unionEdgesById.values());

  // Measure and layout the full union graph.
  const sizes = await measureNodeDefs(unionEntities, renderNode);
  const layout = await layoutGraph(unionEntities, unionEdges, sizes);

  return {
    nodes: layout.nodes,
    edges: layout.edges,
    frameCache,
    nodePresence,
    edgePresence,
  };
}

// ── Entity diffs ──────────────────────────────────────────────

export interface EntityDiff {
  appeared: boolean;
  disappeared: boolean;
  statusChanged: { from: string; to: string } | null;
  statChanged: { from: string | undefined; to: string | undefined } | null;
  ageChange: number;
}

export function diffEntityBetweenFrames(
  entityId: string,
  currentFrameIndex: number,
  prevFrameIndex: number,
  unionLayout: UnionLayout,
): EntityDiff | null {
  const currentData = unionLayout.frameCache.get(currentFrameIndex);
  const prevData = unionLayout.frameCache.get(prevFrameIndex);

  const currentEntity = currentData?.entities.find((e) => e.id === entityId) ?? null;
  const prevEntity = prevData?.entities.find((e) => e.id === entityId) ?? null;

  if (!currentEntity && !prevEntity) return null;

  const appeared = !!currentEntity && !prevEntity;
  const disappeared = !currentEntity && !!prevEntity;

  let statusChanged: { from: string; to: string } | null = null;
  let statChanged: { from: string | undefined; to: string | undefined } | null = null;
  let ageChange = 0;

  if (currentEntity && prevEntity) {
    const fromStatus = prevEntity.status?.label ?? "?";
    const toStatus = currentEntity.status?.label ?? "?";
    if (fromStatus !== toStatus) {
      statusChanged = { from: fromStatus, to: toStatus };
    }
    if (currentEntity.stat !== prevEntity.stat) {
      statChanged = { from: prevEntity.stat, to: currentEntity.stat };
    }
    ageChange = currentEntity.ageMs - prevEntity.ageMs;
  }

  return { appeared, disappeared, statusChanged, statChanged, ageChange };
}

// ── Change summaries ──────────────────────────────────────────

export interface FrameChangeSummary {
  nodesAdded: number;
  nodesRemoved: number;
  edgesAdded: number;
  edgesRemoved: number;
}

export function computeFrameChangeSummary(
  frameIndex: number,
  unionLayout: UnionLayout,
): FrameChangeSummary {
  const prevIndex = frameIndex - 1;
  let nodesAdded = 0;
  let nodesRemoved = 0;
  let edgesAdded = 0;
  let edgesRemoved = 0;

  for (const [, frames] of unionLayout.nodePresence) {
    const inCurrent = frames.has(frameIndex);
    const inPrev = prevIndex >= 0 && frames.has(prevIndex);
    if (inCurrent && !inPrev) nodesAdded++;
    if (!inCurrent && inPrev) nodesRemoved++;
  }

  for (const [, frames] of unionLayout.edgePresence) {
    const inCurrent = frames.has(frameIndex);
    const inPrev = prevIndex >= 0 && frames.has(prevIndex);
    if (inCurrent && !inPrev) edgesAdded++;
    if (!inCurrent && inPrev) edgesRemoved++;
  }

  return { nodesAdded, nodesRemoved, edgesAdded, edgesRemoved };
}

export function computeChangeSummaries(unionLayout: UnionLayout): FrameChangeSummary[] {
  const frameCount = unionLayout.frameCache.size;
  return Array.from({ length: frameCount }, (_, i) => computeFrameChangeSummary(i, unionLayout));
}

export function computeChangeFrames(unionLayout: UnionLayout): number[] {
  const frameCount = unionLayout.frameCache.size;
  const result: number[] = [0];

  for (let i = 1; i < frameCount; i++) {
    let changed = false;
    for (const [, frames] of unionLayout.nodePresence) {
      if (frames.has(i) !== frames.has(i - 1)) {
        changed = true;
        break;
      }
    }
    if (!changed) {
      for (const [, frames] of unionLayout.edgePresence) {
        if (frames.has(i) !== frames.has(i - 1)) {
          changed = true;
          break;
        }
      }
    }
    if (changed) result.push(i);
  }

  return result;
}

// ── Per-frame rendering ───────────────────────────────────────

export function renderFrameFromUnion(
  frameIndex: number,
  unionLayout: UnionLayout,
  hiddenKrates: ReadonlySet<string>,
  hiddenProcesses: ReadonlySet<string>,
  focusedEntityId: string | null,
  ghostMode?: boolean,
): LayoutResult {
  const frameData = unionLayout.frameCache.get(frameIndex);
  if (!frameData) return { nodes: [], edges: [] };

  // Apply krate/process filters.
  let filteredEntities = frameData.entities.filter(
    (e) =>
      (hiddenKrates.size === 0 || !hiddenKrates.has(e.krate ?? "~no-crate")) &&
      (hiddenProcesses.size === 0 || !hiddenProcesses.has(e.processId)),
  );
  let filteredEdges = frameData.edges;

  // Apply focused entity subgraph filtering.
  if (focusedEntityId) {
    const subgraph = getConnectedSubgraph(focusedEntityId, filteredEntities, filteredEdges);
    filteredEntities = subgraph.entities;
    filteredEdges = subgraph.edges;
  }

  // Build the visible ID sets for this frame.
  const visibleNodeIds = new Set(filteredEntities.map((e) => e.id));
  const visibleEdgeIds = new Set(
    filteredEdges
      .filter((e) => visibleNodeIds.has(e.source) && visibleNodeIds.has(e.target))
      .map((e) => e.id),
  );

  // Build a lookup from frame entity/edge data for updating node data.
  const frameEntityById = new Map(filteredEntities.map((e) => [e.id, e]));

  // Track all rendered node IDs (present + ghost) for edge validity in ghost mode.
  const renderedNodeIds = new Set<string>();

  const nodes: Node[] = [];
  for (const unionNode of unionLayout.nodes) {
    const isPresent = visibleNodeIds.has(unionNode.id);

    if (isPresent) {
      const frameDef = frameEntityById.get(unionNode.id);
      if (!frameDef) continue;

      // Rebuild node data from the frame's entity (body/status may change per frame)
      // but keep the position from the union layout.
      let data: Record<string, unknown>;
      if (frameDef.channelPair) {
        data = {
          tx: frameDef.channelPair.tx,
          rx: frameDef.channelPair.rx,
          channelName: frameDef.name,
          selected: false,
          statTone: frameDef.statTone,
        };
      } else if (frameDef.rpcPair) {
        data = {
          req: frameDef.rpcPair.req,
          resp: frameDef.rpcPair.resp,
          rpcName: frameDef.name,
          selected: false,
        };
      } else {
        data = {
          kind: frameDef.kind,
          label: frameDef.name,
          inCycle: frameDef.inCycle,
          selected: false,
          status: frameDef.status,
          ageMs: frameDef.ageMs,
          stat: frameDef.stat,
          statTone: frameDef.statTone,
        };
      }

      nodes.push({ ...unionNode, data });
      renderedNodeIds.add(unionNode.id);
    } else if (ghostMode) {
      nodes.push({
        ...unionNode,
        data: { ...unionNode.data, ghost: true, selected: false },
        selectable: false,
        style: { pointerEvents: "none" as const },
      });
      renderedNodeIds.add(unionNode.id);
    }
  }

  const edges: Edge[] = [];
  for (const unionEdge of unionLayout.edges) {
    if (visibleEdgeIds.has(unionEdge.id)) {
      edges.push(unionEdge);
    } else if (
      ghostMode &&
      renderedNodeIds.has(unionEdge.source) &&
      renderedNodeIds.has(unionEdge.target)
    ) {
      edges.push({
        ...unionEdge,
        data: { ...unionEdge.data, ghost: true },
      });
    }
  }

  return { nodes, edges };
}
