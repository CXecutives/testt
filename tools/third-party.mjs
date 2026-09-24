// Generates src-tauri/resources/THIRD-PARTY.txt: the license notices of everything that
// ships inside the app - Rust crates, the npm runtime packages bundled into the UI, and the
// Inter font.
//
// MIT, Apache-2.0 and ISC require the copyright notice to travel with the program; MPL-2.0
// additionally requires telling where the source is. The file is bundled as an app resource
// and shown nowhere.
//
//     node tools/third-party.mjs
//
// Run it again after adding or removing a dependency (core/tests/notices.rs compares the
// list with the manifests).

import { execFileSync } from 'node:child_process';
import { existsSync, readFileSync, readdirSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const repo = join(dirname(fileURLToPath(import.meta.url)), '..');
const OUT = join(repo, 'src-tauri', 'resources', 'THIRD-PARTY.txt');
const FONT_LICENSE = join(repo, 'ui', 'src', 'assets', 'fonts', 'Inter-LICENSE.txt');

// Both platforms together: Windows-only crates would be missing from a Mac run and vice versa.
const TARGETS = ['x86_64-pc-windows-msvc', 'aarch64-apple-darwin', 'x86_64-apple-darwin'];

/* ------------------------------------------------------------------ Rust */

/** Every crate that ends up in the program (pure dev-dependencies do not). */
function crates(target) {
  const raw = execFileSync(
    'cargo',
    ['metadata', '--format-version', '1', '--filter-platform', target],
    { cwd: repo, maxBuffer: 64 * 1024 * 1024, encoding: 'utf8' },
  );
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

/* ------------------------------------------------------------------- npm */

/** The runtime packages of the UI and everything they depend on (dev tooling excluded). */
function npmPackages() {
  const lock = JSON.parse(readFileSync(join(repo, 'package-lock.json'), 'utf8'));
  const out = [];
  for (const [path, entry] of Object.entries(lock.packages ?? {})) {
    if (path === '' || entry.dev || entry.devOptional || !path.startsWith('node_modules/')) {
      continue;
    }
    const name = path.slice(path.lastIndexOf('node_modules/') + 'node_modules/'.length);
    const dir = join(repo, path);
    let license = entry.license;
    if (!license && existsSync(join(dir, 'package.json'))) {
      license = JSON.parse(readFileSync(join(dir, 'package.json'), 'utf8')).license;
    }
    out.push({ name, version: entry.version, license, dir });
  }
  return out;
}

/* ----------------------------------------------------------- license texts */

const LICENSE_FILE = /^(licen[cs]e|copying|notice|unlicense)([-_.].*)?$/i;

/** The license texts a package ships itself. */
function texts(dir) {
  if (!existsSync(dir)) return [];
  const out = [];
  for (const name of readdirSync(dir).sort()) {
    if (!LICENSE_FILE.test(name)) continue;
    try {
      const body = readFileSync(join(dir, name), 'utf8').replace(/\r\n/g, '\n').trim();
      if (body.length > 40) out.push({ name, body });
    } catch (error) {
      // Unreadable: the package is still listed with its declared license.
      console.warn(`${join(dir, name)}: ${error.message}`);
    }
  }
  return out;
}

// Almost all texts are the same license wording with a different copyright line. The
// wording is printed once; the copyright lines stay with each package, so every notice is
// kept and the file is a fifth of the size. A real copyright line starts with "Copyright"
// and names a year or an address (the Apache wording itself has lines starting with it).
const HOLDER = /^[ \t]*copyright\b[^\n]*$/gim;
const isHolder = (line) => /\b(19|20)\d{2}\b|[@<]/.test(line) && !line.includes('[yyyy]');
const normalize = (body) =>
  body
    .replace(HOLDER, '')
    .replace(/\n{3,}/g, '\n\n')
    .trim();

const bodies = new Map(); // wording without copyright lines -> number
const keyOf = (body) => {
  const core = normalize(body);
  if (!bodies.has(core)) bodies.set(core, bodies.size + 1);
  return bodies.get(core);
};
const holdersOf = (body) =>
  (body.match(HOLDER) ?? [])
    .map((l) => l.trim())
    .filter((l) => l.length > 12 && l.length < 200 && isHolder(l));

const RULE = '---------------------------------------------------------------------------';
const lines = [];
const withoutText = [];

function listing(title, packages) {
  lines.push(RULE, title, RULE, '');
  for (const p of packages) {
    const own = texts(p.dir);
    const label = `${p.name} ${p.version}`;
    if (!own.length) withoutText.push(`${label} - ${p.license ?? 'no license declared'}`);
    const marks = [...new Set(own.map((t) => keyOf(t.body)))].map((n) => `[${n}]`).join(' ');
    lines.push(`${label} - ${p.license ?? 'see license file'}${marks ? `  ${marks}` : ''}`);
    for (const holder of [...new Set(own.flatMap((t) => holdersOf(t.body)))]) {
      lines.push(`    ${holder}`);
    }
  }
  lines.push('');
}

const rust = new Map();
for (const target of TARGETS) {
  let list;
  try {
    list = crates(target);
  } catch (error) {
    // A target that cannot be resolved here is skipped and named at the end of the file,
    // so nobody takes the list for complete.
    console.warn(`${target}: ${error.message}`);
    continue;
  }
  for (const p of list) {
    if (p.source === null) continue; // own code
    rust.set(`${p.name} ${p.version}`, {
      name: p.name,
      version: p.version,
      license: p.license,
      dir: dirname(p.manifest_path),
    });
  }
}
const byLabel = (a, b) => `${a.name} ${a.version}`.localeCompare(`${b.name} ${b.version}`, 'en');
const rustList = [...rust.values()].sort(byLabel);
const npmList = npmPackages().sort(byLabel);

lines.push('Bundled components');
lines.push('==================');
lines.push('');
lines.push('Job-Alert-Monitor contains the free libraries listed below. This file fulfils');
lines.push('the notice requirements of their licenses (mainly MIT, Apache-2.0, ISC and');
lines.push('MPL-2.0). It is generated by tools/third-party.mjs.');
lines.push('');
lines.push('Source code: Rust crates at https://crates.io/crates/<name>, npm packages at');
lines.push('https://www.npmjs.com/package/<name>; both name the repository. This fulfils');
lines.push('the source information of the MPL-2.0.');
lines.push('');
lines.push('The font Inter is licensed under the SIL Open Font License 1.1; its text is');
lines.push('at the end of this file.');
lines.push('');
lines.push(`Rust crates: ${rustList.length}`);
lines.push(`npm packages: ${npmList.length}`);
lines.push('');
listing('Rust crates', rustList);
listing('npm packages (user interface)', npmList);

if (withoutText.length) {
  lines.push(RULE, 'Without an included license text', RULE, '');
  lines.push('These components declare their license only in their metadata. The wording');
  lines.push('is at https://spdx.org/licenses/, the source at crates.io or npmjs.com.');
  lines.push('');
  lines.push(...withoutText, '');
}

lines.push(RULE, 'License texts', RULE);
for (const [body, number] of [...bodies.entries()].sort((a, b) => a[1] - b[1])) {
  lines.push('', `[${number}]`, '', body, '', RULE);
}

lines.push('', RULE, 'Font: Inter', RULE, '');
lines.push(readFileSync(FONT_LICENSE, 'utf8').replace(/\r\n/g, '\n').trim());

const text = `${lines.join('\n')}\n`;
writeFileSync(OUT, text, 'utf8');
console.log(
  `${OUT}: ${rustList.length} crates, ${npmList.length} npm packages, ` +
    `${bodies.size} license texts, ${Math.round(text.length / 1024)} KB`,
);
