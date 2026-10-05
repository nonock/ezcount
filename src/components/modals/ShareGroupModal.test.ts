import { fireEvent, render, screen } from "@testing-library/svelte";
import { toast } from "svelte-sonner";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { t } from "@/lib/i18n/index.svelte";
import { dialogs } from "@/lib/state/dialogs.svelte";
import { openGroup } from "@/lib/state/groups.svelte";
import { api } from "@/services/api";
import { group } from "@/test/fixtures";
import type { SyncInfo } from "@/types";
import ShareGroupModal from "./ShareGroupModal.svelte";

// What the device can do, as `native.svelte` reports it.
const device = vi.hoisted(() => ({ features: { share: false, scan: false, save: false } }));

vi.mock("@/services/api", () => ({ api: { shareText: vi.fn() } }));
vi.mock("@/services/native.svelte", () => ({
  nativeFeatures: device.features,
  closeTopLayer: () => false,
}));
vi.mock("@/lib/actions/scan", () => ({ sendInviteToScanned: vi.fn() }));
vi.mock("svelte-sonner", () => ({ toast: { success: vi.fn(), info: vi.fn(), error: vi.fn() } }));

const INVITE = "https://relay.example.com/join#v=2&g=g1&k=secret";
const shared: SyncInfo = {
  group_id: "g1",
  enabled: true,
  server_url: "https://relay.example.com",
  invite_code: INVITE,
  last_synced_at: null,
  last_error: null,
};

beforeEach(() => {
  vi.clearAllMocks();
  device.features.share = false;
  device.features.scan = false;
  openGroup.group = group();
  openGroup.syncInfo = shared;
  dialogs.share = true;
});

describe("ShareGroupModal", () => {
  it("waits for the invite", async () => {
    openGroup.syncInfo = null;
    render(ShareGroupModal, { group: group() });
    const dialog = await screen.findByRole("dialog");
    expect(dialog.textContent).toContain(t("common.loading"));
    expect(screen.queryByAltText(t("share.qrAlt", "Trip"))).toBeNull();
  });

  it("shows the invite as a link and as a QR code", async () => {
    render(ShareGroupModal, { group: group() });
    const link = (await screen.findByLabelText(t("join.link"))) as HTMLTextAreaElement;
    expect(link.value).toBe(INVITE);
    expect(link.readOnly).toBe(true);
    expect(screen.getByAltText(t("share.qrAlt", "Trip")).getAttribute("src")).toMatch(
      /^data:image\/svg\+xml/
    );
    expect((await screen.findByRole("dialog")).textContent).toContain(t("share.never"));
  });

  it("copies the link", async () => {
    const writeText = vi.fn().mockResolvedValue(undefined);
    Object.defineProperty(navigator, "clipboard", { value: { writeText }, configurable: true });
    render(ShareGroupModal, { group: group() });
    await fireEvent.click(await screen.findByRole("button", { name: t("share.copy") }));
    expect(writeText).toHaveBeenCalledWith(INVITE);
    await vi.waitFor(() => expect(toast.success).toHaveBeenCalledWith(t("share.copied")));
  });

  it("hands the invite to the phone's share sheet where there is one", async () => {
    device.features.share = true;
    render(ShareGroupModal, { group: group() });
    await fireEvent.click(await screen.findByRole("button", { name: t("share.share") }));
    const title = t("share.message", "Trip");
    expect(api.shareText).toHaveBeenCalledWith(`${title}: ${INVITE}`, title);
  });

  it("syncs when asked, and says why the last sync failed", async () => {
    const syncNow = vi.spyOn(openGroup, "syncNow").mockResolvedValue();
    openGroup.syncInfo = { ...shared, last_error: "The sync server does not know this group" };
    render(ShareGroupModal, { group: group() });
    const dialog = await screen.findByRole("dialog");
    expect(dialog.textContent).toContain("The sync server does not know this group");
    await fireEvent.click(screen.getByRole("button", { name: t("share.syncNow") }));
    expect(syncNow).toHaveBeenCalled();
  });
});
