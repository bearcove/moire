import "./RecordingTimeline.css";
import type { FrameSummary } from "../../api/types";

export function formatElapsed(ms: number): string {
  const totalSeconds = Math.floor(Math.abs(ms) / 1000);
  const minutes = Math.floor(totalSeconds / 60);
  const seconds = totalSeconds % 60;
  return `${minutes}:${String(seconds).padStart(2, "0")}`;
}

interface RecordingTimelineProps {
  frames: FrameSummary[];
  frameCount: number;
  currentFrameIndex: number;
  onScrub: (index: number) => void;
}

export function RecordingTimeline({ frames, frameCount, currentFrameIndex, onScrub }: RecordingTimelineProps) {
  const firstMs = frames[0]?.captured_at_unix_ms ?? 0;
  const currentMs = frames[currentFrameIndex]?.captured_at_unix_ms ?? firstMs;
  const elapsedMs = currentMs - firstMs;

  return (
    <div className="recording-timeline">
      <span className="recording-timeline-label">
        Frame {currentFrameIndex + 1} / {frameCount}
      </span>
      <input
        type="range"
        min={0}
        max={frameCount - 1}
        value={currentFrameIndex}
        onChange={(e) => onScrub(Number(e.target.value))}
        className="recording-timeline-slider"
      />
      <span className="recording-timeline-time">
        {formatElapsed(elapsedMs)}
      </span>
    </div>
  );
}
