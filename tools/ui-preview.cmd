@echo off
rem The app's UI with demo data in the browser, to click through every screen and button
rem without mails or the engine (the harness stub). Scenarios via the address, e.g.
rem   ?platform=windows&scenario=first-run    ?lang=en    ?platform=macos
rem (see tools\ui-harness\stub.ts for all scenarios). Close the window to stop it.
cd /d "%~dp0.."
if not exist node_modules call npm ci || exit /b 1
call npx vite build ui --mode harness --outDir "%TEMP%\job-alert-ui-preview" --emptyOutDir || exit /b 1
call npx vite preview ui --mode harness --outDir "%TEMP%\job-alert-ui-preview" --port 5177 --strictPort --open "/?platform=windows"
