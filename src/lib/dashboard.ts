// What the group list says of the user's money: per group, and over all of them.

import type { Group } from "@/types";
import { netBalance } from "./split";

/**
 * What each group owes the user (above zero) or the user owes it, by group id, for the groups
 * where they said who they are (`identityIn`).
 */
export function groupNets(
  groups: Group[],
  identityIn: (groupId: string) => string | null
): Map<string, number> {
  return new Map(
    groups.flatMap((g): [string, number][] => {
      const me = identityIn(g.id);
      return me ? [[g.id, netBalance(g, me)]] : [];
    })
  );
}

export interface CurrencyTotal {
  currency: string;
  /** What the groups owe the user. */
  owed: number;
  /** What the user owes the groups. */
  owes: number;
}

/** Over these groups, a line per currency: nothing adds up across them. */
export function totalsByCurrency(groups: Group[], nets: Map<string, number>): CurrencyTotal[] {
  const byCurrency = new Map<string, { owed: number; owes: number }>();
  for (const g of groups) {
    const net = nets.get(g.id);
    if (!net) continue;
    const total = byCurrency.get(g.currency) ?? { owed: 0, owes: 0 };
    if (net > 0) total.owed += net;
    else total.owes -= net;
    byCurrency.set(g.currency, total);
  }
  return [...byCurrency].map(([currency, total]) => ({ currency, ...total }));
}
