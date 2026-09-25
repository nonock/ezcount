import { Alert, AlertDescription } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Field, FieldDescription, FieldGroup, FieldLabel } from "@/components/ui/field";
import { Spinner } from "@/components/ui/spinner";
import { Textarea } from "@/components/ui/textarea";
import { api } from "@/services/api";
import { useNativeFeatures } from "@/services/native";
import type { Group, SyncInfo } from "@/types";
import { errorMessage } from "@/utils/errors";
import { formatDateTime } from "@/utils/formatters";
import { CopyIcon, LockIcon, RefreshCwIcon, Share2Icon, TriangleAlertIcon } from "lucide-react";
import type React from "react";
import { useMemo, useState } from "react";
import { toast } from "sonner";
import { renderSVG } from "uqr";

interface ShareGroupModalProps {
  isOpen: boolean;
  onClose: () => void;
  group: Group;
  syncInfo: SyncInfo | null;
  onSyncNow: () => Promise<void>;
}

export const ShareGroupModal: React.FC<ShareGroupModalProps> = ({
  isOpen,
  onClose,
  group,
  syncInfo,
  onSyncNow,
}) => {
  const [busy, setBusy] = useState(false);
  const native = useNativeFeatures();
  const invite = syncInfo?.invite_code ?? null;
  // Dark on light whatever the theme: scanners expect it.
  const qrCode = useMemo(
    () =>
      invite &&
      `data:image/svg+xml;utf8,${encodeURIComponent(renderSVG(invite, { border: 2, blackColor: "#000", whiteColor: "#fff" }))}`,
    [invite]
  );
  const canShare = native.share || typeof navigator.share === "function";

  const handleSyncNow = async () => {
    setBusy(true);
    try {
      await onSyncNow();
    } finally {
      setBusy(false);
    }
  };

  const handleCopy = async () => {
    if (!invite) return;
    try {
      await navigator.clipboard.writeText(invite);
      toast.success("Invite link copied");
    } catch {
      document.getElementById("share-invite-code")?.focus();
      toast.info("Select the link and copy it manually");
    }
  };

  const handleShare = async () => {
    if (!invite) return;
    const title = `Join "${group.name}" on ezcount`;
    try {
      if (native.share) {
        await api.shareText(`${title}: ${invite}`, title);
      } else {
        await navigator.share({ title, text: `${title}:`, url: invite });
      }
    } catch (err) {
      // Closing the browser's share sheet without picking an app rejects too.
      if (err instanceof DOMException && err.name === "AbortError") return;
      toast.error("Could not share the invite", { description: errorMessage(err) });
    }
  };

  return (
    <Dialog open={isOpen} onOpenChange={(open) => !open && onClose()}>
      <DialogContent className="max-h-[calc(100dvh-2rem)] overflow-y-auto sm:max-w-md">
        <DialogHeader>
          <DialogTitle>Invite to Group</DialogTitle>
          <DialogDescription>
            Send this link to the people you want in "{group.name}", or let them scan the code from
            ezcount's Join with Code. Everyone can edit, even offline.
          </DialogDescription>
        </DialogHeader>

        {!invite || !qrCode ? (
          <div className="flex items-center justify-center gap-2 py-6 text-sm text-muted-foreground">
            <Spinner /> Loading…
          </div>
        ) : (
          <>
            <FieldGroup>
              <img
                src={qrCode}
                alt={`QR code of the invite to "${group.name}"`}
                className="mx-auto size-56 rounded-lg"
              />
              <Field>
                <FieldLabel htmlFor="share-invite-code">Invite link</FieldLabel>
                <Textarea
                  id="share-invite-code"
                  readOnly
                  rows={3}
                  value={invite}
                  onFocus={(e) => e.currentTarget.select()}
                  className="resize-none font-mono text-xs break-all"
                />
                <FieldDescription className="flex items-start gap-1.5">
                  <LockIcon className="mt-0.5 size-3.5 shrink-0" aria-hidden />
                  Anyone with this link can see and edit the group, so share it only with members.
                  Changes are end-to-end encrypted: the sync server cannot read them.
                </FieldDescription>
              </Field>

              <dl className="grid grid-cols-[auto_1fr] gap-x-4 gap-y-1 text-sm">
                <dt className="text-muted-foreground">Server</dt>
                <dd className="truncate text-right font-mono">{syncInfo?.server_url}</dd>
                <dt className="text-muted-foreground">Last synced</dt>
                <dd className="text-right">
                  {syncInfo?.last_synced_at ? formatDateTime(syncInfo.last_synced_at) : "Never"}
                </dd>
              </dl>

              {syncInfo?.last_error && (
                <Alert variant="destructive">
                  <TriangleAlertIcon />
                  <AlertDescription>Last sync failed: {syncInfo.last_error}</AlertDescription>
                </Alert>
              )}
            </FieldGroup>

            <DialogFooter>
              <Button variant="outline" onClick={handleSyncNow} disabled={busy}>
                {busy ? (
                  <Spinner data-icon="inline-start" />
                ) : (
                  <RefreshCwIcon data-icon="inline-start" />
                )}
                Sync Now
              </Button>
              <Button variant={canShare ? "outline" : "default"} onClick={handleCopy}>
                <CopyIcon data-icon="inline-start" />
                Copy Link
              </Button>
              {canShare && (
                <Button onClick={handleShare}>
                  <Share2Icon data-icon="inline-start" />
                  Share
                </Button>
              )}
            </DialogFooter>
          </>
        )}
      </DialogContent>
    </Dialog>
  );
};
