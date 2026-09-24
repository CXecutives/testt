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
        "mousedown",
        "mouseup",
        "click",
        "auxclick",
        "dblclick",
        "dragstart",
        "selectstart",
        "wheel",
    ] {
        assert!(
            input.code.contains(&format!("'{event}'")),
            "input.ts does not handle {event}"
        );
    }
    // Text a user would copy is marked `data-copy`: selectable (base.css) and let through
    // by the input policy (selection and Ctrl/Cmd+C).
    assert!(
        input.code.contains("[data-copy]"),
        "input.ts does not know the copyable text"
    );
    let base = all
        .iter()
        .find(|s| s.is("styles/base.css"))
        .expect("styles/base.css");
    assert!(
        base.code.contains("[data-copy]"),
        "base.css does not make the copyable text selectable"
    );
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

/// The keyboard stays native (audit 2026-09-24): fields take every character the layout
/// types, including `AltGr` (Windows) and Option (macOS, where @ is Option+L on a German
/// keyboard), and the editing keys of the OS; the macOS menu keeps its Cmd shortcuts
/// (Cmd+, too); Tab and Enter/Space work on controls; a modal dialog holds the focus; the
/// zoom guard is a wheel listener that exists only while Ctrl/Cmd is held. The behaviour
/// itself is tested in tools/ui-harness/specs/input.spec.ts on both engines.
#[test]
fn the_keyboard_stays_native() {
    let all = scanned(MIN_FILES);
    let file = |path: &str| -> &Source {
        all.iter()
            .find(|s| s.is(path))
            .unwrap_or_else(|| panic!("{path} missing"))
    };
    let input = &file("lib/input/input.ts").code;
    let platform = &file("lib/platform.ts").code;
    let mut problems = Vec::new();
    let mut need = |ok: bool, what: &str| {
        if !ok {
            problems.push(what.to_string());
        }
    };
    need(
        input.contains("getModifierState('AltGraph')") && input.contains("optionTypes"),
        "input.ts: AltGr and Option characters must type in fields",
    );
    need(
        platform.contains("optionTypes: mac"),
        "platform.ts: Option types characters on macOS",
    );
    need(
        input
            .lines()
            .any(|l| l.contains("MAC_MENU_KEYS = new Set(") && l.contains("','")),
        "input.ts: Cmd+, (Settings) must reach the macOS menu",
    );
    need(
        input.contains("EDITING_KEYS") && input.contains("redoWithY"),
        "input.ts: the editing keys of native fields (word, line, redo)",
    );
    need(
        input.contains("isFocusMove") && input.contains("pressesControl"),
        "input.ts: Tab moves the focus and Enter/Space press controls everywhere",
    );
    need(
        input.contains("[aria-modal=\"true\"]") && input.contains("cycleFocus"),
        "input.ts: a modal dialog holds the focus",
    );
    need(
        input.contains("removeEventListener('wheel'")
            && !input.contains("addEventListener(\n    'wheel'")
            && input.matches("addEventListener('wheel'").count() == 1,
        "input.ts: the non-passive wheel listener is attached only while Ctrl/Cmd is held",
    );
    let dialog = &file("components/Dialog.svelte").code;
    need(
        dialog.contains("aria-modal=\"true\"") && dialog.contains("tabindex=\"-1\""),
        "Dialog.svelte: modal and focusable (a click on its text keeps the focus inside)",
    );
    let field = &file("components/TextField.svelte").code;
    need(
        field.matches("inField").count() >= 2 && field.contains("use:formKeys"),
        "TextField.svelte: in-field buttons keep the caret; a search clears on Esc",
    );
    fail(&problems, "the keyboard stays native");
}

#[test]
fn per_os_code_only_in_platform_ts() {
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
                s.is("lib/platform.ts")
                    // Font smoothing and the scrollbars of the OS (documented differences).
                    || s.is("styles/base.css")
                    // `AppState.platform` is part of the IPC contract.
                    || s.under("lib/ipc/types/")
            },
        ),
        "per-OS differences live only in platform.ts (and base.css for font smoothing and \
         scrollbars); components ask platform.ts",
    );
}

fn config(name: &str) -> serde_json::Value {
    serde_json::from_str(&std::fs::read_to_string(repo(&format!("src-tauri/{name}"))).expect(name))
        .unwrap_or_else(|e| panic!("{name}: {e}"))
}

/// Both OS show their native window frame (title bar, caption buttons, system menu, snap
/// layouts); the page draws no title bar and no caption buttons. Windows: the native bar
/// above the page (coloured in platform.rs). macOS: the unified toolbar row of a Mac app -
/// the title bar transparent over the page, the title hidden, the traffic lights moved into
/// the 52 px row; the page keeps that row free and marks its empty parts as drag regions
/// (only `DragBand` and the list's first row, through Tauri's drag script).
#[test]
fn the_window_frame_is_native_on_both_os() {
    let all = scanned(MIN_FILES);
    fail(
        &find(&all, &["data-tauri-drag-region"], |s| {
            s.is("components/DragBand.svelte") || s.is("features/jobs/ListHeader.svelte")
        }),
        "drag regions only in DragBand and the list's toolbar row",
    );
    fail(
        &find(
            &all,
            &[
                "app-region",
                "@tauri-apps/api/window",
                "getCurrentWindow",
                "startDragging",
                "Segoe Fluent",
                "Segoe MDL2",
            ],
            |_| false,
        ),
        "no title bar and no caption buttons in the page: the frame of the OS carries them",
    );
    // The list row sets the attribute only where dragBands() says so (macOS).
    let header = all
        .iter()
        .find(|s| s.is("features/jobs/ListHeader.svelte"))
        .expect("ListHeader.svelte");
    assert!(
        header
            .code
            .contains("data-tauri-drag-region={dragBands() ? '' : undefined}"),
        "the list row is a drag region on macOS only"
    );

    let shared = config("tauri.conf.json");
    let window = &shared["app"]["windows"][0];
    assert_eq!(
        window["decorations"], true,
        "Windows keeps the native frame"
    );
    let windows = std::fs::read_to_string(repo("src-tauri/tauri.windows.conf.json"))
        .expect("tauri.windows.conf.json");
    for bad in ["decorations", "titleBarStyle", "hiddenTitle"] {
        assert!(!windows.contains(bad), "tauri.windows.conf.json: {bad}");
    }
    let macos = config("tauri.macos.conf.json");
    let mac = &macos["app"]["windows"][0];
    assert_eq!(mac["decorations"], true, "macOS keeps its native frame");
    assert_eq!(mac["titleBarStyle"], "Overlay", "unified toolbar row");
    assert_eq!(mac["hiddenTitle"], true, "the title stays set but hidden");
    assert_eq!(
        mac["trafficLightPosition"]["x"], 20,
        "traffic lights at x 20"
    );
    // The platform file replaces the window array: apart from the title bar it is the same
    // window as the shared one.
    let mut same = mac.clone();
    for key in ["titleBarStyle", "hiddenTitle", "trafficLightPosition"] {
        same.as_object_mut().expect("window").remove(key);
    }
    assert_eq!(&same, window, "one window, two title bars");
    // Small enough to snap into every Windows 11 layout, quarters of 1366 x 768 included.
    let (min_width, min_height) = (window["minWidth"].as_u64(), window["minHeight"].as_u64());
    assert!(
        min_width.is_some_and(|w| w <= 480),
        "minWidth {min_width:?}"
    );
    assert!(
        min_height.is_some_and(|h| h <= 360),
        "minHeight {min_height:?}"
    );
}

/// The macOS toolbar row in the page matches the window: the band is as high as the row the
/// traffic lights are centred in, and the rail is as wide as the lights.
#[test]
fn the_macos_toolbar_row_matches_the_traffic_lights() {
    let tokens = std::fs::read_to_string(repo("ui/src/styles/tokens.css")).expect("tokens.css");
    let px = |name: &str| -> u64 {
        let line = tokens
            .lines()
            .find(|l| l.trim_start().starts_with(&format!("{name}:")))
            .unwrap_or_else(|| panic!("{name} missing"));
        line.split(':')
            .nth(1)
            .expect("value")
            .trim()
            .trim_end_matches(';')
            .trim_end_matches("px")
            .parse()
            .unwrap_or_else(|e| panic!("{name}: {e}"))
    };
    let row = px("--mac-toolbar");
    let lights =
        config("tauri.macos.conf.json")["app"]["windows"][0]["trafficLightPosition"].clone();
    let (x, y) = (
        lights["x"].as_u64().expect("x"),
        lights["y"].as_u64().expect("y"),
    );
    // Measured on the macOS CI runner (smoke line `SMOKE {"lights":...}`): the buttons are
    // 14 pt high and their centre sits 2 pt above `y` (y 18 gave centre 16), so y - 2 is the
    // centre that must meet the middle of the row.
    assert_eq!(y - 2, row / 2, "traffic lights centred in the {row} px row");
    // Three buttons of 14 pt, 6 pt apart, and room to the right.
    assert!(
        x + 3 * 14 + 2 * 6 < px("--traffic-lights-width"),
        "the rail holds the lights"
    );
    let base = std::fs::read_to_string(repo("ui/src/styles/base.css")).expect("base.css");
    for rule in [
        "--window-top: var(--mac-toolbar);",
        "--list-toolbar: var(--mac-toolbar);",
        "--rail-width: var(--traffic-lights-width);",
    ] {
        assert!(base.contains(rule), "base.css (macOS): {rule}");
    }
    // The app places the lights itself (tao applies the inset only while its covered content
    // view draws) and reads the position from this configuration: one source, no second
    // number in the code.
    let platform = std::fs::read_to_string(repo("src-tauri/src/platform.rs")).expect("platform.rs");
    assert!(
        platform.contains("traffic_light_position") && platform.contains("pub mod lights"),
        "platform.rs places the traffic lights from trafficLightPosition"
    );
    for literal in [format!("{x}.0"), format!("{y}.0")] {
        assert!(
            !platform.contains(&literal),
            "platform.rs repeats the position ({literal}); read it from the configuration"
        );
    }
}

/// `--p-*` HSL triplet of tokens.css as 8-bit RGB (rounded like a browser).
fn palette_rgb(tokens: &str, name: &str) -> [u8; 3] {
    let line = tokens
        .lines()
        .find(|l| l.trim_start().starts_with(&format!("--p-{name}:")))
        .unwrap_or_else(|| panic!("--p-{name} missing in tokens.css"));
    let value = line
        .split(':')
        .nth(1)
        .expect("value")
        .trim()
        .trim_end_matches(';');
    let parts: Vec<f64> = value
        .split_whitespace()
        .map(|p| p.trim_end_matches('%').parse().expect("hsl number"))
        .collect();
    let (hue, saturation, lightness) = (parts[0], parts[1] / 100.0, parts[2] / 100.0);
    let chroma = (1.0 - (2.0 * lightness - 1.0).abs()) * saturation;
    let second = chroma * (1.0 - ((hue / 60.0) % 2.0 - 1.0).abs());
    let base = lightness - chroma / 2.0;
    let (red, green, blue) = match hue {
        h if h < 60.0 => (chroma, second, 0.0),
        h if h < 120.0 => (second, chroma, 0.0),
        h if h < 180.0 => (0.0, chroma, second),
        h if h < 240.0 => (0.0, second, chroma),
        h if h < 300.0 => (second, 0.0, chroma),
        _ => (chroma, 0.0, second),
    };
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "0..=255 by construction"
    )]
    let byte = |channel: f64| ((channel + base) * 255.0).round() as u8;
    [byte(red), byte(green), byte(blue)]
}

/// A `pub const NAME: Rgb = [0x.., 0x.., 0x..];` of src-tauri/src/platform.rs.
fn platform_rgb(source: &str, name: &str) -> [u8; 3] {
    let line = source
        .lines()
        .find(|l| {
            l.trim_start()
                .starts_with(&format!("pub const {name}: Rgb"))
        })
        .unwrap_or_else(|| panic!("{name} missing in platform.rs"));
    let list = line
        .split('[')
        .nth(1)
        .expect("array")
        .split(']')
        .next()
        .expect("end");
    let bytes: Vec<u8> = list
        .split(',')
        .map(|b| u8::from_str_radix(b.trim().trim_start_matches("0x"), 16).expect("hex byte"))
        .collect();
    [bytes[0], bytes[1], bytes[2]]
}

/// The native Windows title bar wears the app's colours (platform.rs, DWM): its caption is
/// the cream of the sidebar below it, which is also the window's `backgroundColor` (no flash
/// before the first paint), its title the ink of the text, dimmed to the subtle text.
#[test]
fn the_title_bar_colours_are_the_tokens() {
    let tokens = std::fs::read_to_string(repo("ui/src/styles/tokens.css")).expect("tokens.css");
    let platform = std::fs::read_to_string(repo("src-tauri/src/platform.rs")).expect("platform.rs");
    let cream = palette_rgb(&tokens, "cream");
    assert!(
        tokens.contains("--bg: hsl(var(--p-cream));"),
        "--bg is the cream"
    );
    assert_eq!(
        platform_rgb(&platform, "TITLE_BAR_BACKGROUND"),
        cream,
        "caption = --bg"
    );
    assert_eq!(
        platform_rgb(&platform, "TITLE_BAR_TEXT"),
        palette_rgb(&tokens, "ink"),
        "title = --text"
    );
    assert_eq!(
        platform_rgb(&platform, "TITLE_BAR_TEXT_INACTIVE"),
        palette_rgb(&tokens, "fg-subtle"),
        "inactive title = --text-subtle"
    );
    let shared = config("tauri.conf.json");
    let background = shared["app"]["windows"][0]["backgroundColor"]
        .as_str()
        .expect("backgroundColor");
    assert_eq!(
        background.to_ascii_uppercase(),
        format!("#{:02X}{:02X}{:02X}", cream[0], cream[1], cream[2]),
        "backgroundColor = --bg"
    );
}

/// Motion stays snappy and calm (user test of the installed app): colour changes, entries,
/// view switches and dialogs within 180 ms, the fill of a ring within 400 ms, and no easing
/// that overshoots (no bounce). Nothing blurs a large area or animates layout.
#[test]
fn motion_stays_quick_and_calm() {
    let all = scanned(MIN_FILES);
    let tokens = all
        .iter()
        .find(|s| s.is("styles/tokens.css"))
        .expect("styles/tokens.css");
    let ms = |name: &str| -> u32 {
        let line = tokens
            .code
            .lines()
            .find(|l| l.trim_start().starts_with(&format!("{name}:")))
            .unwrap_or_else(|| panic!("{name} missing"));
        line.split(':')
            .nth(1)
            .expect("value")
            .trim()
            .trim_end_matches(';')
            .trim_end_matches("ms")
            .parse()
            .expect("milliseconds")
    };
    for name in [
        "--dur-instant",
        "--dur-hover",
        "--dur-fast",
        "--dur-base",
        "--dur-slow",
    ] {
        assert!(ms(name) <= 180, "{name} is {} ms (at most 180)", ms(name));
    }
    assert!(ms("--dur-reveal") <= 400, "--dur-reveal above 400 ms");
    let mut problems = Vec::new();
    for (n, line) in tokens.lines() {
        let Some(at) = line.find("cubic-bezier(") else {
            continue;
        };
        let inner = &line[at + "cubic-bezier(".len()..];
        let numbers: Vec<f64> = inner
            .split(')')
            .next()
            .unwrap_or_default()
            .split(',')
            .filter_map(|v| v.trim().parse().ok())
            .collect();
        if numbers.len() == 4
            && [numbers[1], numbers[3]]
                .iter()
                .any(|y| !(0.0..=1.0).contains(y))
        {
            problems.push(format!("tokens.css:{n}: easing overshoots"));
        }
    }
    problems.extend(find(
        &all,
        &["backdrop-filter", "filter: blur", "grid-template-rows var("],
        |_| false,
    ));
    fail(&problems, "quick, calm and cheap motion");
}

/// The score ring's ten colour steps are one table: `--p-score-0` ... `--p-score-9` in
/// tokens.css say exactly what `export::scale::SCORE_SCALE` says (HTML overview, Excel).
#[test]
fn the_score_scale_is_one_table() {
    use jobalert_core::export::scale::SCORE_SCALE;
    let tokens = std::fs::read_to_string(repo("ui/src/styles/tokens.css")).expect("tokens.css");
    for (n, colour) in SCORE_SCALE.iter().enumerate() {
        let name = format!("--p-score-{n}:");
        let line = tokens
            .lines()
            .find(|l| l.trim_start().starts_with(&name))
            .unwrap_or_else(|| panic!("{name} missing in tokens.css"));
        let value = line
            .split(':')
            .nth(1)
            .expect("value")
            .trim()
            .trim_end_matches(';');
        let expected = format!(
            "{} {}% {}%",
            colour.hue, colour.saturation, colour.lightness
        );
        assert_eq!(value, expected, "{name} differs from export::scale");
        assert_eq!(
            palette_rgb(&tokens, &format!("score-{n}")),
            {
                let [_, red, green, blue] = colour.rgb().to_be_bytes();
                [red, green, blue]
            },
            "{name} rgb"
        );
    }
}

/// Two brand colours with two jobs (user decisions): coral acts, navy orients - and they
/// never blend. No second hue in a gradient (no coral-to-navy), no gradient text, no
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
                "--p-navy",
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

/// The UI catalogs: German, the source, and English, the same keys (a type error otherwise).
const CATALOGS: [&str; 2] = ["lib/i18n/de.ts", "lib/i18n/en.ts"];

fn catalog<'a>(all: &'a [Source], path: &str) -> &'a Source {
    all.iter()
        .find(|s| s.is(path))
        .unwrap_or_else(|| panic!("{path} missing"))
}

/// One word per thing (glossary in docs/PLAN.md and the header of en.ts); short texts, no
/// walls of text.
#[test]
fn the_catalog_keeps_the_glossary() {
    let all = scanned(MIN_FILES);
    let mut problems = Vec::new();
    for (n, line) in catalog(&all, "lib/i18n/de.ts").lines() {
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
    }
    for (n, line) in catalog(&all, "lib/i18n/en.ts").lines() {
        for (old, new) in [
            ("Entry", "Job"),
            ("Entries", "Jobs"),
            ("Candidate", "Job"),
            ("Source", "Portal"),
            ("Full text", "Details"),
            ("Hit", "Match"),
            ("Hits", "Matches"),
            ("Inbox", "Mailbox"),
            ("Pinned", "Saved"),
            ("Bookmark", "Saved"),
        ] {
            // Whole words in any case ("Hit" is no part of "white").
            let used = literals(line).iter().any(|text| {
                let plain: String = words(text)
                    .to_lowercase()
                    .chars()
                    .map(|c| if c.is_alphanumeric() { c } else { ' ' })
                    .collect();
                format!(" {plain} ").contains(&format!(" {} ", old.to_lowercase()))
            });
            if used {
                problems.push(format!("en.ts:{n}: \"{old}\" is called \"{new}\""));
            }
        }
    }
    for path in CATALOGS {
        let name = path.rsplit('/').next().unwrap_or(path);
        for (n, line) in catalog(&all, path).lines() {
            for quoted in line.split('\'').skip(1).step_by(2) {
                if quoted.chars().count() > 140 {
                    problems.push(format!(
                        "{name}:{n}: {} characters (max 140)",
                        quoted.chars().count()
                    ));
                }
            }
        }
    }
    fail(&problems, "glossary and length of the UI catalogs");
}

/// The English catalog is English: no umlaut or sharp s and no German word in anything the
/// user reads. Product and portal names (Job-Alert-Monitor, freelance.de) and the name of
/// the German language (Deutsch) are no German words.
#[test]
fn the_english_catalog_has_no_german() {
    let all = scanned(MIN_FILES);
    let german = [
        "und", "der", "die", "das", "den", "dem", "nicht", "ist", "sind", "mit", "von", "für",
        "oder", "auf", "bei", "aus", "neu", "alle", "ein", "eine", "wird", "werden", "bitte",
        "noch", "kein", "keine", "zu", "im", "ohne", "abrufen", "passung", "postfach",
        "gemerkt", "merken", "profil", "einstellungen",
    ];
    let mut problems = Vec::new();
    let mut strings = 0;
    for (n, line) in catalog(&all, "lib/i18n/en.ts").lines() {
        for literal in literals(line) {
            let text = words(literal);
            strings += 1;
            if text.chars().any(|c| "äöüÄÖÜß„".contains(c)) {
                problems.push(format!("en.ts:{n}: German letters in \"{text}\""));
            }
            let lower = text.to_lowercase();
            for word in lower.split(|c: char| !c.is_alphanumeric()) {
                if german.contains(&word) {
                    problems.push(format!("en.ts:{n}: German \"{word}\" in \"{text}\""));
                }
            }
        }
    }
    assert!(strings >= 200, "only {strings} strings in en.ts");
    fail(&problems, "no German in the English catalog");
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

/// Both catalogs speak plainly (CLAUDE.md): no dash or em dash as a separator, no colon at
/// the end of a label or heading, no "X: Y" construction, no exclamation mark.
#[test]
fn the_catalog_has_no_ai_punctuation() {
    let all = scanned(MIN_FILES);
    let mut problems = Vec::new();
    for path in CATALOGS {
        let name = path.rsplit('/').next().unwrap_or(path);
        let mut strings = 0;
        for (n, line) in catalog(&all, path).lines() {
            for literal in literals(line) {
                let text = words(literal);
                let text = text.as_str();
                strings += 1;
                for dash in [" - ", " – ", "–", "—"] {
                    if text.contains(dash) {
                        problems.push(format!("{name}:{n}: dash as a separator in \"{text}\""));
                    }
                }
                if text.trim_end().ends_with(':') {
                    problems.push(format!("{name}:{n}: colon at the end of \"{text}\""));
                }
                if text.contains(": ") {
                    problems.push(format!("{name}:{n}: \"X: Y\" in \"{text}\""));
                }
                let mut chars = text.chars().peekable();
                while let Some(c) = chars.next() {
                    if c == '!' && chars.peek() != Some(&'=') {
                        problems.push(format!("{name}:{n}: exclamation mark in \"{text}\""));
                    }
                }
            }
        }
        assert!(strings >= 200, "only {strings} strings in {name}");
    }
    fail(&problems, "plain punctuation in the UI catalogs");
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
