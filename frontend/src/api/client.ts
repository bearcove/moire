import type {
  CutStatusResponse,
  RecordCurrentResponse,
  RecordingSessionInfo,
  RecordStartRequest,
  SqlResponse,
  SnapshotCutResponse,
  TriggerCutResponse,
} from "./types.generated";
import type { ConnectionsResponseWithTrace } from "./trace";

export type ApiMode = "live" | "lab";

export interface ApiClient {
  fetchConnections(): Promise<ConnectionsResponseWithTrace>;
  fetchSql(sql: string): Promise<SqlResponse>;
  triggerCut(): Promise<TriggerCutResponse>;
  fetchCutStatus(cutId: string): Promise<CutStatusResponse>;
  fetchExistingSnapshot(): Promise<SnapshotCutResponse | null>;
  fetchSnapshot(): Promise<SnapshotCutResponse>;
  startRecording(req?: RecordStartRequest): Promise<RecordingSessionInfo>;
  stopRecording(): Promise<RecordingSessionInfo>;
  fetchRecordingCurrent(): Promise<RecordCurrentResponse>;
  fetchRecordingFrame(frameIndex: number): Promise<SnapshotCutResponse>;
  exportRecording(): Promise<Blob>;
  importRecording(file: File): Promise<RecordingSessionInfo>;
}
