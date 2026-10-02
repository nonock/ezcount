// One accessible confirmation dialog for the whole app, replacing `window.confirm`.
// `ConfirmDialog.svelte`, mounted once in `App`, shows it.

export interface ConfirmOptions {
  title: string;
  description?: string;
  confirmLabel?: string;
  cancelLabel?: string;
  destructive?: boolean;
}

class Confirmation {
  open = $state(false);
  // Kept after closing so the text doesn't vanish during the close animation.
  options = $state<ConfirmOptions>({ title: "" });
  #resolve: ((confirmed: boolean) => void) | null = null;

  ask(options: ConfirmOptions): Promise<boolean> {
    this.#resolve?.(false);
    this.options = options;
    this.open = true;
    return new Promise((resolve) => {
      this.#resolve = resolve;
    });
  }

  settle(confirmed: boolean) {
    this.#resolve?.(confirmed);
    this.#resolve = null;
    this.open = false;
  }
}

export const confirmation = new Confirmation();

/** Asks the user to confirm; resolves to false if they cancel or close the dialog. */
export function askConfirm(options: ConfirmOptions): Promise<boolean> {
  return confirmation.ask(options);
}
