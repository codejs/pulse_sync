import type { FolderMode } from "@/api/generated/FolderMode";

export const MODE_LABELS: Record<FolderMode, string> = {
  sendReceive: "Send & Receive",
  sendOnly: "Send Only",
  receiveOnly: "Receive Only",
};

export const MODE_HINTS: Record<FolderMode, string> = {
  sendReceive: "Changes flow both ways.",
  sendOnly: "This device is the source of truth; remote changes are ignored.",
  receiveOnly: "A backup copy: remote changes apply, local edits are not sent.",
};
