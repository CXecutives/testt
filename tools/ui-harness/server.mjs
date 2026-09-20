// Prüfstand für die Oberfläche: liefert ui/ aus und setzt vor main.js ein nachgebautes __TAURI__ ein.
import http from 'node:http';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const here = path.dirname(fileURLToPath(import.meta.url));
const ui = path.resolve(here, '../../ui');
const port = Number(process.argv[2] || 5177);
const types = { '.html': 'text/html', '.js': 'text/javascript', '.mjs': 'text/javascript', '.css': 'text/css' };

http.createServer((req, res) => {
  const url = new URL(req.url, 'http://x');
  let file;
  if (url.pathname === '/__stub.js') file = path.join(here, 'stub.js');
  else if (url.pathname === '/__checks.js') file = path.join(here, 'checks.js');
  else file = path.join(ui, url.pathname === '/' ? 'index.html' : url.pathname);
  if (!file.startsWith(ui) && !file.startsWith(here)) { res.writeHead(403); res.end(); return; }
  fs.readFile(file, (err, data) => {
    if (err) { res.writeHead(404); res.end(); return; }
    let body = data;
    if (file.endsWith('index.html')) {
      body = data.toString().replace('<script type="module" src="js/main.js"></script>',
        '<script src="/__stub.js"></script><script type="module" src="js/main.js"></script>');
    }
    res.writeHead(200, { 'content-type': types[path.extname(file)] || 'application/octet-stream', 'cache-control': 'no-store' });
    res.end(body);
  });
}).listen(port, '127.0.0.1', () => console.log(`harness on http://127.0.0.1:${port}/`));
