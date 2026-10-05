import { flushSync } from "svelte";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { hideAppBehindModals, modalOutside } from "./modalBackdrop.svelte";

let root: HTMLElement;

/** A modal dialog as bits-ui renders one. */
function dialog(state: "open" | "closed", parent: HTMLElement = document.body) {
  const element = document.createElement("div");
  element.setAttribute("aria-modal", "true");
  element.dataset.state = state;
  parent.append(element);
  return element;
}

beforeEach(() => {
  root = document.createElement("div");
  root.id = "root";
  document.body.append(root);
});

afterEach(() => {
  document.body.replaceChildren();
});

describe("modalOutside", () => {
  it("is a modal open at the end of the page, not one inside the app", () => {
    expect(modalOutside(root)).toBe(false);
    dialog("closed");
    expect(modalOutside(root)).toBe(false);
    dialog("open", root);
    expect(modalOutside(root)).toBe(false);
  });

  it("sees an open one outside the app", () => {
    dialog("open");
    expect(modalOutside(root)).toBe(true);
  });
});

describe("hideAppBehindModals", () => {
  it("hides the app from screen readers while a dialog is open", async () => {
    const stop = $effect.root(() => hideAppBehindModals());
    flushSync();
    expect(root.hasAttribute("aria-hidden")).toBe(false);

    const opened = dialog("open");
    await vi.waitFor(() => expect(root.getAttribute("aria-hidden")).toBe("true"));

    opened.dataset.state = "closed";
    await vi.waitFor(() => expect(root.hasAttribute("aria-hidden")).toBe(false));
    stop();
  });

  it("shows the app again when it stops", async () => {
    dialog("open");
    const stop = $effect.root(() => hideAppBehindModals());
    flushSync();
    expect(root.getAttribute("aria-hidden")).toBe("true");
    stop();
    expect(root.hasAttribute("aria-hidden")).toBe(false);
  });
});
