import { revealItemInDir } from "@tauri-apps/plugin-opener";
import { toast } from "svelte-sonner";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { api } from "@/services/api";
import { group } from "@/test/fixtures";
import { pressAction } from "@/test/toast";
import { openGroup } from "../state/groups.svelte";
import { exportGroup, exportGroupPdf } from "./files";

// What the device can do, as `native.svelte` reports it.
const device = vi.hoisted(() => ({
  android: false,
  features: { share: false, scan: false, save: false },
}));

vi.mock("@/services/api", () => ({
  api: {
    exportGroupCsv: vi.fn(),
    shareText: vi.fn(),
    shareFile: vi.fn(),
    saveDownload: vi.fn(),
    saveFile: vi.fn(),
  },
}));
vi.mock("@/services/native.svelte", () => ({
  get isAndroid() {
    return device.android;
  },
  nativeFeatures: device.features,
  closeTopLayer: () => false,
}));
vi.mock("@tauri-apps/plugin-opener", () => ({ revealItemInDir: vi.fn() }));
vi.mock("svelte-sonner", () => ({ toast: { success: vi.fn(), error: vi.fn() } }));
vi.mock("../report", () => ({ groupReport: vi.fn(async () => new Uint8Array([37, 80, 68, 70])) }));

/** The file the page handed to the browser's downloads. */
let downloaded: { name: string; file: Blob } | null;

beforeEach(() => {
  vi.clearAllMocks();
  device.android = false;
  device.features.save = false;
  downloaded = null;
  openGroup.group = group({ name: "Trip: Rome/Paris?" });
  vi.mocked(api.exportGroupCsv).mockResolvedValue("Date,Title");
  let file: Blob;
  vi.spyOn(URL, "createObjectURL").mockImplementation((blob) => {
    file = blob as Blob;
    return "blob:export";
  });
  vi.spyOn(URL, "revokeObjectURL").mockImplementation(() => {});
  vi.spyOn(HTMLAnchorElement.prototype, "click").mockImplementation(function (
    this: HTMLAnchorElement
  ) {
    downloaded = { name: this.download, file };
  });
});

describe("exportGroup", () => {
  it("downloads the CSV file in a browser, named after the group", async () => {
    await exportGroup();
    expect(downloaded?.name).toBe("Trip_ Rome_Paris_.csv");
    // The byte order mark makes Excel read the accents right.
    expect(await downloaded?.file.text()).toBe("\uFEFFDate,Title");
    expect(URL.revokeObjectURL).toHaveBeenCalledWith("blob:export");
    expect(toast.success).toHaveBeenCalled();
  });

  it("saves it in the Downloads folder on a computer, with a way to it", async () => {
    device.features.save = true;
    vi.mocked(api.saveDownload).mockResolvedValue("C:/Downloads/Trip.csv");
    await exportGroup();
    expect(api.saveDownload).toHaveBeenCalledWith("Trip_ Rome_Paris_.csv", "\uFEFFDate,Title");
    const [, options] = vi.mocked(toast.success).mock.calls[0];
    expect(options?.description).toBe("C:/Downloads/Trip.csv");
    pressAction(options);
    expect(revealItemInDir).toHaveBeenCalledWith("C:/Downloads/Trip.csv");
    expect(downloaded).toBeNull();
  });

  it("hands it to Android's share sheet", async () => {
    device.android = true;
    await exportGroup();
    expect(api.shareText).toHaveBeenCalledWith("Date,Title", "Trip_ Rome_Paris_.csv");
    expect(downloaded).toBeNull();
  });

  it("says why it failed, and does nothing without an open group", async () => {
    vi.mocked(api.exportGroupCsv).mockRejectedValue(new Error("Group not found"));
    await exportGroup();
    expect(toast.error).toHaveBeenCalledWith(expect.any(String), {
      description: "Group not found",
    });

    openGroup.group = null;
    await exportGroup();
    expect(api.exportGroupCsv).toHaveBeenCalledTimes(1);
  });
});

describe("exportGroupPdf", () => {
  it("downloads the summary in a browser", async () => {
    await exportGroupPdf();
    expect(downloaded?.name).toBe("Trip_ Rome_Paris_.pdf");
    expect(downloaded?.file.type).toBe("application/pdf");
  });

  it("saves it on a computer and shares it on Android", async () => {
    device.features.save = true;
    vi.mocked(api.saveFile).mockResolvedValue("C:/Downloads/Trip.pdf");
    await exportGroupPdf();
    expect(api.saveFile).toHaveBeenCalledWith("Trip_ Rome_Paris_.pdf", expect.any(Uint8Array));

    device.android = true;
    await exportGroupPdf();
    expect(api.shareFile).toHaveBeenCalledWith(
      "Trip_ Rome_Paris_.pdf",
      "application/pdf",
      expect.any(Uint8Array)
    );
  });
});
