import { Link } from "@tanstack/react-router";
import { FolderSync } from "lucide-react";
import { useFolders } from "@/api/queries";
import { Card } from "@/components/ui/card";
import { formatBytes, formatCount, formatRelative } from "@/lib/format";
import { AddFolderDialog } from "./AddFolderDialog";
import { FolderActions } from "./FolderActions";
import { FolderStatusBadge } from "./FolderStatusBadge";
import { MODE_LABELS } from "./labels";

export function FoldersPage() {
  const folders = useFolders();

  return (
    <div className="grid gap-6">
      <div className="flex items-end justify-between gap-4">
        <div>
          <h1 className="text-2xl font-semibold tracking-tight">Folders</h1>
          <p className="text-sm text-muted-foreground">
            Folders on this device that Pulse Sync tracks.
          </p>
        </div>
        <AddFolderDialog />
      </div>

      {folders.isError && (
        <Card className="p-5 text-sm text-destructive">
          Cannot reach the daemon: {folders.error.message}
        </Card>
      )}

      {folders.data?.length === 0 && (
        <Card className="flex flex-col items-center gap-3 p-12 text-center">
          <FolderSync className="size-10 text-primary" />
          <div>
            <p className="font-medium">No folders yet</p>
            <p className="text-sm text-muted-foreground">
              Add a folder to start indexing it for sync.
            </p>
          </div>
        </Card>
      )}

      <div className="grid gap-3">
        {folders.data?.map((f) => (
          <Card key={f.id} className="flex flex-wrap items-center gap-x-6 gap-y-3 p-4">
            <Link
              to="/folders/$folderId"
              params={{ folderId: f.id }}
              className="min-w-0 flex-1 basis-64 rounded-md outline-none focus-visible:ring-[3px] focus-visible:ring-ring/50"
            >
              <div className="flex items-center gap-2">
                <span className="truncate font-medium hover:underline">{f.label}</span>
                <FolderStatusBadge status={f.status} />
              </div>
              <p className="truncate font-mono text-xs text-muted-foreground">{f.path}</p>
              {f.status.error && <p className="mt-1 text-xs text-destructive">{f.status.error}</p>}
            </Link>
            <dl className="grid grid-cols-3 gap-6 text-sm">
              <Stat label="Files" value={formatCount(f.stats.files)} />
              <Stat label="Size" value={formatBytes(f.stats.bytes)} />
              <Stat label="Scanned" value={formatRelative(f.status.lastScanAtMs)} />
            </dl>
            <span className="text-xs text-muted-foreground">{MODE_LABELS[f.mode]}</span>
            <FolderActions folder={f} />
          </Card>
        ))}
      </div>
    </div>
  );
}

function Stat({ label, value }: { label: string; value: string }) {
  return (
    <div>
      <dt className="text-xs text-muted-foreground">{label}</dt>
      <dd className="font-medium tabular-nums">{value}</dd>
    </div>
  );
}
