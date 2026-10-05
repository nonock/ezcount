// bits-ui doesn't hide the page behind a modal dialog, as Radix does: this does it for the app.

/** Whether a modal dialog is open somewhere outside `root`. */
export function modalOutside(root: HTMLElement): boolean {
  const modal = document.querySelector('[aria-modal="true"][data-state="open"]');
  return modal !== null && !root.contains(modal);
}

/**
 * While a dialog is open, hides the app behind it (`#root`) from screen readers. Dialogs
 * render outside the app's root, at the end of the page. Call it while a component
 * initializes.
 */
export function hideAppBehindModals() {
  $effect(() => {
    const root = document.getElementById("root");
    if (!root) return;
    const update = () => {
      if (modalOutside(root)) root.setAttribute("aria-hidden", "true");
      else root.removeAttribute("aria-hidden");
    };
    const observer = new MutationObserver(update);
    observer.observe(document.body, {
      subtree: true,
      childList: true,
      attributes: true,
      attributeFilter: ["data-state"],
    });
    update();
    return () => {
      observer.disconnect();
      root.removeAttribute("aria-hidden");
    };
  });
}
