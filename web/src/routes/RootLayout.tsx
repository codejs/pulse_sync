import { Link, Outlet } from "@tanstack/react-router";
import { Activity, FolderSync, MonitorSmartphone } from "lucide-react";
import { type Connection, useLiveEvents } from "@/api/events";
import { useHealth } from "@/api/queries";
import { cn } from "@/lib/utils";

export function RootLayout() {
  const connection = useLiveEvents();
  const health = useHealth();

  return (
    <div className="min-h-dvh">
      <header className="border-b bg-card/70 backdrop-blur">
        <div className="mx-auto flex h-14 max-w-6xl items-center gap-6 px-4">
          <Link to="/" className="flex items-center gap-2 font-semibold">
            <span className="grid size-7 place-items-center rounded-md bg-primary text-primary-foreground">
              <Activity className="size-4" />
            </span>
            Pulse Sync
          </Link>
          <nav className="flex items-center gap-1 text-sm">
            <Link
              to="/"
              className="flex items-center gap-2 rounded-md px-3 py-1.5 text-muted-foreground hover:bg-muted hover:text-foreground"
              activeProps={{ className: "bg-muted text-foreground" }}
              activeOptions={{ exact: false }}
            >
              <FolderSync className="size-4" /> Folders
            </Link>
            <span
              className="flex cursor-not-allowed items-center gap-2 rounded-md px-3 py-1.5 text-muted-foreground/60"
              title="Device pairing arrives in M2"
            >
              <MonitorSmartphone className="size-4" /> Devices
            </span>
          </nav>
          <div className="ml-auto flex items-center gap-4 text-xs text-muted-foreground">
            {health.data && (
              <span className="hidden font-mono md:inline" title={health.data.deviceId}>
                {health.data.deviceId.slice(0, 7)}…
              </span>
            )}
            <ConnectionDot connection={connection} />
          </div>
        </div>
      </header>
      <main className="mx-auto max-w-6xl px-4 py-8">
        <Outlet />
      </main>
    </div>
  );
}

const CONNECTION: Record<Connection, { label: string; className: string }> = {
  open: { label: "Live", className: "bg-success" },
  connecting: { label: "Connecting", className: "bg-warning animate-pulse" },
  closed: { label: "Daemon offline", className: "bg-destructive" },
};

function ConnectionDot({ connection }: { connection: Connection }) {
  const c = CONNECTION[connection];
  return (
    <span className="flex items-center gap-2">
      <span className={cn("size-2 rounded-full", c.className)} />
      {c.label}
    </span>
  );
}
