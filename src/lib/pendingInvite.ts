// An invite that waits for the person to log in or create their account is kept on the device,
// so that closing the app meanwhile (to install it, say) doesn't lose it. For a week: an older
// one would join a group long forgotten.

const INVITE_KEY = "ezcount_pending_invite";
const INVITE_DAYS = 7;

/** The invite link kept for after the login, if it isn't too old. */
export function savedInvite(): string | null {
  try {
    const saved = JSON.parse(localStorage.getItem(INVITE_KEY) ?? "null");
    const fresh = saved && Date.now() - saved.at < INVITE_DAYS * 24 * 3600 * 1000;
    return fresh && typeof saved.link === "string" ? saved.link : null;
  } catch {
    return null;
  }
}

/** Keeps an invite link for after the login; `null` forgets the one kept. */
export function saveInvite(link: string | null) {
  try {
    if (link) localStorage.setItem(INVITE_KEY, JSON.stringify({ link, at: Date.now() }));
    else localStorage.removeItem(INVITE_KEY);
  } catch {
    // Without storage it only waits while the app stays open.
  }
}
