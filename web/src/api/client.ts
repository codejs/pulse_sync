// Thin typed wrapper over the daemon's REST API. Types come from Rust via ts-rs.
import type { BrowseResponse } from "./generated/BrowseResponse";
import type { CreateFolderRequest } from "./generated/CreateFolderRequest";
import type { ErrorResponse } from "./generated/ErrorResponse";
import type { FileListQuery } from "./generated/FileListQuery";
import type { FileListResponse } from "./generated/FileListResponse";
import type { FolderView } from "./generated/FolderView";
import type { HealthResponse } from "./generated/HealthResponse";
import type { UpdateFolderRequest } from "./generated/UpdateFolderRequest";

export class ApiError extends Error {
  constructor(
    readonly status: number,
    message: string,
  ) {
    super(message);
    this.name = "ApiError";
  }
}

type Query = Record<string, string | number | boolean | undefined>;

function withQuery(path: string, query?: Query): string {
  if (!query) return path;
  const params = new URLSearchParams();
  for (const [key, value] of Object.entries(query)) {
    if (value !== undefined && value !== "") params.set(key, String(value));
  }
  const qs = params.toString();
  return qs ? `${path}?${qs}` : path;
}

async function request<T>(method: string, path: string, body?: unknown): Promise<T> {
  const res = await fetch(path, {
    method,
    credentials: "same-origin",
    headers: body === undefined ? undefined : { "Content-Type": "application/json" },
    body: body === undefined ? undefined : JSON.stringify(body),
  });
  if (!res.ok) {
    let message = `${res.status} ${res.statusText}`;
    try {
      message = ((await res.json()) as ErrorResponse).error;
    } catch {
      // not JSON; keep the status line
    }
    throw new ApiError(res.status, message);
  }
  if (res.status === 204 || res.status === 202) return undefined as T;
  return (await res.json()) as T;
}

export const api = {
  health: () => request<HealthResponse>("GET", "/api/health"),

  listFolders: () => request<FolderView[]>("GET", "/api/folders"),
  getFolder: (id: string) => request<FolderView>("GET", `/api/folders/${id}`),
  createFolder: (body: CreateFolderRequest) => request<FolderView>("POST", "/api/folders", body),
  updateFolder: (id: string, body: UpdateFolderRequest) =>
    request<FolderView>("PATCH", `/api/folders/${id}`, body),
  removeFolder: (id: string) => request<void>("DELETE", `/api/folders/${id}`),
  rescanFolder: (id: string) => request<void>("POST", `/api/folders/${id}/rescan`),
  listFiles: (id: string, query: FileListQuery) =>
    request<FileListResponse>("GET", withQuery(`/api/folders/${id}/files`, query)),

  browse: (path?: string, showHidden?: boolean) =>
    request<BrowseResponse>("GET", withQuery("/api/fs/browse", { path, showHidden })),
};
