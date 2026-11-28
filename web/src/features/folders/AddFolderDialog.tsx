import { Plus } from "lucide-react";
import { type FormEvent, useState } from "react";
import type { FolderMode } from "@/api/generated/FolderMode";
import { useCreateFolder } from "@/api/queries";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
  DialogTrigger,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { DirectoryPicker } from "./DirectoryPicker";
import { MODE_HINTS, MODE_LABELS } from "./labels";

export function AddFolderDialog() {
  const [open, setOpen] = useState(false);
  const [path, setPath] = useState("");
  const [label, setLabel] = useState("");
  const [mode, setMode] = useState<FolderMode>("sendReceive");
  const create = useCreateFolder();

  const reset = () => {
    setPath("");
    setLabel("");
    setMode("sendReceive");
    create.reset();
  };

  const submit = (e: FormEvent) => {
    e.preventDefault();
    create.mutate(
      { path, label: label.trim() || undefined, mode },
      {
        onSuccess: () => {
          setOpen(false);
          reset();
        },
      },
    );
  };

  return (
    <Dialog
      open={open}
      onOpenChange={(next) => {
        setOpen(next);
        if (!next) reset();
      }}
    >
      <DialogTrigger asChild>
        <Button>
          <Plus /> Add folder
        </Button>
      </DialogTrigger>
      <DialogContent className="sm:max-w-xl">
        <DialogHeader>
          <DialogTitle>Add a folder</DialogTitle>
          <DialogDescription>
            Pick a folder on this device. Its contents are indexed now and will sync with paired
            devices once device pairing lands.
          </DialogDescription>
        </DialogHeader>

        <form onSubmit={submit} className="grid gap-5">
          {open && <DirectoryPicker value={path} onChange={setPath} />}

          <div className="grid gap-2">
            <Label htmlFor="folder-label">Label</Label>
            <Input
              id="folder-label"
              value={label}
              onChange={(e) => setLabel(e.target.value)}
              placeholder="Defaults to the folder name"
            />
          </div>

          <fieldset className="grid gap-2">
            <legend className="mb-2 text-sm font-medium">Mode</legend>
            {(Object.keys(MODE_LABELS) as FolderMode[]).map((m) => (
              <label
                key={m}
                className="flex cursor-pointer items-start gap-3 rounded-md border p-3 has-checked:border-primary has-checked:bg-primary/5"
              >
                <input
                  type="radio"
                  name="mode"
                  value={m}
                  checked={mode === m}
                  onChange={() => setMode(m)}
                  className="mt-0.5"
                />
                <span>
                  <span className="block text-sm font-medium">{MODE_LABELS[m]}</span>
                  <span className="block text-xs text-muted-foreground">{MODE_HINTS[m]}</span>
                </span>
              </label>
            ))}
          </fieldset>

          {create.isError && <p className="text-sm text-destructive">{create.error.message}</p>}

          <DialogFooter>
            <Button type="submit" disabled={!path || create.isPending}>
              {create.isPending ? "Adding…" : "Add folder"}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}
