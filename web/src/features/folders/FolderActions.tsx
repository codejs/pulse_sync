import { Pause, Play, RefreshCw } from "lucide-react";
import type { FolderView } from "@/api/generated/FolderView";
import { useRescanFolder, useUpdateFolder } from "@/api/queries";
import { Button } from "@/components/ui/button";
import { RemoveFolderDialog } from "./RemoveFolderDialog";

export function FolderActions({
  folder,
  onRemoved,
}: {
  folder: FolderView;
  onRemoved?: () => void;
}) {
  const update = useUpdateFolder(folder.id);
  const rescan = useRescanFolder();

  return (
    <div className="flex items-center gap-1">
      <Button
        variant="ghost"
        size="icon-sm"
        title="Rescan now"
        disabled={folder.paused || folder.status.state === "scanning"}
        onClick={() => rescan.mutate(folder.id)}
      >
        <RefreshCw />
        <span className="sr-only">Rescan</span>
      </Button>
      <Button
        variant="ghost"
        size="icon-sm"
        title={folder.paused ? "Resume" : "Pause"}
        disabled={update.isPending}
        onClick={() => update.mutate({ paused: !folder.paused })}
      >
        {folder.paused ? <Play /> : <Pause />}
        <span className="sr-only">{folder.paused ? "Resume" : "Pause"}</span>
      </Button>
      <RemoveFolderDialog folder={folder} onRemoved={onRemoved} />
    </div>
  );
}
