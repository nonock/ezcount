import type { Group } from "@/types";

/** How many `.tone-N` classes styles.css defines. */
const TONES = 5;

/**
 * Classes giving a member's avatar or badge their color: a soft fill and readable text. The
 * color follows the member's position in the group, which every device sorts the same way, so a
 * member has the same color everywhere. Unknown members get no color.
 */
export function memberTone(group: Group, participantId: string | undefined): string {
  const index = group.participants.findIndex((p) => p.id === participantId);
  return index < 0 ? "" : `tone-${index % TONES} bg-(--tone)/15 text-(--tone-text)`;
}
