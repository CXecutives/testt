# Bewertungsregel

Diese Regel gilt für die Prüfung in einem KI-Chat aus der App und für den Skill job-matching.
Beide lesen genau diesen Text. Das Profil gewinnt: Jede Schwelle kommt aus dem Profil, ein Schlüssel,
den das Profil nicht setzt, schaltet seine Regel ab.

Die weiteren Anzeigen, die ein Portal unter einer Anzeige zeigt (etwa „Ähnliche Projekte“ oder
„Similar jobs“), gehören nicht zu ihr. Keine Regel liest sie, und sie schließen nichts aus.

## Punkte von 1 bis 10

- 9 bis 10 Kernfeld, alle Muss-Punkte erfüllt, mindestens ein Schwerpunkt erfüllt
- 7 bis 8 Kernfeld, kleine Lücken oder überqualifiziert
- 5 bis 6 ein Muss-Punkt offen oder eine Festanstellung mit offenen Rahmenpunkten
- 3 bis 4 mehrere Muss-Punkte oder eine formale Pflicht offen
- 1 bis 2 außerhalb des Fachgebiets

Nennt das Profil keine Schwerpunkte, entfällt die Bedingung mit dem Schwerpunkt. Ein fehlender
Kann-Punkt kostet höchstens einen halben Punkt. Die Entfernung kostet bei Interim nichts.

## Obergrenzen

- Eine formale Pflicht offen (Abschluss, Zulassung, Zertifikat) höchstens 4
- Mindestens zwei und mindestens die Hälfte der Muss-Punkte offen höchstens 4
- Ein Muss-Punkt oder ein gezählter Rahmenpunkt offen höchstens 6
- Kein Muss-Punkt erfüllt höchstens 3
- Nichts offen mindestens 4
- Ein gezeigter Job mindestens 2

Der Einsatzort einer Interim-Rolle zählt nicht als Rahmenpunkt.

## Vertragsart und ANÜ

- Interim (Tagessatz, freiberuflich, Werkvertrag, Contract) ist die erste Kategorie.
- Eine Festanstellung kostet etwa einen Punkt, außer das Profil sucht nur Festanstellungen.
- Eine Personalagentur ohne Angaben zur Vertragsart ist unklar. Sie wird wie eine Festanstellung
  geprüft, und das Risiko der Arbeitnehmerüberlassung wird genannt.
- Arbeitnehmerüberlassung (auch Überlassung, Payrolling, Equal Pay, iGZ, BAP) schließt aus, wenn
  das Profil sie ausschließt. Als eine von mehreren Möglichkeiten ist sie ein offener Punkt.
- Eine unklare Vertragsart ist ein Punkt zum Prüfen, nie ein Ausschluss.

## Ausschluss

- Ein Ausschluss braucht ein wörtliches Zitat aus der Anzeige, das ihn belegt.
- Nur harte Kriterien schließen aus. Das sind Arbeitnehmerüberlassung, ein Tagessatz unter
  min_tagessatz, ein Land außerhalb von laender, ein Gehalt unter min_jahresgehalt oder ein Ort
  außerhalb der Region bei einer genannten Festanstellung, weniger verlangte Jahre als
  zielprofil_min_jahre und eine zwingende formale Pflicht.
- Ein Wunsch ist nie ein Grund für einen Ausschluss.

## Schwerpunkte, Wunschrollen und Wünsche

- Schwerpunkte (schwerpunkte) sind die drei bis fünf Kernkompetenzen. Ein Muss-Punkt, den ein
  Schwerpunkt erfüllt, zählt doppelt.
- Eine passende Wunschrolle (wunschrollen) ist ein Plus von höchstens einem Punkt. Sie hebt keine
  Obergrenze auf.
- Wünsche (tagessatz_wunsch, remote, regionen und branchen in einsatzpraeferenzen) schließen nie
  aus und bewegen die Bewertung zusammen höchstens einen Punkt nach oben oder unten.
- tagessatz_wunsch ist ein Wunsch und keine Untergrenze. Die Untergrenze ist min_tagessatz.
- Nennt die Anzeige einen Wert nicht, zählt der Wunsch nicht.
- Wunschrolle und Wünsche heben keinen Job auf 8 oder mehr, wenn weniger als die Hälfte der
  Muss-Punkte erfüllt ist.
