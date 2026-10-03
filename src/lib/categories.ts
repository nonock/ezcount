// The kinds of spending an expense can be filed under. The group stores the key; each
// language names it.

import CarIcon from "@lucide/svelte/icons/car";
import GiftIcon from "@lucide/svelte/icons/gift";
import HeartPulseIcon from "@lucide/svelte/icons/heart-pulse";
import HouseIcon from "@lucide/svelte/icons/house";
import PlaneIcon from "@lucide/svelte/icons/plane";
import ShoppingBagIcon from "@lucide/svelte/icons/shopping-bag";
import ShoppingCartIcon from "@lucide/svelte/icons/shopping-cart";
import TagIcon from "@lucide/svelte/icons/tag";
import TicketIcon from "@lucide/svelte/icons/ticket";
import UtensilsIcon from "@lucide/svelte/icons/utensils";
import ZapIcon from "@lucide/svelte/icons/zap";
import { t } from "./i18n/index.svelte";

/** How many `.tone-N` classes styles.css defines. */
const TONES = 5;

export const CATEGORIES = [
  { key: "food", label: "category.food", icon: UtensilsIcon },
  { key: "groceries", label: "category.groceries", icon: ShoppingCartIcon },
  { key: "transport", label: "category.transport", icon: CarIcon },
  { key: "housing", label: "category.housing", icon: HouseIcon },
  { key: "bills", label: "category.bills", icon: ZapIcon },
  { key: "leisure", label: "category.leisure", icon: TicketIcon },
  { key: "travel", label: "category.travel", icon: PlaneIcon },
  { key: "shopping", label: "category.shopping", icon: ShoppingBagIcon },
  { key: "health", label: "category.health", icon: HeartPulseIcon },
  { key: "gifts", label: "category.gifts", icon: GiftIcon },
  { key: "other", label: "category.other", icon: TagIcon },
] as const;

export type Category = (typeof CATEGORIES)[number];

const OTHER = CATEGORIES[CATEGORIES.length - 1];

/**
 * The category with this key, or nothing for an expense without one. A key this version
 * doesn't know (set by a newer one) counts as "Other".
 */
export function categoryOf(key: string | null | undefined): Category | null {
  if (!key) return null;
  return CATEGORIES.find((c) => c.key === key) ?? OTHER;
}

/** The category's name, "No category" without one. */
export function categoryName(key: string | null | undefined): string {
  const category = categoryOf(key);
  return category ? t(category.label) : t("category.none");
}

/** Classes giving a category its color: a soft fill and readable text. */
export function categoryTone(key: string | null | undefined): string {
  const category = categoryOf(key);
  if (!category) return "";
  return `tone-${CATEGORIES.indexOf(category) % TONES} bg-(--tone-fill) text-(--tone-text)`;
}
