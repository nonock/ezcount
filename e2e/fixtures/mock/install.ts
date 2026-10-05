// Puts the mock in the page, where Tauri's own internals would be: the app's bindings then
// call these commands instead of the Rust ones.

import { accountCommands } from "./commands/account";
import { deviceCommands } from "./commands/device";
import { expenseCommands } from "./commands/expenses";
import { groupCommands } from "./commands/groups";
import { eventCommands, installEvents, transformCallback } from "./events";
import { getGroups, w } from "./state";
import type { Commands } from "./types";

const commands: Commands = {
  ...eventCommands,
  ...deviceCommands,
  ...accountCommands,
  ...groupCommands,
  ...expenseCommands,
};

export function install() {
  installEvents();
  /** As if another device of the account had left the group. */
  w.__removeGroupElsewhere = (groupId: string) => {
    const idx = getGroups().findIndex((x) => x.id === groupId);
    if (idx !== -1) getGroups().splice(idx, 1);
  };
  w.__TAURI_INTERNALS__ = {
    transformCallback,
    invoke: async (cmd: string, args?: any) => {
      if (!Object.hasOwn(commands, cmd)) throw new Error(`Unknown command: ${cmd}`);
      return commands[cmd](args);
    },
  };
}
