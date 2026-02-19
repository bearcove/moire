// @vitest-environment jsdom
import React from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { Menu } from "./Menu";

afterEach(() => cleanup());

const nodeTypeItems = [
  { id: "show-kind", label: "Show only this kind" },
  { id: "hide-kind", label: "Hide this kind" },
  { id: "reset", label: "Reset filters", danger: true },
] as const;

const processItems = [
  { id: "open-resources", label: "Open in Resources" },
  { id: "show-process", label: "Show only this process" },
] as const;

function NodeTypesMenu({ onAction }: { onAction?: (id: string) => void }) {
  return <Menu label="Node types" items={nodeTypeItems} onAction={onAction} />;
}

describe("Menu — click interactions", () => {
  it("opens on click", async () => {
    const user = userEvent.setup();
    render(<NodeTypesMenu />);

    expect(screen.queryByRole("menuitem", { name: "Show only this kind" })).toBeNull();
    await user.click(screen.getByRole("button", { name: "Node types" }));
    expect(screen.getByRole("menuitem", { name: "Show only this kind" })).toBeTruthy();
  });

  it("closes when clicking the trigger again", async () => {
    const user = userEvent.setup();
    render(<NodeTypesMenu />);

    const trigger = screen.getByRole("button", { name: "Node types" });
    await user.click(trigger);
    expect(screen.getByRole("menuitem", { name: "Show only this kind" })).toBeTruthy();

    await user.click(trigger);
    expect(screen.queryByRole("menuitem", { name: "Show only this kind" })).toBeNull();
  });

  it("closes when clicking outside", async () => {
    const user = userEvent.setup();
    render(
      <div>
        <NodeTypesMenu />
        <button>Outside</button>
      </div>,
    );

    await user.click(screen.getByRole("button", { name: "Node types" }));
    expect(screen.getByRole("menuitem", { name: "Show only this kind" })).toBeTruthy();

    await user.click(screen.getByRole("button", { name: "Outside" }));
    await waitFor(() => {
      expect(screen.queryByRole("menuitem", { name: "Show only this kind" })).toBeNull();
    });
  });

  it("fires onAction and closes when clicking an item", async () => {
    const user = userEvent.setup();
    const onAction = vi.fn();
    render(<NodeTypesMenu onAction={onAction} />);

    await user.click(screen.getByRole("button", { name: "Node types" }));
    await user.click(screen.getByRole("menuitem", { name: "Hide this kind" }));

    expect(onAction).toHaveBeenCalledTimes(1);
    expect(onAction).toHaveBeenCalledWith("hide-kind");
    await waitFor(() => {
      expect(screen.queryByRole("menuitem", { name: "Hide this kind" })).toBeNull();
    });
  });
});

describe("Menu — drag-to-select (press, drag, release)", () => {
  it("opens on pointerdown before release", async () => {
    render(<NodeTypesMenu />);

    const trigger = screen.getByRole("button", { name: "Node types" });
    fireEvent.pointerDown(trigger, { button: 0, buttons: 1, pointerId: 1 });

    await waitFor(() => {
      expect(screen.getByRole("menuitem", { name: "Show only this kind" })).toBeTruthy();
    });
  });

  it("fires onAction when releasing over an item without a separate click", async () => {
    const onAction = vi.fn();
    render(<NodeTypesMenu onAction={onAction} />);

    const trigger = screen.getByRole("button", { name: "Node types" });
    fireEvent.pointerDown(trigger, { button: 0, buttons: 1, pointerId: 1 });

    const item = await screen.findByRole("menuitem", { name: "Hide this kind" });
    fireEvent.pointerEnter(item, { buttons: 1, pointerId: 1 });
    fireEvent.pointerUp(item, { button: 0, pointerId: 1 });

    expect(onAction).toHaveBeenCalledTimes(1);
    expect(onAction).toHaveBeenCalledWith("hide-kind");
    await waitFor(() => {
      expect(screen.queryByRole("menuitem", { name: "Hide this kind" })).toBeNull();
    });
  });

  it("does not fire onAction when releasing over an item after a normal click-open", async () => {
    const user = userEvent.setup();
    const onAction = vi.fn();
    render(<NodeTypesMenu onAction={onAction} />);

    // Normal click to open (press + release on trigger)
    await user.click(screen.getByRole("button", { name: "Node types" }));

    // Move to item and release — this is NOT a drag-open scenario, onAction should not fire
    // (onAction should only fire when the item is actually clicked, not just hovered)
    const item = screen.getByRole("menuitem", { name: "Hide this kind" });
    fireEvent.pointerEnter(item, { buttons: 0, pointerId: 1 });
    fireEvent.pointerUp(item, { button: 0, pointerId: 1 });

    expect(onAction).not.toHaveBeenCalled();
  });
});

describe("Menu — drag between two menus", () => {
  it("switches to a different menu when dragging onto its trigger", async () => {
    render(
      <div>
        <Menu label="Node types" items={nodeTypeItems} />
        <Menu label="Process" items={processItems} />
      </div>,
    );

    const trigger1 = screen.getByRole("button", { name: "Node types" });
    const trigger2 = screen.getByRole("button", { name: "Process" });

    // Press on first menu trigger — opens Node types
    fireEvent.pointerDown(trigger1, { button: 0, buttons: 1, pointerId: 1 });
    await waitFor(() => {
      expect(screen.getByRole("menuitem", { name: "Show only this kind" })).toBeTruthy();
    });

    // Drag to second menu trigger — should open Process and close Node types
    fireEvent.pointerEnter(trigger2, { buttons: 1, pointerId: 1 });
    await waitFor(() => {
      expect(screen.queryByRole("menuitem", { name: "Show only this kind" })).toBeNull();
      expect(screen.getByRole("menuitem", { name: "Open in Resources" })).toBeTruthy();
    });
  });
});
