//! English-only sweep (see `CLAUDE.md`): every git-tracked text file must be English in
//! its comments and doc text. Exceptions are data and user-facing German text by product
//! decision (see `ALLOWLIST_PATHS`).
//!
//! The check only looks at comment lines (or, for Markdown, the whole line - Markdown has
//! no other kind of "code"), not at string literals: data such as mail/page parsing
//! patterns, profile JSON keys or test fixtures is allowed to be German by contract and is
//! never scanned here, matching `CLAUDE.md`'s "everything in the repo is English (code,
//! comments, docs, logs, errors, tests, CI, commits)" rule with its documented exceptions.

use std::path::Path;
use std::process::Command;

/// Whole files or directories that stay German on purpose and are not scanned: data
/// fixtures and snapshots, generated third-party licence text, and product-decision
/// German prose (the user-facing README, the UI vocabulary glossary in `docs/`, the UI
/// catalog itself). None of these are files this track is allowed to translate.
const ALLOWLIST_PATHS: &[&str] = &[
    // User-facing German text / product decision, not a code comment.
    "README.md",
    "docs/PLAN.md",
    "docs/MATCHING.md",
    "ui/src/lib/i18n/de.ts",
    // Real or invented German mail/page/profile data, and recorded output.
    "core/tests/fixtures/",
    "core/tests/snapshots/",
    // Generated third-party licence text: author names and quoted licence bodies produce
    // only false positives (accented names, curly quotes), never real German prose.
    "src-tauri/resources/THIRD-PARTY.txt",
];

/// Individual `path:line` false positives: a comment that is already English but quotes a
/// real German example (a place name, a mail field label, a job title) to explain what the
/// code matches. The line number is 1-based, exactly as reported by a test failure below.
const LINE_ALLOWLIST: &[(&str, u32)] = &[
    ("core/src/text/company_location.rs", 6),
    ("core/src/text/company_location.rs", 49),
    ("core/src/text/company_location.rs", 53),
    ("core/src/mail/tests.rs", 214),
    ("core/src/portal/freelance_de.rs", 419),
];

/// True if `path` (repo-relative, `/`-separated) must not be scanned.
fn is_excluded(path: &str) -> bool {
    ALLOWLIST_PATHS
        .iter()
        .any(|p| path == *p || path.starts_with(p))
}

/// How to pull the "comment text" (if any) out of one line of `path`, by file type. Only
/// line-start comments are recognised - the repo's own style never relies on trailing
/// end-of-line comments for prose, so this keeps the heuristic simple and false-positive
/// free (a `//` inside a string, e.g. a `https://` URL, is never at the start of a line).
fn comment_text<'a>(path: &str, line: &'a str) -> Option<&'a str> {
    let trimmed = line.trim_start();
    let ext = Path::new(path).extension().and_then(|e| e.to_str());
    let name = Path::new(path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("");
    match ext {
        Some("rs" | "ts" | "tsx" | "js" | "mjs" | "cjs" | "svelte" | "css") => trimmed
            .strip_prefix("//")
            .or_else(|| trimmed.strip_prefix("/*"))
            .or_else(|| {
                trimmed
                    .strip_prefix('*')
                    .filter(|_| !trimmed.starts_with("*/"))
            }),
        Some("py" | "toml" | "yml" | "yaml") => trimmed.strip_prefix('#'),
        Some("md") => Some(trimmed).filter(|l| !l.starts_with("```")),
        None if matches!(
            name,
            ".gitignore" | ".gitattributes" | ".prettierignore" | "pre-push"
        ) =>
        {
            trimmed.strip_prefix('#')
        }
        _ => None,
    }
}

/// Common German function words, padded with spaces so they only match whole words (not
/// substrings of English words like "consist" or "forward"), plus the German-only letters
/// (umlauts and the sharp s) that never occur in English prose.
const GERMAN_WORDS: &[&str] = &[
    " und ", " der ", " die ", " das ", " nicht ", " wird ", " ist ",
];

fn looks_german(text: &str) -> bool {
    // Quoted examples (`code`, „...", "...") are data, not prose: drop them first.
    let mut cleaned = String::new();
    let mut quote: Option<char> = None;
    for c in text.chars() {
        match (quote, c) {
            (None, '`' | '„' | '"' | '“') => quote = Some(if c == '„' { '“' } else { c }),
            (Some(q), _) if c == q || (q == '“' && c == '"') => quote = None,
            (Some(_), _) => {}
            (None, _) => cleaned.push(c),
        }
    }
    let words = whole_words(&cleaned);
    if GERMAN_WORDS.iter().any(|w| words.contains(w)) {
        return true;
    }
    // A German term with umlauts inside an English sentence ("ANÜ named and not negated")
    // is a quoted name; German prose has no English function words around it.
    cleaned.chars().any(|c| "äöüÄÖÜß".contains(c))
        && !ENGLISH_WORDS.iter().any(|w| words.contains(w))
}

/// English function words that mark a comment as English prose.
const ENGLISH_WORDS: &[&str] = &[
    " the ",
    " a ",
    " an ",
    " of ",
    " and ",
    " or ",
    " is ",
    " to ",
    " in ",
    " with ",
    " for ",
    " not ",
    " only ",
    " as ",
    " by ",
    " from ",
    " then ",
    " without ",
    " value ",
    " meaning ",
    " at ",
    " on ",
];

/// Lower-cased words separated and padded by single spaces (punctuation becomes a space).
fn whole_words(text: &str) -> String {
    let normalised: String = text
        .to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { ' ' })
        .collect();
    format!(
        " {} ",
        normalised.split_whitespace().collect::<Vec<_>>().join(" ")
    )
}

fn tracked_files() -> Vec<String> {
    let repo_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let output = Command::new("git")
        .args(["ls-files"])
        .current_dir(&repo_root)
        .output()
        .expect("git ls-files");
    assert!(output.status.success(), "git ls-files failed");
    String::from_utf8(output.stdout)
        .expect("utf8")
        .lines()
        .map(|l| l.replace('\\', "/"))
        .collect()
}

#[test]
fn no_german_comments_outside_the_allowlist() {
    let repo_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let mut scanned = 0usize;
    let mut offenses: Vec<String> = Vec::new();

    for path in tracked_files() {
        if is_excluded(&path) {
            continue;
        }
        let Ok(content) = std::fs::read_to_string(repo_root.join(&path)) else {
            continue; // binary file (png, ico, woff2, ...): nothing to scan as text
        };
        scanned += 1;
        for (number, line) in content.lines().enumerate() {
            let Some(comment) = comment_text(&path, line) else {
                continue;
            };
            let line_number = u32::try_from(number + 1).unwrap_or(u32::MAX);
            if LINE_ALLOWLIST.contains(&(path.as_str(), line_number)) {
                continue;
            }
            if looks_german(comment) {
                offenses.push(format!("{path}:{line_number}: {}", line.trim()));
            }
        }
    }

    assert!(
        scanned >= 80,
        "expected at least 80 files scanned, got {scanned} - did the file list break?"
    );
    println!("language.rs: {scanned} files scanned");
    assert!(
        offenses.is_empty(),
        "German comment lines found ({} files scanned):\n{}",
        scanned,
        offenses.join("\n")
    );
}

#[test]
fn detector_catches_german_and_leaves_english_alone() {
    assert!(looks_german("Dies ist ein Kommentar und kein Test"));
    assert!(looks_german("Ist das ein Fehler?")); // capitalised, word at line start
    assert!(looks_german("Ort: München")); // umlaut alone is enough
    assert!(looks_german("Text, der nicht passt.")); // word next to punctuation
    assert!(!looks_german("This is an English comment and a real test"));
    assert!(!looks_german("Consistent assistants list artists")); // "ist" as a substring
    assert!(!looks_german("Forward the request, keep it moving"));
    assert!(!looks_german(""));
    assert!(!looks_german(
        "ANÜ named and not negated anywhere in the text."
    ));
    assert!(!looks_german("Case-folded (`Übersicht` -> `ubersicht`)."));
    assert!(looks_german("Grünwerk Übersicht Köln"));

    assert_eq!(comment_text("core/src/lib.rs", "// hello"), Some(" hello"));
    assert_eq!(
        comment_text("core/src/lib.rs", "    /// doc"),
        Some("/ doc")
    );
    assert_eq!(
        comment_text("core/src/lib.rs", "let url = \"https://x\";"),
        None
    );
    assert_eq!(comment_text("tools/icon.py", "# note"), Some(" note"));
    assert_eq!(
        comment_text("core/tests/fixtures/mails/x.eml", "# note"),
        None
    );
}
