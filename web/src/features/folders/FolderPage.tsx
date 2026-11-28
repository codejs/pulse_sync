import { Link, useNavigate, useParams } from "@tanstack/react-router";
import { ChevronLeft, FileIcon, FileX } from "lucide-react";
import { useState } from "react";
import { useFiles, useFolder } from "@/api/queries";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { formatBytes, formatCount, formatRelative } from "@/lib/format";
import { FolderActions } from "./FolderActions";
import { FolderStatusBadge } from "./FolderStatusBadge";
import { MODE_LABELS } from "./labels";

const PAGE = 200;

export function FolderPage() {
  const { folderId } = useParams({ from: "/folders/$folderId" });
  const navigate = useNavigate();
  const folder = useFolder(folderId);
  const [includeDeleted, setIncludeDeleted] = useState(false);
  const [limit, setLimit] = useState(PAGE);
  const files = useFiles(folderId, { includeDeleted, limit });

  if (folder.isError) {
    return (
      <Card className="p-6">
        <p className="text-sm text-destructive">{folder.error.message}</p>
        <Button asChild variant="link" className="px-0">
          <Link to="/">Back to folders</Link>
        </Button>
      </Card>
    );
  }
  const f = folder.data;
  if (!f) return <p className="text-sm text-muted-foreground">Loading…</p>;

  const list = files.data?.files ?? [];

  return (
    <div className="grid gap-6">
      <div>
        <Link
          to="/"
          className="inline-flex items-center gap-1 text-sm text-muted-foreground hover:text-foreground"
        >
          <ChevronLeft className="size-4" /> Folders
        </Link>
        <div className="mt-2 flex flex-wrap items-center gap-3">
          <h1 className="text-2xl font-semibold tracking-tight">{f.label}</h1>
          <FolderStatusBadge status={f.status} />
          <div className="ml-auto">
            <FolderActions folder={f} onRemoved={() => navigate({ to: "/" })} />
          </div>
        </div>
        <p className="font-mono text-xs text-muted-foreground">{f.path}</p>
        {f.status.error && <p className="mt-2 text-sm text-destructive">{f.status.error}</p>}
      </div>

      <div className="grid grid-cols-2 gap-3 sm:grid-cols-4">
        <Summary label="Files" value={formatCount(f.stats.files)} />
        <Summary label="Total size" value={formatBytes(f.stats.bytes)} />
        <Summary label="Last scan" value={formatRelative(f.status.lastScanAtMs)} />
        <Summary label="Mode" value={MODE_LABELS[f.mode]} />
      </div>

      <Card>
        <CardHeader className="flex-row items-center justify-between">
          <div className="grid gap-1">
            <CardTitle>Index</CardTitle>
            <CardDescription>
              Every file Pulse Sync knows about. Deleted files are kept as tombstones so deletions
              can reach other devices.
            </CardDescription>
          </div>
          <label className="flex shrink-0 items-center gap-2 text-sm">
            <input
              type="checkbox"
              checked={includeDeleted}
              onChange={(e) => setIncludeDeleted(e.target.checked)}
            />
            Show deleted ({formatCount(f.stats.deleted)})
          </label>
        </CardHeader>
        <CardContent className="px-0 pb-2">
          <table className="w-full text-sm">
            <thead className="border-y bg-muted/40 text-left text-xs text-muted-foreground">
              <tr>
                <th className="px-5 py-2 font-medium">Path</th>
                <th className="px-3 py-2 text-right font-medium">Size</th>
                <th className="hidden px-3 py-2 text-right font-medium sm:table-cell">Chunks</th>
                <th className="px-5 py-2 text-right font-medium">Modified</th>
              </tr>
            </thead>
            <tbody>
              {list.map((file) => (
                <tr
                  key={file.path}
                  className={file.deleted ? "text-muted-foreground line-through" : undefined}
                >
                  <td className="max-w-0 px-5 py-1.5">
                    <span className="flex items-center gap-2" title={file.hash || undefined}>
                      {file.deleted ? (
                        <FileX className="size-4 shrink-0" />
                      ) : (
                        <FileIcon className="size-4 shrink-0 text-muted-foreground" />
                      )}
                      <span className="truncate font-mono text-xs">{file.path}</span>
                    </span>
                  </td>
                  <td className="px-3 py-1.5 text-right tabular-nums">
                    {file.deleted ? "—" : formatBytes(file.size)}
                  </td>
                  <td className="hidden px-3 py-1.5 text-right tabular-nums sm:table-cell">
                    {file.deleted ? "—" : file.chunkCount}
                  </td>
                  <td className="px-5 py-1.5 text-right whitespace-nowrap text-muted-foreground">
                    {file.deleted ? "deleted" : formatRelative(file.modifiedMs)}
                  </td>
                </tr>
              ))}
              {files.isSuccess && list.length === 0 && (
                <tr>
                  <td colSpan={4} className="px-5 py-8 text-center text-muted-foreground">
                    {f.status.state === "scanning" ? "Scanning…" : "No files indexed."}
                  </td>
                </tr>
              )}
            </tbody>
          </table>
          {list.length >= limit && (
            <div className="flex justify-center p-3">
              <Button variant="outline" size="sm" onClick={() => setLimit((l) => l + PAGE)}>
                Show more
              </Button>
            </div>
          )}
        </CardContent>
      </Card>
    </div>
  );
}

function Summary({ label, value }: { label: string; value: string }) {
  return (
    <Card className="p-4">
      <p className="text-xs text-muted-foreground">{label}</p>
      <p className="mt-1 text-lg font-semibold tabular-nums">{value}</p>
    </Card>
  );
}
