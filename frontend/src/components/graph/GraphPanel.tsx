import React, { useEffect, useMemo, useState } from "react";
import { ReactFlowProvider } from "@xyflow/react";
import { Camera, CircleNotch, Crosshair } from "@phosphor-icons/react";
import { ActionButton } from "../../ui/primitives/ActionButton";
import { FilterMenu, type FilterMenuItem } from "../../ui/primitives/FilterMenu";
import type { EntityDef, EdgeDef, Tone } from "../../snapshot";
import { measureNodeDefs, layoutGraph, type LayoutResult, type SubgraphScopeMode } from "../../layout";
import { GraphFlow, type GraphSelection } from "./GraphFlow";
import { renderNodeForMeasure } from "./nodeTypes";
import { scopeHueForKey } from "./scopeColors";
import "./GraphPanel.css";

export type { GraphSelection };

export type SnapPhase = "idle" | "cutting" | "loading" | "ready" | "error";

export type ScopeColorMode = "none" | "process" | "crate";

const GRAPH_EMPTY_MESSAGES: Record<SnapPhase, string> = {
  idle: "Take a snapshot to see the current state",
  cutting: "Waiting for all processes to sync…",
  loading: "Loading snapshot data…",
  ready: "No entities in snapshot",
  error: "Snapshot failed",
};

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
  scopeColorMode,
  onToggleProcessColorBy,
  onToggleCrateColorBy,
  subgraphScopeMode,
  onToggleProcessSubgraphs,
  onToggleCrateSubgraphs,
  unionFrameLayout,
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
  scopeColorMode: ScopeColorMode;
  onToggleProcessColorBy: () => void;
  onToggleCrateColorBy: () => void;
  subgraphScopeMode: SubgraphScopeMode;
  onToggleProcessSubgraphs: () => void;
  onToggleCrateSubgraphs: () => void;
  /** When provided, use this pre-computed layout (union mode) instead of measuring + ELK. */
  unionFrameLayout?: LayoutResult;
}) {
  const [layout, setLayout] = useState<LayoutResult>({ nodes: [], edges: [] });

  // In snapshot mode (no unionFrameLayout), measure and lay out from scratch.
  React.useEffect(() => {
    if (unionFrameLayout) return; // skip — union mode provides layout directly
    if (entityDefs.length === 0) return;
    measureNodeDefs(entityDefs, renderNodeForMeasure)
      .then((sizes) => layoutGraph(entityDefs, edgeDefs, sizes, subgraphScopeMode))
      .then(setLayout)
      .catch(console.error);
  }, [entityDefs, edgeDefs, subgraphScopeMode, unionFrameLayout]);

  const effectiveLayout = unionFrameLayout ?? layout;

  const entityById = useMemo(() => new Map(entityDefs.map((entity) => [entity.id, entity])), [entityDefs]);

  const nodesWithSelection = useMemo(
    () =>
      effectiveLayout.nodes.map((n) => {
        const entity = entityById.get(n.id);
        const scopeKey =
          !entity
            ? undefined
            : scopeColorMode === "process"
              ? entity.processId
              : scopeColorMode === "crate"
                ? (entity.krate ?? "~no-crate")
                : undefined;
        return {
          ...n,
          data: {
            ...n.data,
            selected: selection?.kind === "entity" && n.id === selection.id,
            scopeHue: scopeKey ? scopeHueForKey(scopeKey) : undefined,
          },
        };
      }),
    [effectiveLayout.nodes, entityById, scopeColorMode, selection],
  );

  const edgesWithSelection = useMemo(
    () =>
      effectiveLayout.edges.map((e) => ({
        ...e,
        selected: selection?.kind === "edge" && e.id === selection.id,
      })),
    [effectiveLayout.edges, selection],
  );

  const isBusy = snapPhase === "cutting" || snapPhase === "loading";
  const showToolbar = crateItems.length > 1 || processItems.length > 0 || focusedEntityId;

  return (
    <div className="graph-panel">
      {showToolbar && (
        <div className="graph-toolbar">
          <div className="graph-toolbar-left">
            {entityDefs.length > 0 && (
              <>
                <span className="graph-stat">{entityDefs.length} entities</span>
                <span className="graph-stat">{edgeDefs.length} edges</span>
              </>
            )}
          </div>
          <div className="graph-toolbar-right">
            {processItems.length > 0 && (
              <FilterMenu
                label="Process"
                items={processItems}
                hiddenIds={hiddenProcesses}
                onToggle={onProcessToggle}
                onSolo={onProcessSolo}
                colorByActive={scopeColorMode === "process"}
                onToggleColorBy={onToggleProcessColorBy}
                colorByLabel="Use process colors"
                subgraphsActive={subgraphScopeMode === "process"}
                onToggleSubgraphs={onToggleProcessSubgraphs}
                subgraphsLabel="Use subgraphs"
              />
            )}
            {crateItems.length > 1 && (
              <FilterMenu
                label="Crate"
                items={crateItems}
                hiddenIds={hiddenKrates}
                onToggle={onKrateToggle}
                onSolo={onKrateSolo}
                colorByActive={scopeColorMode === "crate"}
                onToggleColorBy={onToggleCrateColorBy}
                colorByLabel="Use crate colors"
                subgraphsActive={subgraphScopeMode === "crate"}
                onToggleSubgraphs={onToggleCrateSubgraphs}
                subgraphsLabel="Use subgraphs"
              />
            )}
            {focusedEntityId && (
              <ActionButton onPress={onExitFocus}>
                <Crosshair size={14} weight="bold" />
                Exit Focus
              </ActionButton>
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
        <div className="graph-flow">
          <ReactFlowProvider>
            <GraphFlow
              nodes={nodesWithSelection}
              edges={edgesWithSelection}
              onSelect={onSelect}
              suppressAutoFit={!!unionFrameLayout}
            />
          </ReactFlowProvider>
        </div>
      )}
    </div>
  );
}
