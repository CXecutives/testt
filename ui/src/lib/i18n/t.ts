// `t`: the UI catalog of the app's language, used like the catalogs themselves
// (`t.nav.jobs`, `t.run.of(3, 7)`). Every read goes through `language.current`, reactive
// state: a template, `$derived` or `$effect` that reads `t` follows a switch of the language
// at once. A value copied into a plain variable once (at the top of a script) does not,
// so screens read `t` where they render, or through `$derived`.

import type { Language } from '../ipc/types';
import { de, type Catalog } from './de';
import { en } from './en';
import { language } from './language.svelte';

const catalogs: Record<Language, Catalog> = { de, en };

/** The catalog of the current language (`catalogs[language.current]`, but reactive). */
export const t: Catalog = new Proxy(de as Catalog, {
  get: (_target, key) => Reflect.get(catalogs[language.current], key) as unknown,
  has: (_target, key) => Reflect.has(catalogs[language.current], key),
});
