import { Trash2 } from "lucide-react";
import { useState } from "react";
import type { FolderView } from "@/api/generated/FolderView";
import { useRemoveFolder } from "@/api/queries";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogClose,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
  DialogTrigger,
} from "@/components/ui/dialog";

export function RemoveFolderDialog({
  folder,
  onRemoved,
}: {
  folder: FolderView;
  onRemoved?: () => void;
}) {
  const [open, setOpen] = useState(false);
  const remove = useRemoveFolder();

  return (
    <Dialog open={open} onOpenChange={setOpen}>
      <DialogTrigger asChild>
        <Button variant="ghost" size="icon-sm" title="Remove folder">
          <Trash2 />
          <span className="sr-only">Remove folder</span>
        </Button>
      </DialogTrigger>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>Stop syncing “{folder.label}”?</DialogTitle>
          <DialogDescription>
            Pulse Sync forgets this folder and its index. The files in{" "}
            <span className="font-mono">{folder.path}</span> stay on disk.
          </DialogDescription>
        </DialogHeader>
        {remove.isError && <p className="text-sm text-destructive">{remove.error.message}</p>}
        <DialogFooter>
          <DialogClose asChild>
            <Button variant="outline">Cancel</Button>
          </DialogClose>
          <Button
            variant="destructive"
            disabled={remove.isPending}
            onClick={() =>
              remove.mutate(folder.id, {
                onSuccess: () => {
                  setOpen(false);
                  onRemoved?.();
                },
              })
            }
          >
            Remove folder
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
