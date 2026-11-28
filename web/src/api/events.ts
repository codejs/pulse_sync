import { useQueryClient } from "@tanstack/react-query";
import { useEffect, useState } from "react";
import type { Event } from "./generated/Event";
import type { FolderView } from "./generated/FolderView";
import { keys } from "./queries";

export type Connection = "connecting" | "open" | "closed";

/**
 * Subscribes to the daemon's event stream and keeps the query cache fresh.
 * Reconnects with backoff; refetches everything after a reconnect since
 * events may have been missed.
 */
export function useLiveEvents(): Connection {
  const qc = useQueryClient();
  const [connection, setConnection] = useState<Connection>("connecting");

  useEffect(() => {
    let socket: WebSocket | undefined;
    let retry: ReturnType<typeof setTimeout> | undefined;
    let attempt = 0;
    let disposed = false;

    const handle = (event: Event) => {
      switch (event.type) {
        case "foldersChanged":
          qc.invalidateQueries({ queryKey: keys.folders });
          break;
        case "folderStatusChanged": {
          const patch = (f: FolderView) =>
            f.id === event.folderId ? { ...f, status: event.status } : f;
          qc.setQueryData<FolderView[]>(keys.folders, (list) => list?.map(patch));
          qc.setQueryData<FolderView>(keys.folder(event.folderId), (f) => f && patch(f));
          break;
        }
        case "indexUpdated":
          qc.invalidateQueries({ queryKey: keys.folders });
          qc.invalidateQueries({ queryKey: keys.files(event.folderId) });
          break;
      }
    };

    const connect = () => {
      const scheme = location.protocol === "https:" ? "wss" : "ws";
      socket = new WebSocket(`${scheme}://${location.host}/api/events`);
      setConnection("connecting");
      socket.onopen = () => {
        if (attempt > 0) qc.invalidateQueries();
        attempt = 0;
        setConnection("open");
      };
      socket.onmessage = (msg) => {
        try {
          handle(JSON.parse(msg.data as string) as Event);
        } catch (err) {
          console.warn("bad event", err);
        }
      };
      socket.onclose = () => {
        if (disposed) return;
        setConnection("closed");
        attempt += 1;
        retry = setTimeout(connect, Math.min(1000 * 2 ** (attempt - 1), 15_000));
      };
    };

    connect();
    return () => {
      disposed = true;
      clearTimeout(retry);
      socket?.close();
    };
  }, [qc]);

  return connection;
}
