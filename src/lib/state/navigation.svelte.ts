// Which group is open, and which of its tabs.
//
// The open group is also a history entry, so Back (Android's back button, a mouse's back
// button) returns to the group list instead of leaving the app. A reload keeps it open.
import { closeTopLayer } from "@/services/native.svelte";
import type { TabType } from "@/types";

/** The group open in the current history entry. */
function historyGroup(): string | null {
  const state = window.history.state as { groupId?: unknown } | null;
  return typeof state?.groupId === "string" ? state.groupId : null;
}

class Navigation {
  groupId = $state<string | null>(historyGroup());
  tab = $state<TabType>("expenses");
  // Set while the app itself goes back to the group list.
  #closing = false;

  open(groupId: string) {
    if (historyGroup()) window.history.replaceState({ groupId }, "");
    else window.history.pushState({ groupId }, "");
    this.groupId = groupId;
    this.tab = "expenses";
  }

  close() {
    if (historyGroup()) {
      this.#closing = true;
      window.history.back();
    }
    this.groupId = null;
  }

  /** Follows Back and Forward. Returns the function that stops it. */
  listen(): () => void {
    const onPopState = () => {
      const groupId = historyGroup();
      const open = this.groupId;
      const closing = this.#closing;
      this.#closing = false;
      if (!closing && !groupId && open && closeTopLayer()) {
        // Back first closed what was open over the group: stay in it.
        window.history.pushState({ groupId: open }, "");
        return;
      }
      this.groupId = groupId;
    };
    window.addEventListener("popstate", onPopState);
    return () => window.removeEventListener("popstate", onPopState);
  }
}

export const navigation = new Navigation();
