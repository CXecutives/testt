// Erzeugt ui/THIRD-PARTY.txt: die Lizenzhinweise aller Bausteine, die im Programm landen.
//
// MIT und Apache-2.0 verlangen, dass der Urheberhinweis bei der Weitergabe mitgeht;
// MPL-2.0 zusätzlich den Hinweis, wo der Quelltext liegt. Die Datei liegt in `ui/` und
// wird damit ins Programm eingebettet – sie kann nicht verlorengehen, wenn jemand nur
// die Exe weitergibt. Angezeigt wird sie nirgends.
//
//     node tools/third-party.mjs
//
// Nach jedem Hinzufügen oder Entfernen einer Abhängigkeit erneut laufen lassen
// (ein Test vergleicht die Liste mit Cargo.lock).

import { execFileSync } from 'node:child_process';
import { readFileSync, readdirSync, writeFileSync, existsSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const repo = join(dirname(fileURLToPath(import.meta.url)), '..');

// Beide Plattformen zusammen: Windows-eigene Kisten fehlen sonst im Mac-Lauf und umgekehrt.
const TARGETS = ['x86_64-pc-windows-msvc', 'aarch64-apple-darwin', 'x86_64-apple-darwin'];

/** Alle Pakete, die wirklich ins Programm kommen (reine Test-Abhängigkeiten nicht). */
function collect(target) {
  const raw = execFileSync('cargo', [
    'metadata', '--format-version', '1', '--filter-platform', target,
  ], { cwd: repo, maxBuffer: 64 * 1024 * 1024, encoding: 'utf8' });
  const meta = JSON.parse(raw);
  const byId = new Map(meta.packages.map((p) => [p.id, p]));
  const nodes = new Map(meta.resolve.nodes.map((n) => [n.id, n]));
  const seen = new Set();
  const stack = [...meta.workspace_members];
  while (stack.length) {
    const id = stack.pop();
    if (seen.has(id) || !nodes.has(id)) continue;
    seen.add(id);
    for (const dep of nodes.get(id).deps) {
      const kinds = new Set((dep.dep_kinds ?? [{}]).map((k) => k.kind ?? null));
      if (kinds.size === 1 && kinds.has('dev')) continue;
      stack.push(dep.pkg);
    }
  }
  return [...seen].map((id) => byId.get(id)).filter(Boolean);
}

const LICENSE_FILE = /^(licen[cs]e|copying|notice|unlicense)([-_.].*)?$/i;

/** Die Lizenztexte, die eine Kiste selbst mitliefert. */
function texts(pkg) {
  const dir = dirname(pkg.manifest_path);
  if (!existsSync(dir)) return [];
  const out = [];
  for (const name of readdirSync(dir).sort()) {
    if (!LICENSE_FILE.test(name)) continue;
    try {
      const body = readFileSync(join(dir, name), 'utf8').replace(/\r\n/g, '\n').trim();
      if (body.length > 40) out.push({ name, body });
    } catch { /* unlesbar: dann steht nur die Lizenzangabe da */ }
  }
  return out;
}

const packages = new Map();
for (const target of TARGETS) {
  let list;
  try {
    list = collect(target);
  } catch {
    // Ein Ziel, das hier nicht aufgelöst werden kann, wird übersprungen; der Hinweis
    // steht am Ende der Datei, damit niemand glaubt, sie sei vollständig.
    continue;
  }
  for (const p of list) {
    if (p.source === null) continue;            // eigener Code
    packages.set(`${p.name} ${p.version}`, p);
  }
}

const sorted = [...packages.entries()].sort(([a], [b]) => a.localeCompare(b, 'en'));

// Fast alle Texte sind derselbe Lizenzwortlaut mit einer anderen Urheberzeile. Der
// Wortlaut wird einmal abgedruckt, die Urheberzeilen stehen bei den Bausteinen – so
// bleibt jeder Hinweis erhalten und die Datei ein Fünftel so groß.
// Eine echte Urheberzeile beginnt mit „Copyright“ und nennt ein Jahr. Ohne diese
// Bedingung fängt man Sätze aus dem Apache-Wortlaut mit ein („copyright notice that …“).
const HOLDER = /^[ \t]*copyright\b[^\n]*$/gim;
// Der Apache-Wortlaut enthält selbst Zeilen, die mit „copyright“ beginnen. Eine echte
// Urheberzeile nennt ein Jahr oder eine Adresse.
const isHolder = (line) => /\b(19|20)\d{2}\b|[@<]/.test(line) && !line.includes('[yyyy]');
const normalize = (body) => body.replace(HOLDER, '').replace(/\n{3,}/g, '\n\n').trim();

const bodies = new Map();   // Wortlaut ohne Urheberzeile → Nummer
const keyOf = (body) => {
  const core = normalize(body);
  if (!bodies.has(core)) bodies.set(core, bodies.size + 1);
  return bodies.get(core);
};
const holdersOf = (body) => (body.match(HOLDER) ?? [])
  .map((l) => l.trim())
  .filter((l) => l.length > 12 && l.length < 200 && isHolder(l));

const lines = [];
lines.push('Mitgelieferte Bausteine');
lines.push('=======================');
lines.push('');
lines.push('Der Job-Alert-Monitor enthält die unten aufgeführten freien Bibliotheken.');
lines.push('Diese Datei erfüllt die Hinweispflicht ihrer Lizenzen (vor allem MIT,');
lines.push('Apache-2.0 und MPL-2.0). Sie wird von tools/third-party.mjs erzeugt.');
lines.push('');
lines.push('Quelltext jeder Bibliothek: https://crates.io/crates/<name> – dort steht auch');
lines.push('das Projektarchiv. Das erfüllt die Quelltext-Auskunft der MPL-2.0.');
lines.push('');
lines.push('Die Schrift Inter steht unter der SIL Open Font License; ihr Text liegt in');
lines.push('ui/fonts/Inter-LICENSE.txt.');
lines.push('');
lines.push(`Bausteine: ${sorted.length}`);
lines.push('');
lines.push('---------------------------------------------------------------------------');
lines.push('');

const ohneText = [];
for (const [label, p] of sorted) {
  const own = texts(p);
  if (!own.length) ohneText.push(`${label} — ${p.license ?? 'ohne Angabe'}`);
  const marks = [...new Set(own.map((t) => keyOf(t.body)))].map((n) => `[${n}]`).join(' ');
  lines.push(`${label} — ${p.license ?? 'siehe Lizenzdatei'}${marks ? `  ${marks}` : ''}`);
  for (const holder of [...new Set(own.flatMap((t) => holdersOf(t.body)))]) {
    lines.push(`    ${holder}`);
  }
}

// Nichts verschweigen: Wer keinen Lizenztext mitliefert, steht trotzdem hier – mit der
// Lizenzangabe aus seiner Cargo.toml und dem Verweis auf crates.io.
if (ohneText.length) {
  lines.push('');
  lines.push('---------------------------------------------------------------------------');
  lines.push('Ohne beigelegten Lizenztext');
  lines.push('---------------------------------------------------------------------------');
  lines.push('');
  lines.push('Diese Bausteine nennen ihre Lizenz nur in den Metadaten. Der Wortlaut steht');
  lines.push('unter https://spdx.org/licenses/, der Quelltext auf crates.io.');
  lines.push('');
  for (const line of ohneText) lines.push(line);
}

lines.push('');
lines.push('---------------------------------------------------------------------------');
lines.push('Lizenztexte');
lines.push('---------------------------------------------------------------------------');
for (const [body, number] of [...bodies.entries()].sort((a, b) => a[1] - b[1])) {
  lines.push('');
  lines.push(`[${number}]`);
  lines.push('');
  lines.push(body);
  lines.push('');
  lines.push('---------------------------------------------------------------------------');
}

const out = join(repo, 'ui', 'THIRD-PARTY.txt');
writeFileSync(out, `${lines.join('\n')}\n`, 'utf8');
console.log(`${out}: ${sorted.length} Bausteine, ${bodies.size} Lizenztexte, ${Math.round(lines.join('\n').length / 1024)} KB`);
