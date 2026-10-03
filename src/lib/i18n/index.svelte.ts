// The app's language: the system's unless the user picked one, and its text (`t`).
// To add a language, write its messages next to `fr.ts` and list it in `MESSAGES` and
// `LANGUAGES`.

import { en, type Messages } from "./en";
import { fr } from "./fr";

const MESSAGES = { en, fr } satisfies Record<string, Messages>;

export type Language = keyof typeof MESSAGES;
/** `system` follows the device. */
export type LanguageChoice = Language | "system";

/** The languages on offer, each under its own name. */
export const LANGUAGES: { code: Language; name: string }[] = [
  { code: "en", name: "English" },
  { code: "fr", name: "Français" },
];

const STORAGE_KEY = "language";

const isLanguage = (code: string | null): code is Language => code !== null && code in MESSAGES;

function savedChoice(): LanguageChoice {
  try {
    const saved = localStorage.getItem(STORAGE_KEY);
    return isLanguage(saved) ? saved : "system";
  } catch {
    return "system";
  }
}

/** The first of the device's languages the app speaks, English otherwise. */
function systemLanguage(): Language {
  for (const tag of navigator.languages ?? [navigator.language]) {
    const code = tag.toLowerCase().split("-")[0];
    if (isLanguage(code)) return code;
  }
  return "en";
}

class I18n {
  choice = $state<LanguageChoice>(savedChoice());

  get language(): Language {
    return this.choice === "system" ? systemLanguage() : this.choice;
  }

  /**
   * For dates and numbers: the device's own variant of the language when it has one (`en-GB`,
   * `fr-CA`), so they read as the user's other apps show them.
   */
  get locale(): string {
    const own = navigator.language ?? "";
    return own.toLowerCase().split("-")[0] === this.language ? own : this.language;
  }

  choose(choice: LanguageChoice) {
    this.choice = choice;
    try {
      if (choice === "system") localStorage.removeItem(STORAGE_KEY);
      else localStorage.setItem(STORAGE_KEY, choice);
    } catch {
      // Remembering the language is only a convenience.
    }
    document.documentElement.lang = this.language;
  }
}

export const i18n = new I18n();
document.documentElement.lang = i18n.language;

type Args<K extends keyof Messages> = Messages[K] extends (...args: infer A) => string ? A : [];

/** The text for `key` in the app's language. Reactive: it follows a change of language. */
export function t<K extends keyof Messages>(key: K, ...args: Args<K>): string {
  const message = MESSAGES[i18n.language][key] as string | ((...args: unknown[]) => string);
  return typeof message === "function" ? message(...args) : message;
}
