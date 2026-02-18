import React, { createContext, useCallback, useContext, useEffect, useRef, useState } from "react";
import type { GraphGeometry } from "../geometry";
import { type Camera, cameraTransform } from "./camera";
import { useCameraController } from "./useCameraController";
import "./GraphCanvas.css";

interface CameraContextValue {
  camera: Camera;
  setCamera: (c: Camera) => void;
  fitView: () => void;
  viewportWidth: number;
  viewportHeight: number;
}

export const CameraContext = createContext<CameraContextValue | null>(null);

export function useCameraContext(): CameraContextValue {
  const ctx = useContext(CameraContext);
  if (!ctx) throw new Error("useCameraContext must be used inside GraphCanvas");
  return ctx;
}

interface GraphCanvasProps {
  geometry: GraphGeometry | null;
  children?: React.ReactNode;
  className?: string;
  onBackgroundClick?: () => void;
}

export function GraphCanvas({
  geometry,
  children,
  className,
  onBackgroundClick,
}: GraphCanvasProps) {
  const svgRef = useRef<SVGSVGElement>(null);
  const [viewportSize, setViewportSize] = useState({ width: 800, height: 600 });

  useEffect(() => {
    const svg = svgRef.current;
    if (!svg) return;
    const observer = new ResizeObserver((entries) => {
      for (const entry of entries) {
        const { width, height } = entry.contentRect;
        setViewportSize({ width, height });
      }
    });
    observer.observe(svg);
    return () => observer.disconnect();
  }, []);

  const { camera, setCamera, fitView, handlers } = useCameraController(
    svgRef,
    geometry?.bounds ?? null,
  );

  // Attach wheel listener as non-passive to allow preventDefault
  useEffect(() => {
    const svg = svgRef.current;
    if (!svg) return;
    svg.addEventListener("wheel", handlers.onWheel, { passive: false });
    return () => svg.removeEventListener("wheel", handlers.onWheel);
  }, [handlers.onWheel]);

  const handlePointerDown = useCallback(
    (e: React.PointerEvent<SVGSVGElement>) => {
      handlers.onPointerDown(e.nativeEvent);
    },
    [handlers],
  );

  const handlePointerMove = useCallback(
    (e: React.PointerEvent<SVGSVGElement>) => {
      handlers.onPointerMove(e.nativeEvent);
    },
    [handlers],
  );

  const handlePointerUp = useCallback(
    (e: React.PointerEvent<SVGSVGElement>) => {
      handlers.onPointerUp(e.nativeEvent);
    },
    [handlers],
  );

  const handleClick = useCallback(
    (e: React.MouseEvent<SVGSVGElement>) => {
      const target = e.target as Element;
      const svg = svgRef.current;
      if (!svg) return;
      if (target === svg || target.getAttribute("data-background") === "true") {
        onBackgroundClick?.();
      }
    },
    [onBackgroundClick],
  );

  const transform = cameraTransform(camera, viewportSize.width, viewportSize.height);

  return (
    <CameraContext.Provider
      value={{
        camera,
        setCamera,
        fitView,
        viewportWidth: viewportSize.width,
        viewportHeight: viewportSize.height,
      }}
    >
      <div className={`graph-canvas${className ? ` ${className}` : ""}`}>
        <svg
          ref={svgRef}
          className="graph-canvas__svg"
          onPointerDown={handlePointerDown}
          onPointerMove={handlePointerMove}
          onPointerUp={handlePointerUp}
          onClick={handleClick}
        >
          <rect
            width="100%"
            height="100%"
            fill="transparent"
            data-background="true"
          />
          <g transform={transform}>{children}</g>
        </svg>
      </div>
    </CameraContext.Provider>
  );
}
