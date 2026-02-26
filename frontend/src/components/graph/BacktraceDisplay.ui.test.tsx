// @vitest-environment jsdom
import React from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, render } from "@testing-library/react";
import { BacktraceDisplay } from "./BacktraceDisplay";
import type { GraphFrameData } from "./graphNodeData";

vi.mock("./GraphNode", () => ({
  FrameLine: ({ frame, active }: { frame: GraphFrameData; active?: boolean }) => (
    <div className={`graph-node-frame-section${active ? " graph-node-frame-section--active" : ""}`}>
      <div className="graph-node-frame-block__line--target">{frame.function_name}</div>
    </div>
  ),
}));

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

function rect({
  top,
  left = 0,
  width = 0,
  height = 0,
}: {
  top: number;
  left?: number;
  width?: number;
  height?: number;
}): DOMRect {
  return {
    x: left,
    y: top,
    top,
    left,
    width,
    height,
    right: left + width,
    bottom: top + height,
    toJSON: () => ({}),
  } as DOMRect;
}

describe("BacktraceDisplay scroll centering", () => {
  it("centers the highlighted line in container coordinates even when graph is zoomed", () => {
    vi.spyOn(HTMLElement.prototype, "getBoundingClientRect").mockImplementation(function (
      this: HTMLElement,
    ) {
      if (this.classList.contains("graph-node-frames-scroll")) {
        return rect({ top: 0, width: 400, height: 100 });
      }
      if (this.classList.contains("graph-node-frame-block__line--target")) {
        const isActive = this.closest(".graph-node-frame-section--active") != null;
        if (isActive) return rect({ top: 50, width: 350, height: 10 });
      }
      return rect({ top: 0 });
    });

    const frames: GraphFrameData[] = [
      { function_name: "frame0", source_file: "a.rs", frame_id: 1 },
      { function_name: "frame1", source_file: "a.rs", frame_id: 2 },
    ];

    const { container, rerender } = render(
      <div className="graph-node-frames-scroll">
        <BacktraceDisplay
          frames={frames}
          allFrames={frames}
          framesLoading={false}
          activeFrameIndex={0}
        />
      </div>,
    );

    const scrollContainer = container.querySelector(".graph-node-frames-scroll");
    if (!(scrollContainer instanceof HTMLElement)) {
      throw new Error("expected .graph-node-frames-scroll");
    }

    Object.defineProperty(scrollContainer, "clientHeight", { value: 200, configurable: true });
    Object.defineProperty(scrollContainer, "scrollHeight", { value: 1000, configurable: true });
    scrollContainer.scrollTop = 20;

    rerender(
      <div className="graph-node-frames-scroll">
        <BacktraceDisplay
          frames={frames}
          allFrames={frames}
          framesLoading={false}
          activeFrameIndex={1}
        />
      </div>,
    );

    // Expected target: 20 + ((50 + 10/2) / 0.5) - 200/2 = 30
    expect(scrollContainer.scrollTop).toBe(30);
  });
});
