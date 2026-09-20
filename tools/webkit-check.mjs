// Die Oberfläche in Apples Engine prüfen, ohne einen Mac zu besitzen.
//
// macOS zeichnet Webinhalte mit WebKit. Playwright liefert eine echte WebKit-Version
// auch für Windows – damit lässt sich hier feststellen, ob die Oberfläche dort genauso
// aussieht und ob die Interaktions-Szenarien durchlaufen. Was das NICHT prüft:
// Fensterrahmen, Ampelknöpfe, Schlüsselbund, Datei-Dialoge – alles, was nicht Webinhalt
// ist. Dafür gibt es die CI auf echter Apple-Hardware.
//
// Playwright ist Absicht keine Abhängigkeit des Projekts (die App selbst kommt ohne
// Node und ohne Bauschritt aus). Einmalig irgendwo installieren:
//
//     npm i playwright && npx playwright install webkit chromium
//
// Dann, mit laufendem Prüfstand (node tools/ui-harness/server.mjs 5177):
//
//     node tools/webkit-check.mjs [url] [ausgabeordner]

import { mkdirSync } from 'node:fs';

const url = process.argv[2] ?? 'http://127.0.0.1:5177/';
const out = process.argv[3] ?? 'webkit-shots';

// Playwright liegt bewusst außerhalb des Projekts. Wo, sagt PLAYWRIGHT (Pfad zum Ordner,
// in dem `npm i playwright` lief) – sonst wird es neben diesem Skript gesucht.
let webkit;
let chromium;
const where = process.env.PLAYWRIGHT;
try {
  const { pathToFileURL } = await import('node:url');
  const from = where
    ? pathToFileURL(`${where}/node_modules/playwright/index.js`).href
    : 'playwright';
  // Über den Dateipfad geladen liegen die Engines unter `default`, über den Namen daneben.
  const mod = await import(from);
  ({ webkit, chromium } = mod.webkit ? mod : mod.default);
} catch {
  console.error('Playwright fehlt. Einmalig irgendwo:');
  console.error('  npm i playwright && npx playwright install webkit chromium');
  console.error('Dann diesen Ordner als PLAYWRIGHT mitgeben.');
  process.exit(2);
}

mkdirSync(out, { recursive: true });

// Fenstergrößen, die die App aushalten muss (Untergrenze laut tauri.conf.json: 760×520).
const SIZES = [[780, 560], [1280, 860], [1920, 1080]];

async function check(engine, name) {
  const browser = await engine.launch();
  const page = await browser.newPage({ viewport: { width: 1280, height: 860 } });
  const problems = [];
  page.on('pageerror', (e) => problems.push(`Seitenfehler: ${e.message}`));
  page.on('console', (m) => { if (m.type() === 'error') problems.push(`Konsole: ${m.text()}`); });

  await page.goto(url, { waitUntil: 'networkidle' });
  await page.waitForSelector('.row, .steps', { timeout: 8000 })
    .catch(() => problems.push('Weder Jobzeilen noch Erststart-Schritte erschienen'));

  // Jede Größe: kein waagerechter Bildlauf, Umbruch an der richtigen Stelle.
  const sizes = {};
  for (const [w, h] of SIZES) {
    await page.setViewportSize({ width: w, height: h });
    await page.waitForTimeout(300);
    await page.screenshot({ path: `${out}/${name}-${w}x${h}.png` });
    sizes[`${w}x${h}`] = await page.evaluate(() => {
      const list = document.querySelector('#list')?.getBoundingClientRect();
      const reader = document.querySelector('#reader')?.getBoundingClientRect();
      return {
        hscroll: document.documentElement.scrollWidth > document.documentElement.clientWidth,
        einspaltig: !list || !reader || reader.width === 0 || list.width === 0
          || Math.abs(list.left - reader.left) < 1,
      };
    });
  }

  await page.setViewportSize({ width: 1280, height: 860 });
  await page.waitForTimeout(300);
  const szenarien = await page.evaluate(async () => (await import('/__checks.js')).run())
    .catch((e) => `Szenarien nicht ausführbar: ${e.message}`);

  await browser.close();
  return { sizes, szenarien, problems };
}

const a = await check(webkit, 'webkit');
const b = await check(chromium, 'chromium');

const fehler = (text) => String(text).split('\n').filter((l) => !l.startsWith('OK')).length;

console.log('Größen (WebKit):  ', JSON.stringify(a.sizes));
console.log('Größen (Chromium):', JSON.stringify(b.sizes));
console.log(`Szenarien: WebKit ${fehler(a.szenarien)} Fehler, Chromium ${fehler(b.szenarien)} Fehler`);
for (const [name, r] of [['WebKit', a], ['Chromium', b]]) {
  for (const line of String(r.szenarien).split('\n')) {
    if (!line.startsWith('OK')) console.log(`  ${name}: ${line}`);
  }
  for (const p of r.problems) console.log(`  ${name}: ${p}`);
}

const schlecht = fehler(a.szenarien) + fehler(b.szenarien) + a.problems.length + b.problems.length;
console.log(schlecht ? 'NICHT GRÜN' : `grün – Bilder in ${out}/`);
process.exit(schlecht ? 1 : 0);
