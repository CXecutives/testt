# Wave 2 progress

One line per item: done, skipped (why) or open.

## Setup
- Linux container: Rust, Node and the Tauri system packages installed; Chromium for the harness from Google's Chrome
  for Testing store. WebKit cannot be downloaded here (the Playwright CDN is blocked), so the harness ran in Chromium
  only; WebKit runs in CI. Three core tests need an OS keychain and fail on Linux only
  (`reset::tests::removes_only_app_data_and_keeps_policy_and_foreign_files`, `reset::tests::leftovers_of_earlier_attempts_are_swept`,
  `profile::tests::the_previous_file_becomes_the_one_backup`); they pass on Windows and macOS.
- Branch: the session's `claude/hopeful-fermat-0x79s0` instead of `wave2` (the session may push only there).

## 5.1 Features
- features-01 done: the reader's Rahmen strip shows the ad's rate and start as plain chips where no criterion covers them.
- features-02 done: the search matches every word (at most 8) in any field plus the portal's name; list, counts and
  "all read" alike; stored rows get the new search column once (`search_version`).
- features-07 done: the tile and portal filter code is gone.
- features-05 not done: docs/wave2/PLAN.md lists only 01, 02 and 07 for this wave (section 6: no features beyond 5.1).

## 5.2 Findings
### High
- ui-forms-01 done: a refused value of the hidden Festanstellung block brings the block back with the error and caret.
- export-matching-01 done: engine 15, whole headings of the other listings only.
- ui-jobs-01 done: keys, selection and next job count rows in the drawn order.
- live-forms-01 done: the AI's answer lives in the editor state; reopening leaves the clipboard alone. Open: closing
  the window with only an answer held does not ask (the leave dialog's "Speichern" has nothing to save).
- live-forms-02 done: typed chip text is a change (Speichern, leaving, closing).

### Medium
- ui-forms-02 done: Ctrl/Cmd+S takes the typed chip text in first.
- live-forms-03 done: a decimal part is cut, never joined.
- live-forms-04 done: the refusal carries the limit ("Höchstens 100.000.") and no longer repeats the label.
- live-forms-05 done: a dialog keeps a focus its action moved.
- export-matching-06 done: the ENGINE_VERSION doc names 12 to 15.
- backend-01 done: the new original keeps the later inbox_at of the teaser it replaces.
- backend-04 done: at the start, scores go only when the profile is certainly unusable.
- backend-05 skipped: fixed before (the overview shows favourites and new matches, `new_total` feeds "n more").
- backend-06, export-matching-04 done: Excel and the overview use the app's display title.
- export-matching-05 done: the overview's waiting ring matches the app's.
- ui-forms-03, export-matching-02 done: every offered country name reads back to its code (engine 15).
- export-matching-03 done: assess-level tests for engines 12 and 13; held-out set 7's floor at engine 12's level.
- names-01, live-jobs-13 done: "gelöscht"/"deleted" only for deleted forever; the trash sort is "Nach Datum".
- names-02 done: "Job" (en "job") for the listed position in both catalogs and the export texts; "Role" only for the
  consultant's own field. The AI prompt's wording is left as it is.
- names-03 done: hints say "ausgeschlossen" instead of "fällt weg".
- names-04 done: "Von dir trotzdem gewertet." / "You included this job anyway."
- names-05 done: the Profil page's reading uses the form's labels; "Orte für Festanstellung" instead of "Region";
  "ab 15 Jahren".
- names-06 skipped: fixed before (HTML_NEW is "Neu und passend").
- names-07, live-forms-09 done: the section is "Automatisch" / "Automatic".
- names-09 done: en reader.frame "Conditions".
- backend-02 done: "Diese Details holt die App nur auf Anfrage."
