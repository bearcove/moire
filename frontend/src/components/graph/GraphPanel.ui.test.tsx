// @vitest-environment jsdom
import React, { useState } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { GraphPanel, type ScopeColorMode } from "./GraphPanel";

vi.mock("../../graph/elkAdapter", () => ({
  layoutGraph: vi.fn(async () => ({
    nodes: [],
    groups: [],
    edges: [],
    bounds: { x: 0, y: 0, width: 0, height: 0 },
  })),
}));

afterEach(() => cleanup());

function Harness({ initialFilter }: { initialFilter: string }) {
  const [graphFilterText, setGraphFilterText] = useState(initialFilter);
  const [scopeColorMode] = useState<ScopeColorMode>("none");
  const [subgraphScopeMode] = useState<"none" | "process" | "crate">("none");

  return (
    <GraphPanel
      entityDefs={[]}
      edgeDefs={[]}
      snapPhase="ready"
      selection={null}
      onSelect={() => {}}
      focusedEntityId={null}
      onExitFocus={() => {}}
      waitingForProcesses={false}
      crateItems={[
        { id: "crate-a", label: "crate-a", meta: 1 },
        { id: "crate-b", label: "crate-b", meta: 1 },
      ]}
      hiddenKrates={new Set()}
      onKrateToggle={() => {}}
      onKrateSolo={() => {}}
      processItems={[
        { id: "1", label: "web(1234)", meta: 1 },
      ]}
      hiddenProcesses={new Set()}
      onProcessToggle={() => {}}
      onProcessSolo={() => {}}
      kindItems={[
        { id: "request", label: "request", meta: 1 },
        { id: "response", label: "response", meta: 1 },
      ]}
      hiddenKinds={new Set()}
      onKindToggle={() => {}}
      onKindSolo={() => {}}
      scopeColorMode={scopeColorMode}
      onToggleProcessColorBy={() => {}}
      onToggleCrateColorBy={() => {}}
      subgraphScopeMode={subgraphScopeMode}
      onToggleProcessSubgraphs={() => {}}
      onToggleCrateSubgraphs={() => {}}
      showLoners={false}
      onToggleShowLoners={() => {}}
      scopeFilterLabel={null}
      onClearScopeFilter={() => {}}
      unionFrameLayout={undefined}
      graphFilterText={graphFilterText}
      onGraphFilterTextChange={setGraphFilterText}
      onHideNodeFilter={() => {}}
      onHideLocationFilter={() => {}}
    />
  );
}

describe("GraphPanel filter input interactions", () => {
  it("focus starts a new fragment instead of editing last token", async () => {
    const user = userEvent.setup();
    render(<Harness initialFilter="colorBy:crate groupBy:process loners:off" />);

    const input = screen.getByLabelText("Graph filter query") as HTMLInputElement;
    expect(input.value).toBe("");
    await user.click(input);
    expect(input.value).toBe("");

    await user.type(input, "hid");
    expect(input.value).toBe("hid");
  });

  it("supports two-stage hide autocomplete", async () => {
    const user = userEvent.setup();
    render(<Harness initialFilter="colorBy:crate groupBy:process loners:off" />);

    const input = screen.getByLabelText("Graph filter query") as HTMLInputElement;
    await user.click(input);
    await user.type(input, "hid");

    await user.click(screen.getByText("hide:"));
    expect(input.value).toBe("hide:");

    await user.type(input, "node");
    await user.click(screen.getByText("hide:node:"));
    expect(input.value).toBe("hide:node:");
  });
});
