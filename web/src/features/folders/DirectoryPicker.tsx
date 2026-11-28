import { ArrowUp, Folder, FolderOpen, Home } from "lucide-react";
import { type FormEvent, useEffect, useState } from "react";
import { useBrowse } from "@/api/queries";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { cn } from "@/lib/utils";

/**
 * Browses directories through the daemon. Browsers can't expose real
 * filesystem paths, so a native folder picker won't work here.
 */
export function DirectoryPicker({
  value,
  onChange,
}: {
  value: string;
  onChange: (path: string) => void;
}) {
  const [cwd, setCwd] = useState<string | undefined>(undefined);
  const [showHidden, setShowHidden] = useState(false);
  const [typed, setTyped] = useState("");
  const browse = useBrowse(cwd, showHidden);

  // Selection follows the listing's canonical path, but only once the user has
  // navigated: the initial home-directory view must not be pre-selected.
  useEffect(() => {
    if (!browse.data) return;
    setTyped(browse.data.path);
    if (cwd !== undefined) onChange(browse.data.path);
  }, [browse.data, cwd, onChange]);

  const go = (e: FormEvent) => {
    e.preventDefault();
    if (typed.trim()) setCwd(typed.trim());
  };

  return (
    <div className="grid gap-2">
      <form onSubmit={go} className="flex gap-2">
        <Input
          value={typed}
          onChange={(e) => setTyped(e.target.value)}
          placeholder="/path/to/folder or ~/Documents"
          className="font-mono text-xs"
          aria-label="Folder path"
        />
        <Button type="submit" variant="outline">
          Go
        </Button>
      </form>

      <div className="flex items-center gap-1">
        <Button
          type="button"
          variant="ghost"
          size="sm"
          disabled={!browse.data?.parent}
          onClick={() => browse.data?.parent && setCwd(browse.data.parent)}
        >
          <ArrowUp /> Up
        </Button>
        <Button type="button" variant="ghost" size="sm" onClick={() => setCwd(browse.data?.home)}>
          <Home /> Home
        </Button>
        <label className="ml-auto flex items-center gap-2 text-xs text-muted-foreground">
          <input
            type="checkbox"
            checked={showHidden}
            onChange={(e) => setShowHidden(e.target.checked)}
          />
          Show hidden
        </label>
      </div>

      <div className="h-56 overflow-y-auto rounded-md border bg-muted/40 p-1">
        {browse.isError && <p className="p-3 text-sm text-destructive">{browse.error.message}</p>}
        {browse.isPending && <p className="p-3 text-sm text-muted-foreground">Loading…</p>}
        {browse.data?.entries.length === 0 && (
          <p className="p-3 text-sm text-muted-foreground">No subfolders.</p>
        )}
        {browse.data?.entries.map((entry) => (
          <button
            key={entry.path}
            type="button"
            onClick={() => setCwd(entry.path)}
            className={cn(
              "flex w-full items-center gap-2 rounded px-2 py-1.5 text-left text-sm hover:bg-card",
              value === entry.path && "bg-card",
            )}
          >
            <Folder className="size-4 shrink-0 text-primary" />
            <span className="truncate">{entry.name}</span>
          </button>
        ))}
      </div>

      {value && (
        <p className="flex items-center gap-2 text-xs text-muted-foreground">
          <FolderOpen className="size-3.5 shrink-0" />
          <span className="truncate font-mono">{value}</span>
        </p>
      )}
    </div>
  );
}
