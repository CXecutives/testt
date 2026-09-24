//! The German texts on the Rust side follow the rules of the interface's catalog (CLAUDE.md,
//! glossary in docs/PLAN.md): the export texts (Excel, HTML overview), the startup dialog,
//! the sign-in window title, the file dialogs and the macOS menu. Each block starts at a line
//! naming "User-facing text, German by product decision" and ends at "end of user-facing
//! text"; every string literal in it is checked.
//!
//! Every test asserts how many texts it found: a moved block must not turn a rule into a
//! silent no-op.

use std::path::Path;

/// The files with German text blocks, and how many strings each has at least.
const FILES: [(&str, usize); 5] = [
    ("core/src/export/texts.rs", 45),
    ("src-tauri/src/main.rs", 12),
    ("src-tauri/src/session.rs", 1),
    ("src-tauri/src/platform.rs", 15),
    ("src-tauri/src/commands/mod.rs", 3),
];

const START: &str = "User-facing text, German by product decision";
const END: &str = "end of user-facing text";

/// Old words and the glossary word the texts use instead.
const GLOSSARY: [(&str, &str); 9] = [
    ("Quelle", "Portal"),
    ("Eintrag", "Job"),
    ("Volltext", "Details"),
    ("Jobdetails", "Details"),
    ("Kandidat", "Job"),
    ("Treffer", "Passung"),
    ("Mailbox", "Postfach"),
    ("Lauf", "Abruf"),
    ("Kurzfassung", "Anriss"),
];

/// One German string with where it stands.
struct Text {
    at: String,
    text: String,
}

/// The string literals of the text blocks of every file.
fn texts() -> Vec<Text> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let mut out = Vec::new();
    for (file, min) in FILES {
        let source =
            std::fs::read_to_string(root.join(file)).unwrap_or_else(|e| panic!("{file}: {e}"));
        let found = blocks(&source)
            .into_iter()
            .flat_map(|(line, block)| {
                literals(&block)
                    .into_iter()
                    .map(move |(n, text)| (line + n, text))
            })
            .map(|(line, text)| Text {
                at: format!("{file}:{line}"),
                text,
            })
            .collect::<Vec<_>>();
        assert!(
            found.len() >= min,
            "{file}: only {} texts (at least {min})",
            found.len()
        );
        out.extend(found);
    }
    out
}

/// The text blocks of a file: first line number (1-based) and the lines after the start
/// marker up to the end marker (or the end of the file).
fn blocks(source: &str) -> Vec<(usize, String)> {
    let mut out = Vec::new();
    let mut current: Option<(usize, String)> = None;
    for (index, line) in source.lines().enumerate() {
        if let Some((_, block)) = current.as_mut() {
            if line.contains(END) {
                out.extend(current.take());
            } else {
                block.push_str(line);
                block.push('\n');
            }
        } else if line.contains(START) {
            current = Some((index + 2, String::new()));
        }
    }
    out.extend(current);
    out
}

/// The string literals of Rust code with their line offset in it: comments are skipped,
/// escapes and line continuations resolved.
fn literals(code: &str) -> Vec<(usize, String)> {
    let mut out = Vec::new();
    let mut line = 0;
    let mut chars = code.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\n' => line += 1,
            '/' if chars.peek() == Some(&'/') => {
                for c in chars.by_ref() {
                    if c == '\n' {
                        line += 1;
                        break;
                    }
                }
            }
            '"' => {
                let start = line;
                let mut text = String::new();
                while let Some(c) = chars.next() {
                    match c {
                        '"' => break,
                        '\\' => match chars.next() {
                            Some('\n') => {
                                line += 1;
                                while chars.peek().is_some_and(|c| c.is_whitespace()) {
                                    if chars.next() == Some('\n') {
                                        line += 1;
                                    }
                                }
                            }
                            Some('n') => text.push('\n'),
                            Some(other) => text.push(other),
                            None => {}
                        },
                        '\n' => {
                            line += 1;
                            text.push('\n');
                        }
                        other => text.push(other),
                    }
                }
                out.push((start, text));
            }
            _ => {}
        }
    }
    out
}

/// Whether `text` uses `word`, also inflected ("Laufs") or as part of a hyphenated compound
/// ("Postfach-Lauf") - but not inside another word ("Laufzeit").
fn uses_word(text: &str, word: &str) -> bool {
    text.split(|c: char| !c.is_alphanumeric()).any(|w| {
        w.strip_prefix(word)
            .is_some_and(|rest| ["", "s", "es", "e", "en", "n"].contains(&rest))
    })
}

fn fail(problems: &[String], rule: &str) {
    assert!(problems.is_empty(), "{rule}:\n{}", problems.join("\n"));
}

/// Plain punctuation (CLAUDE.md): no dash or em dash as a separator, no colon at the end of
/// a label, no "X: Y" construction, no exclamation mark.
#[test]
fn the_rust_texts_have_no_ai_punctuation() {
    let mut problems = Vec::new();
    for Text { at, text } in texts() {
        for dash in [" - ", " – ", "–", "—"] {
            if text.contains(dash) {
                problems.push(format!("{at}: dash as a separator in \"{text}\""));
            }
        }
        if text.trim_end().ends_with(':') {
            problems.push(format!("{at}: colon at the end of \"{text}\""));
        }
        if text.contains(": ") {
            problems.push(format!("{at}: \"X: Y\" in \"{text}\""));
        }
        if text.contains('!') {
            problems.push(format!("{at}: exclamation mark in \"{text}\""));
        }
    }
    fail(&problems, "plain punctuation in the Rust texts");
}

/// One word per thing, the same as in the interface; short texts.
#[test]
fn the_rust_texts_keep_the_glossary() {
    let mut problems = Vec::new();
    for Text { at, text } in texts() {
        for (old, new) in GLOSSARY {
            if uses_word(&text, old) {
                problems.push(format!("{at}: \"{old}\" is called \"{new}\" in \"{text}\""));
            }
        }
        if text.chars().count() > 140 {
            problems.push(format!(
                "{at}: {} characters (max 140)",
                text.chars().count()
            ));
        }
    }
    fail(&problems, "glossary and length of the Rust texts");
}

/// The scanner itself: continuation lines, escapes and comments.
#[test]
fn literals_are_read_like_rust_reads_them() {
    let code =
        "const A: &str = \"eins \\\n    zwei\"; // \"kein Text\"\nconst B: &str = \"C:\\\\x\";";
    let found: Vec<String> = literals(code).into_iter().map(|(_, text)| text).collect();
    assert_eq!(found, ["eins zwei", "C:\\x"]);
    let file = "x\n// User-facing text, German by product decision.\nconst A: &str = \"a\";\n// end of user-facing text\nconst B: &str = \"b\";";
    let found: Vec<(usize, String)> = blocks(file)
        .into_iter()
        .flat_map(|(line, block)| {
            literals(&block)
                .into_iter()
                .map(move |(n, t)| (line + n, t))
        })
        .collect();
    assert_eq!(found, [(3, "a".to_owned())]);
    assert!(uses_word("Umfang des letzten Laufs", "Lauf"));
    assert!(uses_word("Postfach-Lauf", "Lauf"));
    assert!(!uses_word("Die WebView2-Laufzeit fehlt.", "Lauf"));
}
