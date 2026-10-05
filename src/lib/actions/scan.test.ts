import { toast } from "svelte-sonner";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { api } from "@/services/api";
import { ScanCancelled, scanQrCode } from "@/services/native.svelte";
import { group } from "@/test/fixtures";
import { dialogs } from "../state/dialogs.svelte";
import { openGroup } from "../state/groups.svelte";
import { navigation } from "../state/navigation.svelte";
import { scanInvite, sendInviteToScanned, sendLoginToScanned } from "./scan";

vi.mock("@/services/api", () => ({
  api: {
    getGroups: vi.fn().mockResolvedValue([]),
    joinGroup: vi.fn(),
    sendGroupInvite: vi.fn(),
    sendLogin: vi.fn(),
  },
}));
vi.mock("@/services/native.svelte", () => {
  class ScanCancelled extends Error {}
  return { ScanCancelled, scanQrCode: vi.fn(), closeTopLayer: () => false };
});
vi.mock("svelte-sonner", () => ({ toast: { success: vi.fn(), error: vi.fn() } }));

beforeEach(() => {
  vi.clearAllMocks();
  window.history.replaceState(null, "");
  navigation.groupId = null;
  openGroup.group = group();
  dialogs.join = { open: true, code: "", error: null };
  dialogs.share = true;
  dialogs.linkDevice = true;
  dialogs.scanning = false;
});

describe("scanInvite", () => {
  it("joins the group scanned right away", async () => {
    vi.mocked(scanQrCode).mockResolvedValue("invite");
    vi.mocked(api.joinGroup).mockResolvedValue(group({ id: "joined" }));
    const done = scanInvite();
    expect(dialogs.join.open).toBe(false);
    expect(dialogs.scanning).toBe(true);
    await done;
    expect(dialogs.scanning).toBe(false);
    expect(api.joinGroup).toHaveBeenCalledWith("invite");
    expect(navigation.groupId).toBe("joined");
  });

  it("returns to the join dialog when the scan is cancelled", async () => {
    vi.mocked(scanQrCode).mockRejectedValue(new ScanCancelled());
    await scanInvite();
    expect(dialogs.join).toEqual({ open: true, code: "", error: null });
    expect(dialogs.scanning).toBe(false);
  });

  it("says why the camera couldn't scan", async () => {
    vi.mocked(scanQrCode).mockRejectedValue(new Error("The camera is needed"));
    await scanInvite();
    expect(dialogs.join).toEqual({ open: true, code: "", error: "The camera is needed" });
  });

  it("keeps the code scanned when joining with it fails", async () => {
    vi.mocked(scanQrCode).mockResolvedValue("invite");
    vi.mocked(api.joinGroup).mockRejectedValue(new Error("Group not found"));
    await scanInvite();
    expect(dialogs.join).toEqual({ open: true, code: "invite", error: "Group not found" });
  });
});

describe("sendInviteToScanned", () => {
  it("sends the open group's invite to the device whose code was scanned", async () => {
    vi.mocked(scanQrCode).mockResolvedValue("ezcount://receive?code=1");
    await sendInviteToScanned();
    expect(dialogs.share).toBe(false);
    expect(api.sendGroupInvite).toHaveBeenCalledWith("g1", "ezcount://receive?code=1");
    expect(toast.success).toHaveBeenCalled();
  });

  it("sends nothing when the scan is cancelled, without complaining", async () => {
    vi.mocked(scanQrCode).mockRejectedValue(new ScanCancelled());
    await sendInviteToScanned();
    expect(api.sendGroupInvite).not.toHaveBeenCalled();
    expect(toast.error).not.toHaveBeenCalled();
    expect(dialogs.scanning).toBe(false);
  });

  it("says when the scan or the sending failed", async () => {
    vi.mocked(scanQrCode).mockRejectedValueOnce(new Error("The camera is needed"));
    await sendInviteToScanned();
    expect(toast.error).toHaveBeenCalledWith("The camera is needed");

    vi.mocked(scanQrCode).mockResolvedValue("code");
    vi.mocked(api.sendGroupInvite).mockRejectedValue(new Error("This group is not shared yet"));
    await sendInviteToScanned();
    expect(toast.error).toHaveBeenLastCalledWith(expect.any(String), {
      description: "This group is not shared yet",
    });
  });
});

describe("sendLoginToScanned", () => {
  it("logs the device scanned into the account", async () => {
    vi.mocked(scanQrCode).mockResolvedValue("code");
    await sendLoginToScanned("hunter2");
    expect(dialogs.linkDevice).toBe(false);
    expect(api.sendLogin).toHaveBeenCalledWith("code", "hunter2");
    expect(toast.success).toHaveBeenCalled();
  });

  it("says why it couldn't", async () => {
    vi.mocked(scanQrCode).mockResolvedValue("code");
    vi.mocked(api.sendLogin).mockRejectedValue(new Error("Wrong password"));
    await sendLoginToScanned("nope");
    expect(toast.error).toHaveBeenCalledWith(expect.any(String), { description: "Wrong password" });
  });
});
