# Job-Alert-Monitor

Windows-App, die die **Job-Alert-Mails von LinkedIn, freelance.de und freelancermap.de** aus einem
Gmail-Posteingang liest, daraus eine dublettenfreie Liste der Jobs macht, zu jedem Job den
**vollständigen Ausschreibungstext** holt und alles als Excel-/CSV-Datei und als Textdateien für das
KI-Matching ablegt. Ein Klick: *Mails rein → Projekte mit Volltext raus.*

Die App **liest nur**: Der Posteingang wird schreibgeschützt geöffnet, Mails bleiben ungelesen, nichts
wird verändert, gelöscht oder versendet.

---

## Einrichten

**Voraussetzungen:** Windows 10 oder 11 mit der Microsoft-Edge-WebView2-Laufzeit (auf Windows 11
vorinstalliert). Sonst nichts – keine Installation, kein Python.

**Gmail-App-Passwort:** Gmail erlaubt Programmen den Zugriff nur mit einem App-Passwort
(16 Buchstaben), nicht mit dem normalen Passwort.

1. Im Google-Konto die **Bestätigung in zwei Schritten** einschalten.
2. <https://myaccount.google.com/apppasswords> öffnen (in der App: *System → App-Passwort anlegen…*),
   einen Namen vergeben, **Erstellen** klicken, das Passwort kopieren. Leerzeichen sind egal.
3. In der App unter **System** Gmail-Adresse und App-Passwort eintragen → **Speichern**.
   Beides liegt danach in der Windows-Anmeldeinformationsverwaltung („Windows-Tresor“) – nie im
   Klartext.

Falls Gmail die Anmeldung ablehnt: in Gmail unter *Einstellungen → Weiterleitung und POP/IMAP* prüfen,
dass IMAP aktiviert ist.

**Starten:** `job-alert-monitor.exe` doppelklicken. Ein zweiter Start holt nur das offene Fenster nach
vorn.

## Bedienen

| Ansicht | Wofür |
|---|---|
| **Job-Alerts** | **Postfach abrufen** = Mails prüfen → neue Jobs speichern → Jobdetails holen → Dateien schreiben. **Jobdetails extrahieren** holt nur noch Fehlendes. Klick auf eine Zeile zeigt die Jobdetails, Doppelklick öffnet die Anzeige (auf dem Datum: die Mail in Gmail). Suche über Titel, Firma, Ort und Jobdetails. |
| **Portal-Zugänge** | Je Portal: Pausen, verbrauchte Abrufe, offene Jobdetails; freelance.de **Anmelden…/Abmelden**. |
| **Beraterprofil** | Profil als JSON hinterlegen (bleibt lokal). |
| **System** | Gmail-Zugang, Arbeitsordner, Ergebnisordner öffnen/leeren, Textdateien neu schreiben, Protokoll, **Alles zurücksetzen**. |

Die **Laufleiste** unter der Kopfzeile zeigt in jeder Ansicht, was die App gerade tut – mit Fortschritt,
Wartezeit bis zum nächsten Portalabruf und **Abbrechen** –, sonst das Ergebnis des letzten Laufs.

**Umfang und Portale** (rechts neben der Tabelle, *Ändern*): *Neu seit letztem Lauf* (Standard; beim ersten Mal 7 Tage) · *Letzte 7 Tage* · *Alle*.
Mehrfach gefundene Jobs werden nur einmal gespeichert – überlappende Läufe schaden nie.

**freelance.de** zeigt Projekttexte nur angemeldeten Nutzern: Einmal selbst anmelden
(*Portal-Zugänge → Anmelden…* oder wenn ein Lauf danach fragt), „angemeldet bleiben“ aktiviert lassen.
Die App trägt nichts ein und speichert kein freelance.de-Passwort; die Anmeldung lebt nur im eigenen
Profil des Anmeldefensters. **LinkedIn und freelancermap.de** brauchen kein Konto.

**Trockenlauf** zum Ausprobieren ohne Postfach, Portale und Dateien: `job-alert-monitor.exe --dry-run`.

## Arbeitsordner und Matching

Standard: `Dokumente\Job-Alert-Monitor` (änderbar unter *System → Arbeitsordner wählen…*).

```
Job-Alert-Monitor\
├─ auswertung\
│  ├─ JobAlerts.xlsx (bzw. .csv)      aus der Datenbank erzeugt, nie von Hand gepflegt
│  └─ beschreibungen_txt\*.txt        je Job mit Volltext genau eine Datei (für das Matching)
└─ profil\beraterprofil.json
```

Die Textdateien haben einen festen Kopf (`Titel`, `Unternehmen`, `Ort`, `Quelle`, `Link`,
`Abgerufen am`; `Quelle` heißt dort so, weil die Matching-Skills darauf zeigen – die App selbst sagt
„Portal“), dann eine Leerzeile und den Volltext. Jede Datei wird **einmal** geschrieben und nie
von selbst neu. Fremde Dateien im Arbeitsordner (z. B. `auswertung\beschreibungen_matching\`,
`Profil_*.json`, `tools\`) fasst die App nie an.

**Einmalig:** In den Matching-Skills (`job-matching`, `job-matching-email`) den Projektordner auf den
Arbeitsordner stellen. Den Arbeitsordner nicht auf den Ordner des alten Programms legen: Dessen
Textdateien würden das Matching sonst doppelt füttern. Findet die App dort eine fremde
`JobAlerts.xlsx`/`.csv`, sichert sie diese vor dem ersten Schreiben als `JobAlerts.alt-<Datum>.xlsx`.

Die Übersicht ist eine Ausgabe, kein Notizort: Excel wird bei jedem Lauf komplett neu geschrieben,
die CSV nur, wenn sich die Daten geändert haben (eine offene CSV stört so nicht bei jedem Lauf).
Das Gedächtnis der App ist ihre Datenbank.

## Schonender Abruf der Portale

Die App ruft Portale zurückhaltend ab – wie ein ruhiger Mensch, nicht wie ein Crawler:
strikt nacheinander, mit zufälligen Pausen, Obergrenzen und ohne Tarnung.

| Portal | Weg | Abstand | Obergrenze |
|---|---|---|---|
| LinkedIn | öffentliche Gastansicht, ohne Konto | 4–7 s | 20 je Stunde, 40 je 24 h |
| freelancermap.de | öffentliche Projektseite | 3–5 s | 25 je Stunde, 60 je 24 h |
| freelance.de | unsichtbares Anmeldefenster (echte Edge-Engine) | 10–20 s + Verweildauer | 15 je Stunde, 30 je 24 h |

- **Sperrsignal** (z. B. LinkedIn-Code 999, Umleitung zur Anmeldung, Captcha): Das Portal pausiert
  24 Stunden, beim zweiten Mal binnen einer Woche 7 Tage. Die App versucht nie, eine Sperre zu umgehen.
- **Zu viele Anfragen / Serverfehler:** 1 Stunde Pause. **Zwei Seiten ohne Beschreibung in Folge:**
  1 Stunde Pause (vermutlich hat sich der Seitenaufbau geändert).
- Pausen und Abrufzähler überleben Neustarts und auch *Alles zurücksetzen*. Ein erneuter
  Klick umgeht nichts; die Abschlussmeldung nennt, wann der Rest möglich ist.
- Erfolgreich Geholtes wird nie erneut abgerufen. Automatisch geholt werden Jobs aus Mails der letzten
  30 Tage; ältere per *Details holen* im Detailbereich.

Ein großer Rückstand braucht deshalb mehrere Tage – das ist gewollt.

## Speicherorte

| Was | Wo |
|---|---|
| Jobs, Volltexte, Einstellungen | `%LOCALAPPDATA%\de.cxecutives.job-alert-monitor\jobs.db` |
| Pausen und Abrufzähler | `…\policy.json` |
| Anmeldung freelance.de | `…\session-freelance\` |
| Protokoll (1 MB, eine Vorgängerdatei) | `…\logs\` – *System → Protokoll öffnen* |
| Gmail-Zugang | Windows-Anmeldeinformationsverwaltung, Eintrag `gmail.de.cxecutives.job-alert-monitor` |

## Fehlerbehebung

- **Windows blockiert den Start („Der Computer wurde durch Windows geschützt“):** Die Exe ist nicht
  signiert. *Weitere Informationen → Trotzdem ausführen.* Ist **Smart App Control** aktiv
  (*Windows-Sicherheit → App- & Browsersteuerung*), startet eine unsignierte Exe gar nicht; dann hilft
  nur, Smart App Control auszuschalten – je nach Windows-Version lässt es sich danach nur mit einer
  Neuinstallation wieder einschalten.
- **Die App startet nicht:** Die Fehlermeldung nennt Grund und Abhilfe (z. B. fehlende
  WebView2-Laufzeit, eine gesperrte oder beschädigte Datenbank samt Dateipfad, eine Datenbank aus einer
  neueren Version); Details stehen im Protokoll unter
  `%LOCALAPPDATA%\de.cxecutives.job-alert-monitor\logs\`.
- **„Die Datei ist gerade geöffnet (z. B. in Excel)“:** Excel schließen und erneut abrufen – es geht
  nichts verloren, die Datei wird beim nächsten Lauf geschrieben.
- **Gmail: Anmeldung abgelehnt:** App-Passwort neu anlegen und unter *System* speichern; IMAP in Gmail
  aktiviert?
- **Ein Portal ist pausiert:** *Portal-Zugänge* zeigt Grund und Ende. Bei einer Sperre das Portal im
  eigenen Browser öffnen (*Im Browser öffnen*) und nachsehen, ob eine Sicherheitsprüfung wartet.
- **freelance.de: „Anmeldung nötig“:** *Portal-Zugänge → Anmelden…*
- **Alert-Mails ohne Jobs (graue Zeilen):** Das Mail-Layout hat sich vermutlich geändert.
  Doppelklick öffnet die Mail in Gmail; bitte melden.
- **Alles zurücksetzen** löscht Jobs, Einstellungen, Gmail-Zugang, freelance.de-Anmeldung, das
  Beraterprofil und die App-Dateien im Ergebnisordner und startet neu. Pausen und Abrufzähler bleiben
  aus Sicherheitsgründen.

---

## Für Entwickler

Rust (Edition 2024) + Tauri 2, Oberfläche in reinem JavaScript ohne Build-Schritt.

```
core/        jobalert-core: die gesamte Fachlogik und alle Tests – kennt Tauri nicht
  src/mail/     IMAP (nur lesend), Mail-Zerlegung, Portal-Erkennung, Extraktion
  src/fetch/    Abrufregeln (policy.json), HTTP (LinkedIn, freelancermap), freelance.de-Auswertung
  src/export/   xlsx, csv, Textdateien (atomar)
  src/pipeline/ ein Lauf: Postfach → Jobdetails → Export; Trockenlauf-Attrappen
src-tauri/   dünne App-Schicht: Befehle, Fenster, Sitzungsfenster freelance.de
ui/          index.html, style.css, js/ (api.js ist die einzige Stelle mit Tauri-Zugriff)
tools/       ui-harness (Oberfläche im Browser mit nachgebautem Backend), icon.py (App-Symbol)
```

```bash
cargo test --workspace                                         # alle Tests (ohne Netz)
cargo clippy --workspace --all-targets -- -D warnings
cargo tauri dev -- -- --dry-run                                # App mit Attrappen
cargo build -p job-alert-monitor && target/debug/job-alert-monitor --dry-run --smoke --smoke-run
cargo tauri build                                              # target/release/job-alert-monitor.exe
node tools/ui-harness/server.mjs 5177                          # Oberfläche ohne Backend: http://localhost:5177
python tools/icon.py                                           # App-Symbol neu erzeugen (Pillow)
cargo run -p jobalert-core --example dump_alerts               # eigene Alert-Mails als Testdaten sichern
```

Im Prüfstand laufen die Interaktions-Szenarien in der Browser-Konsole:
`await (await import('/__checks.js')).run()` (URL-Parameter: `?jobs=5000`, `?dry`, `?nogmail`, `?first`).

Der Push-Wächter in `.githooks/pre-push` lässt nur Fast-Forward-Pushes nach `CXecutives/TEST` zu.
Einmal pro Klon aktivieren: `git config core.hooksPath .githooks`.

`--smoke` (nur Debug-Build) prüft die Oberfläche und beendet sich mit 0/1/2; `--smoke-run` klickt im
Trockenlauf zusätzlich „Postfach abrufen“ und prüft das Ergebnis. Tests berühren weder Portale noch den
echten Tresor-Eintrag.
