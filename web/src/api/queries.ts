import { keepPreviousData, useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { api } from "./client";
import type { CreateFolderRequest } from "./generated/CreateFolderRequest";
import type { UpdateFolderRequest } from "./generated/UpdateFolderRequest";

export const keys = {
  health: ["health"] as const,
  folders: ["folders"] as const,
  folder: (id: string) => ["folders", id] as const,
  files: (id: string) => ["files", id] as const,
  browse: (path: string | undefined, hidden: boolean) => ["browse", path ?? "~", hidden] as const,
};

export const useHealth = () => useQuery({ queryKey: keys.health, queryFn: api.health });

export const useFolders = () => useQuery({ queryKey: keys.folders, queryFn: api.listFolders });

export const useFolder = (id: string) =>
  useQuery({ queryKey: keys.folder(id), queryFn: () => api.getFolder(id) });

export const useFiles = (id: string, opts: { includeDeleted: boolean; limit: number }) =>
  useQuery({
    queryKey: [...keys.files(id), opts],
    queryFn: () => api.listFiles(id, opts),
    placeholderData: keepPreviousData,
  });

export const useBrowse = (path: string | undefined, showHidden: boolean) =>
  useQuery({
    queryKey: keys.browse(path, showHidden),
    queryFn: () => api.browse(path, showHidden),
    retry: false,
  });

function useInvalidateFolders() {
  const qc = useQueryClient();
  return () => qc.invalidateQueries({ queryKey: keys.folders });
}

export function useCreateFolder() {
  const invalidate = useInvalidateFolders();
  return useMutation({
    mutationFn: (body: CreateFolderRequest) => api.createFolder(body),
    onSuccess: invalidate,
  });
}

export function useUpdateFolder(id: string) {
  const invalidate = useInvalidateFolders();
  return useMutation({
    mutationFn: (body: UpdateFolderRequest) => api.updateFolder(id, body),
    onSuccess: invalidate,
  });
}

export function useRemoveFolder() {
  const invalidate = useInvalidateFolders();
  return useMutation({ mutationFn: (id: string) => api.removeFolder(id), onSuccess: invalidate });
}

export function useRescanFolder() {
  return useMutation({ mutationFn: (id: string) => api.rescanFolder(id) });
}
