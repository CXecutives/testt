// Eingabe-Politik: Die App reagiert auf Linksklick und Zeiger – sonst auf nichts.
//
// Das ist die einzige Stelle im Projekt, die Tasten oder andere Maustasten behandelt
// (ein Vertragstest hält das fest). Kein Kontextmenü, keine mittlere Maustaste, kein
// Ziehen, kein Doppelklick-Verhalten, keine Tastenkürzel – es ist ein Werkzeug, kein
// Browserfenster. Nur Eingabefelder bleiben ausgenommen: dort muss man tippen und das
// 16-stellige App-Passwort einfügen können.

const EDITABLE = 'input, textarea, [contenteditable=""], [contenteditable="true"]';

const inField = (target) => target instanceof Element && target.closest(EDITABLE) !== null;

/** Ziehbereich der eigenen Titelleiste: Dort gehören Doppelklick und Ziehen dem Fenster. */
const inDragRegion = (target) => target instanceof Element && target.closest('[data-tauri-drag-region]') !== null;

export function install() {
  // Kontextmenü: in Feldern erlaubt (Einfügen), sonst nie.
  document.addEventListener('contextmenu', (event) => {
    if (!inField(event.target)) event.preventDefault();
  });

  // Mittlere und rechte Taste lösen nichts aus – auch kein Autoscroll.
  document.addEventListener('mousedown', (event) => {
    if (event.button !== 0 && !inField(event.target)) event.preventDefault();
  });
  document.addEventListener('auxclick', (event) => event.preventDefault());

  // Kein Ziehen von Text, Bildern oder Links.
  document.addEventListener('dragstart', (event) => event.preventDefault());

  // Doppelklick hat keine eigene Bedeutung; das Fenster darf ihn behalten.
  document.addEventListener('dblclick', (event) => {
    if (!inField(event.target) && !inDragRegion(event.target)) event.preventDefault();
  });

  // Tasten wirken nur in Eingabefeldern. Damit entfallen alle Kürzel auf einmal –
  // auch die des WebView (Nachladen, Suchen, Drucken).
  document.addEventListener('keydown', (event) => {
    if (!inField(event.target)) event.preventDefault();
  }, true);

  // Strg+Rad zoomt sonst die ganze Oberfläche.
  document.addEventListener('wheel', (event) => {
    if (event.ctrlKey) event.preventDefault();
  }, { passive: false });
}
