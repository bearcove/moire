import { createLiveApiClient } from "./live";
import { createMockApiClient } from "./mock";
import type { ApiClient, ApiMode } from "./client";

const requestedMode = (import.meta.env.VITE_MOIRE_API_MODE as ApiMode | undefined) ?? "live";

function normalizeMode(mode: ApiMode | undefined): ApiMode {
  if (mode === "lab" || mode === "live") {
    return mode;
  }
  return "live";
}

export const apiMode: ApiMode = normalizeMode(requestedMode);

export const apiClient: ApiClient =
  apiMode === "lab" ? createMockApiClient() : createLiveApiClient();
