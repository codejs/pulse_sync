import { AlertTriangle, CheckCircle2, Loader2, PauseCircle } from "lucide-react";
import type { FolderStatus } from "@/api/generated/FolderStatus";
import { Badge } from "@/components/ui/badge";

export function FolderStatusBadge({ status }: { status: FolderStatus }) {
  switch (status.state) {
    case "scanning":
      return (
        <Badge>
          <Loader2 className="animate-spin" /> Scanning
        </Badge>
      );
    case "paused":
      return (
        <Badge variant="secondary">
          <PauseCircle /> Paused
        </Badge>
      );
    case "error":
      return (
        <Badge variant="destructive" title={status.error ?? undefined}>
          <AlertTriangle /> Error
        </Badge>
      );
    case "idle":
      return status.error ? (
        <Badge variant="warning" title={status.error}>
          <AlertTriangle /> Up to date, with warnings
        </Badge>
      ) : (
        <Badge variant="success">
          <CheckCircle2 /> Up to date
        </Badge>
      );
  }
}
