import React, { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { Camera, CircleNotch, Crosshair } from "@phosphor-icons/react";
import { ActionButton } from "../../ui/primitives/ActionButton";
import { Badge } from "../../ui/primitives/Badge";
import type { FilterMenuItem } from "../../ui/primitives/FilterMenu";
import type { EntityDef, EdgeDef } from "../../snapshot";
import { layoutGraph, type SubgraphScopeMode } from "../../graph/elkAdapter";
import { measureGraphLayout } from "../../graph/render/NodeLayer";
import { GraphCanvas, useCameraContext } from "../../graph/canvas/GraphCanvas";
import { GroupLayer } from "../../graph/render/GroupLayer";
import { EdgeLayer } from "../../graph/render/EdgeLayer";
import { NodeLayer } from "../../graph/render/NodeLayer";
import type { GraphGeometry, Point } from "../../graph/geometry";
import type { ScopeColorPair } from "./scopeColors";
import { assignScopeColorRgbByKey } from "./scopeColors";
import type { FrameRenderResult } from "../../recording/unionGraph";
import {
  ensureTrailingSpaceForNewFilter,
  graphFilterEditorParts,
  graphFilterSuggestions,
  parseGraphFilterQuery,
  tokenizeFilterQuery,
  replaceTrailingFragment,
} from "../../graphFilter";
import "./GraphPanel.css";

export type GraphSelection =
  | { kind: "entity"; id: string }
  | { kind: "edge"; id: string }
  | null;

export type SnapPhase = "idle" | "cutting" | "loading" | "ready" | "error";

export type ScopeColorMode = "none" | "process" | "crate";

const GRAPH_EMPTY_MESSAGES: Record<SnapPhase, string> = {
  idle: "Take a snapshot to see the current state",
  cutting: "Waiting for all processes to sync…",
  loading: "Loading snapshot data…",
  ready: "No entities in snapshot",
  error: "Snapshot failed",
};

function scopeKeyForEntity(entity: EntityDef, scopeColorMode: ScopeColorMode): string | undefined {
  if (scopeColorMode === "process") return entity.processId;
  if (scopeColorMode === "crate") return entity.krate ?? "~no-crate";
  return undefined;
}

export function GraphPanel({
  entityDefs,
  edgeDefs,
  snapPhase,
  selection,
  onSelect,
  focusedEntityId,
  onExitFocus,
  waitingForProcesses,
  crateItems,
  hiddenKrates,
  onKrateToggle,
  onKrateSolo,
  processItems,
  hiddenProcesses,
  onProcessToggle,
  onProcessSolo,
  kindItems,
  hiddenKinds,
  onKindToggle,
  onKindSolo,
  scopeColorMode,
  onToggleProcessColorBy,
  onToggleCrateColorBy,
  subgraphScopeMode,
  onToggleProcessSubgraphs,
  onToggleCrateSubgraphs,
  showLoners,
  onToggleShowLoners,
  scopeFilterLabel,
  onClearScopeFilter,
  unionFrameLayout,
  graphFilterText,
  onGraphFilterTextChange,
  onHideNodeFilter,
  onHideLocationFilter,
}: {
  entityDefs: EntityDef[];
  edgeDefs: EdgeDef[];
  snapPhase: SnapPhase;
  selection: GraphSelection;
  onSelect: (sel: GraphSelection) => void;
  focusedEntityId: string | null;
  onExitFocus: () => void;
  waitingForProcesses: boolean;
  crateItems: FilterMenuItem[];
  hiddenKrates: ReadonlySet<string>;
  onKrateToggle: (krate: string) => void;
  onKrateSolo: (krate: string) => void;
  processItems: FilterMenuItem[];
  hiddenProcesses: ReadonlySet<string>;
  onProcessToggle: (pid: string) => void;
  onProcessSolo: (pid: string) => void;
  kindItems: FilterMenuItem[];
  hiddenKinds: ReadonlySet<string>;
  onKindToggle: (kind: string) => void;
  onKindSolo: (kind: string) => void;
  scopeColorMode: ScopeColorMode;
  onToggleProcessColorBy: () => void;
  onToggleCrateColorBy: () => void;
  subgraphScopeMode: SubgraphScopeMode;
  onToggleProcessSubgraphs: () => void;
  onToggleCrateSubgraphs: () => void;
  showLoners: boolean;
  onToggleShowLoners: () => void;
  scopeFilterLabel?: string | null;
  onClearScopeFilter?: () => void;
  /** When provided, use this pre-computed layout (union mode) instead of measuring + ELK. */
  unionFrameLayout?: FrameRenderResult;
  graphFilterText: string;
  onGraphFilterTextChange: (next: string) => void;
  onHideNodeFilter: (entityId: string) => void;
  onHideLocationFilter: (location: string) => void;
}) {
  const [layout, setLayout] = useState<GraphGeometry | null>(null);
  const [portAnchors, setPortAnchors] = useState<Map<string, Point>>(new Map());
  const graphFlowRef = useRef<HTMLDivElement | null>(null);
  const graphFilterInputRef = useRef<HTMLInputElement | null>(null);
  const [nodeContextMenu, setNodeContextMenu] = useState<{
    nodeId: string;
    x: number;
    y: number;
  } | null>(null);
  const [graphFilterSuggestionIndex, setGraphFilterSuggestionIndex] = useState(0);
  const [graphFilterSuggestOpen, setGraphFilterSuggestOpen] = useState(false);
  const [graphFilterEditing, setGraphFilterEditing] = useState(false);

  // In snapshot mode (no unionFrameLayout), measure and lay out from scratch.
  React.useEffect(() => {
    if (unionFrameLayout) return; // skip — union mode provides layout directly
    if (entityDefs.length === 0) return;
    measureGraphLayout(entityDefs, subgraphScopeMode)
      .then((measurements) =>
        layoutGraph(entityDefs, edgeDefs, measurements.nodeSizes, subgraphScopeMode, {
          subgraphHeaderHeight: measurements.subgraphHeaderHeight,
        }),
      )
      .then(setLayout)
      .catch(console.error);
  }, [entityDefs, edgeDefs, subgraphScopeMode, unionFrameLayout]);

  const effectiveGeometry: GraphGeometry | null = unionFrameLayout?.geometry ?? layout;

  const entityById = useMemo(() => new Map(entityDefs.map((entity) => [entity.id, entity])), [entityDefs]);
  const scopeColorByKey = useMemo<Map<string, ScopeColorPair>>(() => {
    if (scopeColorMode === "none") return new Map<string, ScopeColorPair>();
    return assignScopeColorRgbByKey(entityDefs.map((entity) => scopeKeyForEntity(entity, scopeColorMode) ?? ""));
  }, [entityDefs, scopeColorMode]);

  const nodesWithScopeColor = useMemo(() => {
    if (!effectiveGeometry) return [];
    return effectiveGeometry.nodes.map((n) => {
      const entity = entityById.get(n.id);
      const scopeKey = entity ? scopeKeyForEntity(entity, scopeColorMode) : undefined;
      const scopeRgb = scopeKey ? scopeColorByKey.get(scopeKey) : undefined;
      return {
        ...n,
        data: {
          ...n.data,
          scopeRgbLight: scopeRgb?.light,
          scopeRgbDark: scopeRgb?.dark,
        },
      };
    });
  }, [effectiveGeometry, entityById, scopeColorByKey, scopeColorMode]);

  const groupsWithScopeColor = useMemo(() => {
    if (!effectiveGeometry) return [];
    return effectiveGeometry.groups.map((group) => {
      const scopeKey = group.data?.scopeKey as string | undefined;
      const scopeRgb = scopeKey ? scopeColorByKey.get(scopeKey) : undefined;
      return {
        ...group,
        data: {
          ...group.data,
          scopeRgbLight: scopeRgb?.light,
          scopeRgbDark: scopeRgb?.dark,
        },
      };
    });
  }, [effectiveGeometry, scopeColorByKey]);

  const ghostNodeIds = unionFrameLayout?.ghostNodeIds;
  const ghostEdgeIds = unionFrameLayout?.ghostEdgeIds;
  const parsedGraphFilters = useMemo(() => parseGraphFilterQuery(graphFilterText), [graphFilterText]);
  const graphFilterTokens = parsedGraphFilters.tokens;
  const filterParts = useMemo(
    () => graphFilterEditorParts(graphFilterText, graphFilterEditing),
    [graphFilterText, graphFilterEditing],
  );
  const nodeSuggestions = useMemo(
    () => entityDefs.map((entity) => entity.id),
    [entityDefs],
  );
  const locationSuggestions = useMemo(
    () =>
      Array.from(
        new Set(
          entityDefs
            .map((entity) => entity.source?.trim() ?? "")
            .filter((source) => source.length > 0),
        ),
      ),
    [entityDefs],
  );
  const currentFragment = useMemo(() => filterParts.fragment.trim(), [filterParts.fragment]);
  const graphFilterSuggestionsList = useMemo(
    () =>
      graphFilterSuggestions({
        fragment: currentFragment,
        nodeIds: nodeSuggestions,
        locations: locationSuggestions,
        crates: crateItems.map((item) => ({ id: item.id, label: String(item.label ?? item.id) })),
        processes: processItems.map((item) => ({ id: item.id, label: String(item.label ?? item.id) })),
        kinds: kindItems.map((item) => ({ id: item.id, label: String(item.label ?? item.id) })),
      }),
    [currentFragment, nodeSuggestions, locationSuggestions, crateItems, processItems, kindItems],
  );

  const isBusy = snapPhase === "cutting" || snapPhase === "loading";
  const showToolbar =
    crateItems.length > 1 || processItems.length > 0 || kindItems.length > 1 || focusedEntityId || entityDefs.length > 0;

  // Keep track of whether we've fitted the view at least once for this layout.
  const [hasFitted, setHasFitted] = useState(false);
  const geometryKey = effectiveGeometry
    ? effectiveGeometry.nodes.map((n) => n.id).join(",")
    : "";

  const closeNodeContextMenu = useCallback(() => {
    setNodeContextMenu(null);
  }, []);

  const applyGraphFilterSuggestion = useCallback(
    (token: string) => {
      onGraphFilterTextChange(replaceTrailingFragment(graphFilterText, token));
      setGraphFilterSuggestOpen(false);
      setGraphFilterSuggestionIndex(0);
      graphFilterInputRef.current?.focus();
    },
    [graphFilterText, onGraphFilterTextChange],
  );

  const setFilterFragment = useCallback(
    (fragment: string) => {
      const prefix = filterParts.committed.join(" ");
      if (prefix.length === 0) {
        onGraphFilterTextChange(fragment);
        return;
      }
      if (fragment.length === 0) {
        onGraphFilterTextChange(`${prefix} `);
        return;
      }
      onGraphFilterTextChange(`${prefix} ${fragment}`);
    },
    [filterParts.committed, onGraphFilterTextChange],
  );

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
    if (graphFilterSuggestionIndex < graphFilterSuggestionsList.length) return;
    setGraphFilterSuggestionIndex(0);
  }, [graphFilterSuggestionIndex, graphFilterSuggestionsList.length]);

  // Reset fit state when geometry changes structure.
  useEffect(() => {
    setHasFitted(false);
  }, [geometryKey]);

  return (
    <div className="graph-panel">
      {showToolbar && (
        <div className="graph-toolbar">
          <div className="graph-toolbar-left">
            {entityDefs.length > 0 && (
              <>
                <span className="graph-stat">{entityDefs.length} entities</span>
                <span className="graph-stat-sep" aria-hidden="true">|</span>
                <span className="graph-stat">{edgeDefs.length} edges</span>
              </>
            )}
          </div>
          <div className="graph-toolbar-middle">
            <div
              className="graph-filter-input"
              onMouseDown={(event) => {
                if (event.target instanceof HTMLElement && event.target.closest(".graph-filter-chip")) return;
                graphFilterInputRef.current?.focus();
              }}
            >
              {filterParts.committed.map((raw, index) => {
                const parsed = graphFilterTokens[index];
                const valid = parsed?.valid ?? false;
                return (
                  <button
                    key={`${raw}:${index}`}
                    type="button"
                    className={[
                      "graph-filter-chip",
                      valid ? "graph-filter-chip--valid" : "graph-filter-chip--invalid",
                    ].join(" ")}
                    onMouseDown={(event) => event.preventDefault()}
                    onClick={() => {
                      const next = filterParts.committed.filter((_, i) => i !== index);
                      onGraphFilterTextChange(next.join(" "));
                      graphFilterInputRef.current?.focus();
                    }}
                    title={valid ? "remove filter token" : "invalid filter token"}
                  >
                    {raw}
                    <span className="graph-filter-chip-x" aria-hidden="true">×</span>
                  </button>
                );
              })}
              <input
                ref={graphFilterInputRef}
                type="text"
                value={filterParts.fragment}
                onChange={(event) => {
                  setFilterFragment(event.target.value);
                  setGraphFilterSuggestOpen(true);
                  setGraphFilterSuggestionIndex(0);
                }}
                onFocus={() => {
                  setGraphFilterEditing(true);
                  const nextText = ensureTrailingSpaceForNewFilter(graphFilterText);
                  if (nextText !== graphFilterText) onGraphFilterTextChange(nextText);
                  setGraphFilterSuggestOpen(true);
                }}
                onBlur={() => {
                  setGraphFilterEditing(false);
                  window.setTimeout(() => setGraphFilterSuggestOpen(false), 100);
                }}
                onKeyDown={(event) => {
                  if (event.key === "Backspace" && filterParts.fragment.length === 0 && filterParts.committed.length > 0) {
                    event.preventDefault();
                    const next = filterParts.committed.slice(0, -1);
                    onGraphFilterTextChange(next.join(" "));
                    setGraphFilterSuggestOpen(true);
                    setGraphFilterSuggestionIndex(0);
                    return;
                  }
                  if (!graphFilterSuggestOpen || graphFilterSuggestionsList.length === 0) return;
                  if (event.key === "ArrowDown") {
                    event.preventDefault();
                    setGraphFilterSuggestionIndex((idx) => (idx + 1) % graphFilterSuggestionsList.length);
                    return;
                  }
                  if (event.key === "ArrowUp") {
                    event.preventDefault();
                    setGraphFilterSuggestionIndex(
                      (idx) => (idx + graphFilterSuggestionsList.length - 1) % graphFilterSuggestionsList.length,
                    );
                    return;
                  }
                  if (event.key === "Enter" || event.key === "Tab") {
                    const choice = graphFilterSuggestionsList[graphFilterSuggestionIndex];
                    if (!choice) return;
                    event.preventDefault();
                    applyGraphFilterSuggestion(choice.token);
                  }
                }}
                placeholder={
                  filterParts.committed.length === 0
                    ? "filters: node:.. location:.. crate:.. process:.. kind:.. loners:on|off colorBy:.. groupBy:.."
                    : "add filter…"
                }
                className="graph-filter-fragment-input"
                aria-label="Graph filter query"
              />
            </div>
            {graphFilterSuggestOpen && graphFilterSuggestionsList.length > 0 && (
              <div className="graph-filter-suggestions">
                {graphFilterSuggestionsList.map((suggestion, index) => (
                  <button
                    key={suggestion.token}
                    type="button"
                    className={[
                      "graph-filter-suggestion",
                      index === graphFilterSuggestionIndex && "graph-filter-suggestion--active",
                    ].filter(Boolean).join(" ")}
                    onMouseDown={(event) => event.preventDefault()}
                    onClick={() => applyGraphFilterSuggestion(suggestion.token)}
                  >
                    <span className="graph-filter-suggestion-label">{suggestion.label}</span>
                    <span className="graph-filter-suggestion-token">{suggestion.token}</span>
                  </button>
                ))}
              </div>
            )}
          </div>
          <div className="graph-toolbar-right">
            {focusedEntityId && (
              <ActionButton size="sm" onPress={onExitFocus}>
                <Crosshair size={14} weight="bold" />
                Exit Focus
              </ActionButton>
            )}
            {scopeFilterLabel && (
              <>
                <Badge tone="warn">in:{scopeFilterLabel}</Badge>
                <ActionButton size="sm" onPress={onClearScopeFilter}>Clear scope</ActionButton>
              </>
            )}
          </div>
        </div>
      )}
      {entityDefs.length === 0 ? (
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
      ) : (
        <div className="graph-flow" ref={graphFlowRef}>
          {nodeContextMenu && (() => {
            const entity = entityById.get(nodeContextMenu.nodeId);
            const location = entity?.source?.trim() ?? "";
            return (
              <div
                className="graph-node-context-menu"
                style={{ left: nodeContextMenu.x, top: nodeContextMenu.y }}
              >
                <button
                  type="button"
                  className="graph-node-context-menu-item"
                  onClick={() => {
                    onHideNodeFilter(nodeContextMenu.nodeId);
                    setNodeContextMenu(null);
                  }}
                >
                  Hide this node
                </button>
                <button
                  type="button"
                  className="graph-node-context-menu-item"
                  disabled={!location}
                  onClick={() => {
                    if (!location) return;
                    onHideLocationFilter(location);
                    setNodeContextMenu(null);
                  }}
                >
                  Hide this location
                </button>
              </div>
            );
          })()}
          <GraphCanvas
            geometry={effectiveGeometry}
            onBackgroundClick={() => {
              closeNodeContextMenu();
              onSelect(null);
            }}
          >
            <GraphAutoFit
              geometryKey={geometryKey}
              hasFitted={hasFitted}
              onFitted={() => setHasFitted(true)}
              suppressAutoFit={!!unionFrameLayout && hasFitted}
            />
            {effectiveGeometry && (
              <>
                <GroupLayer groups={groupsWithScopeColor} />
                <GraphPortAnchors
                  geometryKey={geometryKey}
                  onAnchorsChange={setPortAnchors}
                />
                <EdgeLayer
                  edges={effectiveGeometry.edges}
                  selectedEdgeId={selection?.kind === "edge" ? selection.id : null}
                  ghostEdgeIds={ghostEdgeIds}
                  portAnchors={portAnchors}
                  onEdgeClick={(id) => {
                    closeNodeContextMenu();
                    onSelect({ kind: "edge", id });
                  }}
                />
                <NodeLayer
                  nodes={nodesWithScopeColor}
                  selectedNodeId={selection?.kind === "entity" ? selection.id : null}
                  ghostNodeIds={ghostNodeIds}
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
      )}
    </div>
  );
}

// ── GraphAutoFit ───────────────────────────────────────────────

/**
 * Renders nothing; uses useCameraContext() to trigger fitView on geometry changes.
 * Must be rendered inside <GraphCanvas>.
 */
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

  // Also wire up F key to fit view
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
