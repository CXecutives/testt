// The app's language: German or English. The backend decides it (the user's choice in
// Einstellungen, else the OS language) and sends it with the app state; until that arrives
// the page guesses from the web view, which follows the OS. Switching needs no restart:
// every text read through `t` (t.ts) and every number or date from format.ts follows
// `language.current`, which is reactive state.

import type { Language } from '../ipc/types';

/** The locale of numbers and dates per language: German, and British English (24 h, day first). */
const LOCALE: Record<Language, string> = { de: 'de-DE', en: 'en-GB' };

/** German for a German web view, English for any other (like the backend's rule). */
function guess(): Language {
  const tag = typeof navigator === 'undefined' ? '' : navigator.language;
  return /^(de|gsw|nds)\b/i.test(tag) ? 'de' : 'en';
}

class LanguageStore {
  current = $state<Language>(guess());

  /** The locale for Intl formats. */
  get locale(): string {
    return LOCALE[this.current];
  }

  /** Switch the language of the whole page at once (also the `lang` of the document). */
  set(next: Language): void {
    this.current = next;
    document.documentElement.lang = next;
  }
}

export const language = new LanguageStore();
