//! Architecture rules of the UI (`ui/src`, Svelte 5 + TypeScript). They run without Node,
//! so `cargo test` alone keeps them: each rule protects a promise of the design system
//! that ESLint/Stylelint also enforce, or one that no linter can see (gallery coverage,
//! per-OS markup, the release build without gallery).
//!
//! Every test asserts how many files it scanned: a moved folder must not turn a rule into
//! a silent no-op.

use std::path::{Path, PathBuf};

/// File kinds of the UI source.
const EXTENSIONS: [&str; 3] = ["svelte", "ts", "css"];
/// Lower bounds of the scan (the foundation has more than this).
const MIN_FILES: usize = 55;
const MIN_SVELTE: usize = 35;
const MIN_COMPONENTS: usize = 24;

fn repo(relative: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join(relative)
}

struct Source {
    /// Path relative to `ui/src`, with forward slashes.
    path: String,
    ext: String,
    /// The text with comments blanked out (line numbers stay valid).
    code: String,
}

impl Source {
    fn is(&self, path: &str) -> bool {
        self.path == path
    }

    fn under(&self, dir: &str) -> bool {
        self.path.starts_with(dir)
    }

    fn lines(&self) -> impl Iterator<Item = (usize, &str)> {
        self.code.lines().enumerate().map(|(i, l)| (i + 1, l))
    }
}

fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            walk(&path, out);
        } else {
            out.push(path);
        }
    }
}

fn sources() -> Vec<Source> {
    let root = repo("ui/src");
    let mut paths = Vec::new();
    walk(&root, &mut paths);
    let mut out: Vec<Source> = paths
        .into_iter()
        .filter_map(|path| {
            let ext = path.extension()?.to_str()?.to_string();
            if !EXTENSIONS.contains(&ext.as_str()) {
                return None;
            }
            let text = std::fs::read_to_string(&path).ok()?;
            let rel = path
                .strip_prefix(&root)
                .ok()?
                .to_string_lossy()
                .replace('\\', "/");
            Some(Source {
                path: rel,
                code: strip_comments(&text),
                ext,
            })
        })
        .collect();
    out.sort_by(|a, b| a.path.cmp(&b.path));
    out
}

/// All sources, after checking that the scan found the UI at all.
fn scanned(min: usize) -> Vec<Source> {
    let all = sources();
    assert!(
        all.len() >= min,
        "only {} files scanned in ui/src (expected at least {min}) - did the UI move?",
        all.len()
    );
    all
}

/// Blank out `/* */`, `<!-- -->` and `//` comments, keeping newlines. A `//` only starts a
/// comment at the line start or after whitespace (URLs such as `https://` stay).
fn strip_comments(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len());
    let mut i = 0;
    let starts = |i: usize, pat: &str| {
        pat.chars()
            .enumerate()
            .all(|(k, c)| chars.get(i + k) == Some(&c))
    };
    while i < chars.len() {
        let end = if starts(i, "/*") {
            Some("*/")
        } else if starts(i, "<!--") {
            Some("-->")
        } else {
            None
        };
        if let Some(end) = end {
            while i < chars.len() && !starts(i, end) {
                out.push(if chars[i] == '\n' { '\n' } else { ' ' });
                i += 1;
            }
            for _ in 0..end.len() {
                if i < chars.len() {
                    out.push(' ');
                    i += 1;
                }
            }
            continue;
        }
        let line_comment = starts(i, "//") && (i == 0 || chars[i - 1].is_whitespace());
        if line_comment {
            while i < chars.len() && chars[i] != '\n' {
                out.push(' ');
                i += 1;
            }
            continue;
        }
        out.push(chars[i]);
        i += 1;
    }
    out
}

/// The markup of a `.svelte` file: script and style blocks blanked out.
fn markup(code: &str) -> String {
    let mut out = code.to_string();
    for (open, close) in [("<script", "</script>"), ("<style", "</style>")] {
        while let Some(start) = out.find(open) {
            let Some(len) = out[start..].find(close) else {
                break;
            };
            let end = start + len + close.len();
            let blank: String = out[start..end]
                .chars()
                .map(|c| if c == '\n' { '\n' } else { ' ' })
                .collect();
            out.replace_range(start..end, &blank);
        }
    }
    out
}

/// Blank `{...}` expressions (nested braces included), keeping newlines.
fn without_expressions(text: &str) -> String {
    let mut depth = 0usize;
    text.chars()
        .map(|c| {
            let inside = depth > 0 || c == '{';
            if c == '{' {
                depth += 1;
            } else if c == '}' && depth > 0 {
                depth -= 1;
                return ' ';
            }
            if inside && c != '\n' { ' ' } else { c }
        })
        .collect()
}

fn fail(problems: &[String], rule: &str) {
    assert!(problems.is_empty(), "{rule}:\n  {}", problems.join("\n  "));
}

/// Lines of files outside `allowed` that contain one of `needles`.
fn find(all: &[Source], needles: &[&str], allowed: impl Fn(&Source) -> bool) -> Vec<String> {
    let mut problems = Vec::new();
    for source in all.iter().filter(|s| !allowed(s)) {
        for (n, line) in source.lines() {
            for needle in needles {
                if line.contains(needle) {
                    problems.push(format!("{}:{n}: {needle}", source.path));
                }
            }
        }
    }
    problems
}

#[test]
fn tauri_only_in_api() {
    let all = scanned(MIN_FILES);
    let mut problems = find(&all, &["@tauri-apps/", "__TAURI"], |s| {
        s.is("lib/ipc/api.ts") || s.under("lib/ipc/types/")
    });
    // The generated types may name Tauri's Channel type - as a type-only import, which
    // leaves no runtime access behind.
    for source in all.iter().filter(|s| s.under("lib/ipc/types/")) {
        for (n, line) in source.lines() {
            if line.contains("@tauri-apps/") && !line.trim_start().starts_with("import type") {
                problems.push(format!("{}:{n}: runtime import of Tauri", source.path));
            }
        }
    }
    fail(
        &problems,
        "Tauri is reached only through lib/ipc/api.ts (the harness swaps exactly that door)",
    );
}

#[test]
fn lucide_only_in_icon() {
    let all = scanned(MIN_FILES);
    fail(
        &find(&all, &["@lucide", "lucide-svelte"], |s| {
            s.is("components/Icon.svelte")
        }),
        "icons come only from components/Icon.svelte",
    );
}

#[test]
fn motion_libraries_only_in_lib_motion() {
    let all = scanned(MIN_FILES);
    fail(
        &find(
            &all,
            &[
                "svelte/transition",
                "svelte/animate",
                "svelte/motion",
                "svelte/easing",
                ".animate(",
            ],
            |s| s.under("lib/motion/"),
        ),
        "motion goes through lib/motion/ (tokens, reduced motion)",
    );
}

#[test]
fn raw_controls_only_in_components() {
    let all = scanned(MIN_FILES);
    let svelte: Vec<&Source> = all.iter().filter(|s| s.ext == "svelte").collect();
    assert!(
        svelte.len() >= MIN_SVELTE,
        "only {} .svelte files",
        svelte.len()
    );
    let mut problems = Vec::new();
    for source in svelte.iter().filter(|s| !s.under("components/")) {
        let text = without_expressions(&markup(&source.code));
        for (n, line) in text.lines().enumerate() {
            for tag in [
                "button", "input", "textarea", "select", "a", "svg", "img", "dialog",
            ] {
                let open = format!("<{tag}");
                for (at, _) in line.match_indices(&open) {
                    let next = line[at + open.len()..].chars().next();
                    if next.is_none_or(|c| c.is_whitespace() || c == '>' || c == '/') {
                        problems.push(format!("{}:{}: <{tag}>", source.path, n + 1));
                    }
                }
            }
        }
    }
    fail(
        &problems,
        "raw controls only inside components/ (use the design system)",
    );
}

#[test]
fn no_title_attribute_and_no_inline_style() {
    let all = scanned(MIN_FILES);
    let mut problems = Vec::new();
    for source in &all {
        let text = if source.ext == "svelte" {
            without_expressions(&markup(&source.code))
        } else {
            String::new()
        };
        for (n, line) in text.lines().enumerate() {
            for bad in [" title=", " style=", " style:", "\ttitle=", "\tstyle="] {
                if line.contains(bad) {
                    problems.push(format!("{}:{}: {}", source.path, n + 1, bad.trim()));
                }
            }
        }
        for (n, line) in source.lines() {
            for bad in [
                "{title}",
                "style:",
                "cssText",
                "setAttribute('style'",
                "setAttribute('title'",
                ".title =",
            ] {
                // `style:` inside CSS (`font-style:` etc.) is fine; only markup directives count.
                if bad == "style:" && source.ext != "svelte" {
                    continue;
                }
                if bad == "style:" && !line.contains(" style:") {
                    continue;
                }
                if line.contains(bad) {
                    problems.push(format!("{}:{n}: {bad}", source.path));
                }
            }
        }
    }
    fail(
        &problems,
        "no native title (use the tooltip action) and no inline style (CSP; use cssVars)",
    );
}

#[test]
fn no_html_injection() {
    let all = scanned(MIN_FILES);
    fail(
        &find(
            &all,
            &[
                "{@html",
                "innerHTML",
                "outerHTML",
                "insertAdjacentHTML",
                "document.write",
            ],
            |_| false,
        ),
        "texts from mails are never inserted as HTML",
    );
}

#[test]
fn keyframes_only_in_motion_css() {
    let all = scanned(MIN_FILES);
    fail(
        &find(&all, &["@keyframes"], |s| s.is("styles/motion.css")),
        "@keyframes live only in styles/motion.css",
    );
}

#[test]
fn forbidden_css_features() {
    let all = scanned(MIN_FILES);
    let mut problems = find(
        &all,
        &[
            "prefers-color-scheme",
            "view-transition",
            "@starting-style",
            "scrollbar-gutter",
            "content-visibility",
            "!important",
        ],
        |_| false,
    );
    problems.extend(find(&all, &["color-mix("], |s| s.is("styles/tokens.css")));
    fail(
        &problems,
        "light mode only, Safari 17 baseline (no View Transitions, @starting-style, \
         scrollbar-gutter, content-visibility), colour maths only in tokens.css",
    );
}

#[test]
fn palette_only_in_tokens() {
    let all = scanned(MIN_FILES);
    fail(
        &find(&all, &["var(--p-"], |s| s.is("styles/tokens.css")),
        "palette tokens (--p-*) are used only inside tokens.css; components use semantic tokens",
    );
}

#[test]
fn input_listeners_only_in_input_ts() {
    let all = scanned(MIN_FILES);
    let events = [
        "keydown",
        "keyup",
        "keypress",
        "contextmenu",
        "auxclick",
        "dblclick",
        "dragstart",
        "selectstart",
        "wheel",
        "gesturestart",
        "gesturechange",
    ];
    let needles: Vec<String> = events
        .iter()
        .flat_map(|e| [format!("'{e}'"), format!("\"{e}\""), format!("on{e}")])
        .collect();
    let needles: Vec<&str> = needles.iter().map(String::as_str).collect();
    fail(
        &find(&all, &needles, |s| s.is("lib/input/input.ts")),
        "key, context-menu, aux-button, wheel and gesture handling only in lib/input/input.ts",
    );
    let input = all
        .iter()
        .find(|s| s.is("lib/input/input.ts"))
        .expect("lib/input/input.ts");
    for event in [
        "keydown",
        "contextmenu",
        "auxclick",
        "dblclick",
        "dragstart",
        "wheel",
    ] {
        assert!(
            input.code.contains(&format!("'{event}'")),
            "input.ts does not handle {event}"
        );
    }
}

#[test]
fn every_component_is_in_the_gallery() {
    let all = scanned(MIN_FILES);
    let components: Vec<&Source> = all.iter().filter(|s| s.under("components/")).collect();
    assert!(
        components.len() >= MIN_COMPONENTS,
        "only {} components scanned",
        components.len()
    );
    let gallery: String = all
        .iter()
        .filter(|s| s.under("features/gallery/"))
        .map(|s| s.code.as_str())
        .collect();
    let missing: Vec<String> = components
        .iter()
        .filter(|c| !gallery.contains(&format!("$components/{}", &c.path["components/".len()..])))
        .map(|c| c.path.clone())
        .collect();
    fail(
        &missing,
        "every component appears in the gallery (features/gallery/)",
    );
}

/// Letter-bearing text in markup outside `{...}` expressions, or as a literal value of a
/// text prop. Texts come from `lib/i18n/de.ts` (gallery samples from its `gallery.ts`).
#[test]
fn no_text_literals_in_markup() {
    let all = scanned(MIN_FILES);
    let props = [
        "label=\"",
        "heading=\"",
        "text=\"",
        "placeholder=\"",
        "aria-label=\"",
        "alt=\"",
        "disabledReason=\"",
        "hint=\"",
        "message=\"",
    ];
    let mut problems = Vec::new();
    for source in all.iter().filter(|s| s.ext == "svelte") {
        let text = without_expressions(&markup(&source.code));
        // Text between tags: blank everything inside <...> (tags span lines).
        let mut depth = 0usize;
        let outside: String = text
            .chars()
            .map(|c| match c {
                '<' => {
                    depth += 1;
                    ' '
                }
                '>' => {
                    depth = depth.saturating_sub(1);
                    ' '
                }
                '\n' => '\n',
                _ if depth > 0 => ' ',
                _ => c,
            })
            .collect();
        for (n, line) in outside.lines().enumerate() {
            if line.chars().any(char::is_alphabetic) {
                problems.push(format!("{}:{}: \"{}\"", source.path, n + 1, line.trim()));
            }
        }
        for (n, line) in text.lines().enumerate() {
            for prop in props {
                for (at, _) in line.match_indices(prop) {
                    let value: String = line[at + prop.len()..]
                        .chars()
                        .take_while(|c| *c != '"')
                        .collect();
                    if value.chars().any(char::is_alphabetic) {
                        problems.push(format!("{}:{}: {prop}{value}\"", source.path, n + 1));
                    }
                }
            }
        }
    }
    fail(&problems, "UI text only from lib/i18n/de.ts");
}

#[test]
fn per_os_markup_only_in_the_title_bar() {
    let all = scanned(MIN_FILES);
    fail(
        &find(
            &all,
            &[
                "'macos'",
                "\"macos\"",
                "'windows'",
                "\"windows\"",
                "userAgent",
                "data-platform",
            ],
            |s| {
                s.is("features/shell/TitleBar.svelte")
                    || s.is("components/WindowControls.svelte")
                    || s.is("lib/platform.ts")
                    // Font smoothing on macOS only (documented platform difference).
                    || s.is("styles/base.css")
                    // `AppState.platform` is part of the IPC contract.
                    || s.under("lib/ipc/types/")
            },
        ),
        "per-OS differences live only in TitleBar, WindowControls and platform.ts",
    );
    // The Windows icon font draws the native caption glyphs - nowhere else (macOS has none).
    fail(
        &find(&all, &["Segoe Fluent", "Segoe MDL2"], |s| {
            s.is("styles/tokens.css")
        }),
        "the Windows icon font is only named in its token",
    );
    fail(
        &find(&all, &["var(--font-caption)"], |s| {
            s.is("components/WindowControls.svelte")
        }),
        "--font-caption is used only by WindowControls",
    );
}

/// Coral-only brand (user decision): no second hue in a gradient, no gradient text, no
/// "sparkles" cliché.
#[test]
fn the_brand_stays_coral() {
    let all = scanned(MIN_FILES);
    let tokens = all
        .iter()
        .find(|s| s.is("styles/tokens.css"))
        .expect("styles/tokens.css");
    let mut problems = Vec::new();
    let mut in_gradient = false;
    for (n, line) in tokens.lines() {
        if line.contains("gradient(") {
            in_gradient = true;
        }
        if in_gradient
            && [
                "--p-slate",
                "--p-success",
                "--p-info",
                "--p-warning",
                "--p-danger",
            ]
            .iter()
            .any(|hue| line.contains(hue))
        {
            problems.push(format!("tokens.css:{n}: second hue in a gradient"));
        }
        if line.contains(';') {
            in_gradient = false;
        }
    }
    problems.extend(find(
        &all,
        &["background-clip: text", "'sparkles'", "\"sparkles\""],
        |_| false,
    ));
    fail(&problems, "coral-only brand");
}

#[test]
fn at_most_one_primary_button_per_view() {
    let all = scanned(MIN_FILES);
    let mut problems = Vec::new();
    for source in all
        .iter()
        .filter(|s| s.ext == "svelte" && s.under("features/") && !s.under("features/gallery/"))
    {
        let count = source.code.matches("variant=\"primary\"").count();
        if count > 1 {
            problems.push(format!("{}: {count} primary buttons", source.path));
        }
    }
    fail(&problems, "at most one primary button per view");
}

#[test]
fn no_leftovers_from_development() {
    let all = scanned(MIN_FILES);
    fail(
        &find(
            &all,
            &["console.", "debugger", "TODO", "FIXME", "XXX"],
            |_| false,
        ),
        "no debugging leftovers in the UI",
    );
}

/// One word per thing (glossary in docs/PLAN.md); short texts, no walls of text.
#[test]
fn the_catalog_keeps_the_glossary() {
    let all = scanned(MIN_FILES);
    let catalog = all
        .iter()
        .find(|s| s.is("lib/i18n/de.ts"))
        .expect("lib/i18n/de.ts");
    let mut problems = Vec::new();
    for (n, line) in catalog.lines() {
        for (old, new) in [
            ("Quelle", "Portal"),
            ("Eintrag", "Job"),
            ("Volltext", "Details"),
            ("Kandidat", "Job"),
            ("Treffer", "Passung"),
            ("Mailbox", "Postfach"),
        ] {
            // Only the words the user reads count (keys like `fullMailbox` are code).
            if literals(line).iter().any(|text| words(text).contains(old)) {
                problems.push(format!("de.ts:{n}: \"{old}\" is called \"{new}\""));
            }
        }
        for quoted in line.split('\'').skip(1).step_by(2) {
            if quoted.chars().count() > 140 {
                problems.push(format!(
                    "de.ts:{n}: {} characters (max 140)",
                    quoted.chars().count()
                ));
            }
        }
    }
    fail(&problems, "glossary and length of the UI catalog");
}

/// The string literals of one line: between single quotes and between backticks.
fn literals(line: &str) -> Vec<&str> {
    let mut out = Vec::new();
    for quote in ['\'', '`'] {
        out.extend(line.split(quote).skip(1).step_by(2));
    }
    out
}

/// The words of a literal: `${...}` expressions of a template blanked out.
fn words(text: &str) -> String {
    let mut out = String::new();
    let mut depth = 0usize;
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if depth == 0 && c == '$' && chars.peek() == Some(&'{') {
            chars.next();
            depth = 1;
            out.push(' ');
        } else if depth > 0 {
            match c {
                '{' => depth += 1,
                '}' => depth -= 1,
                _ => {}
            }
        } else {
            out.push(c);
        }
    }
    out
}

/// The catalog speaks plainly (CLAUDE.md): no dash or em dash as a separator, no colon at
/// the end of a label or heading, no "X: Y" construction, no exclamation mark.
#[test]
fn the_catalog_has_no_ai_punctuation() {
    let all = scanned(MIN_FILES);
    let catalog = all
        .iter()
        .find(|s| s.is("lib/i18n/de.ts"))
        .expect("lib/i18n/de.ts");
    let mut problems = Vec::new();
    let mut strings = 0;
    for (n, line) in catalog.lines() {
        for literal in literals(line) {
            let text = words(literal);
            let text = text.as_str();
            strings += 1;
            for dash in [" - ", " – ", "–", "—"] {
                if text.contains(dash) {
                    problems.push(format!("de.ts:{n}: dash as a separator in \"{text}\""));
                }
            }
            if text.trim_end().ends_with(':') {
                problems.push(format!("de.ts:{n}: colon at the end of \"{text}\""));
            }
            if text.contains(": ") {
                problems.push(format!("de.ts:{n}: \"X: Y\" in \"{text}\""));
            }
            let mut chars = text.chars().peekable();
            while let Some(c) = chars.next() {
                if c == '!' && chars.peek() != Some(&'=') {
                    problems.push(format!("de.ts:{n}: exclamation mark in \"{text}\""));
                }
            }
        }
    }
    assert!(strings >= 200, "only {strings} strings in de.ts");
    fail(&problems, "plain punctuation in the UI catalog");
}

/// The release build must not ship the gallery (it is compiled out via `__GALLERY__`).
#[test]
fn the_release_build_has_no_gallery() {
    scanned(MIN_FILES);
    let dist = repo("ui/dist");
    if !dist.exists() {
        return;
    }
    let mut files = Vec::new();
    walk(&dist, &mut files);
    let mut problems = Vec::new();
    let mut checked = 0;
    for file in files {
        let Some(ext) = file.extension().and_then(|e| e.to_str()) else {
            continue;
        };
        if !["js", "css", "html"].contains(&ext) {
            continue;
        }
        checked += 1;
        let text = std::fs::read_to_string(&file).unwrap_or_default();
        for bad in ["Galerie", "gallery", "ColourBoard", "MotionBoard"] {
            if text.contains(bad) {
                problems.push(format!("{}: {bad}", file.display()));
            }
        }
    }
    assert!(
        checked >= 2,
        "ui/dist exists but holds no build ({checked} files)"
    );
    fail(&problems, "the gallery is development-only");
}
