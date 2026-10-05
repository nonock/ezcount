// The open group as a file: CSV or PDF, saved or shared as the device can.

import { revealItemInDir } from "@tauri-apps/plugin-opener";
import { toast } from "svelte-sonner";
import { api } from "@/services/api";
import { isAndroid, nativeFeatures } from "@/services/native.svelte";
import { errorMessage } from "@/utils/errors";
import { t } from "../i18n/index.svelte";
import { openGroup } from "../state/groups.svelte";

/**
 * Saves the open group as a CSV file and says where it went: the Downloads folder on a
 * computer, the browser's downloads in the web version. On Android it goes to the share sheet.
 */
export async function exportGroup() {
  const group = openGroup.group;
  if (!group) return;
  try {
    const csv = await api.exportGroupCsv(group.id);
    const fileName = `${fileNameOf(group.name)}.csv`;
    if (isAndroid) {
      // Android's web view can't download what the page makes.
      await api.shareText(csv, fileName);
      return;
    }
    // The byte order mark makes Excel read the accents right.
    const text = `\uFEFF${csv}`;
    if (nativeFeatures.save) {
      saved(await api.saveDownload(fileName, text));
      return;
    }
    download(fileName, new Blob([text], { type: "text/csv" }));
  } catch (err) {
    toast.error(t("groups.exportFailed"), { description: errorMessage(err) });
  }
}

/**
 * Saves the open group's summary (balances, who pays whom, expenses) as a PDF file, where
 * `exportGroup` puts its CSV file. On Android it goes to the share sheet, as a file.
 */
export async function exportGroupPdf() {
  const group = openGroup.group;
  if (!group) return;
  try {
    // The library and its fonts are only loaded when a report is asked for.
    const { groupReport } = await import("../report");
    const pdf = await groupReport(group, openGroup.balances, openGroup.settlements);
    const fileName = `${fileNameOf(group.name)}.pdf`;
    if (isAndroid) {
      await api.shareFile(fileName, "application/pdf", pdf);
    } else if (nativeFeatures.save) {
      saved(await api.saveFile(fileName, pdf));
    } else {
      download(fileName, new Blob([pdf as BlobPart], { type: "application/pdf" }));
    }
  } catch (err) {
    toast.error(t("groups.exportFailed"), { description: errorMessage(err) });
  }
}

/** A group's name without what a file name can't hold. */
function fileNameOf(name: string) {
  return name.replace(/[/:*?"<>|\\]/g, "_");
}

/** Says where an exported file went, with a way to it. */
function saved(path: string) {
  toast.success(t("groups.exported"), {
    description: path,
    duration: 10_000,
    action: { label: t("groups.showFile"), onClick: () => revealItemInDir(path) },
  });
}

/** Hands a file the page made to the browser's downloads. */
function download(fileName: string, file: Blob) {
  const url = URL.createObjectURL(file);
  const link = document.createElement("a");
  link.href = url;
  link.download = fileName;
  link.click();
  URL.revokeObjectURL(url);
  toast.success(t("groups.exported"), { description: t("groups.exportedBrowser", fileName) });
}
