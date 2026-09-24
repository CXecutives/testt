//! Das App-Symbol (`src-tauri/icons/icon.ico`) muss zu seinem Erzeuger (`tools/icon.py`) passen:
//! dieselben Stufen in derselben Reihenfolge. Tauri nimmt für das Fenstersymbol (Titelleiste,
//! Alt+Tab, Taskleiste) stur den ersten Eintrag der ICO. Steht dort eine kleine Stufe, bläst
//! Windows sie auf und das Symbol wirkt neben dem scharfen Explorer-Symbol matschig.

use std::path::Path;

/// Eine Stufe im ICO-Verzeichnis.
struct Entry {
    width: u32,
    height: u32,
    planes: u16,
    bpp: u16,
    png: bool,
    len: usize,
    /// Gesetzte Bits der UND-Maske (nur bei DIB-Stufen).
    mask_bits: u32,
}

/// Liest das Verzeichnis einer ICO-Datei (Kopf 6 Byte, danach je Stufe 16 Byte).
fn read_ico(bytes: &[u8]) -> Vec<Entry> {
    assert_eq!(&bytes[0..4], [0, 0, 1, 0], "keine ICO-Datei");
    let count = usize::from(u16::from_le_bytes([bytes[4], bytes[5]]));
    (0..count)
        .map(|i| {
            let e = &bytes[6 + 16 * i..6 + 16 * (i + 1)];
            // 0 im Breiten-/Höhenbyte heißt 256.
            let size = |b: u8| if b == 0 { 256 } else { u32::from(b) };
            let at = |o: usize| u32::from_le_bytes([e[o], e[o + 1], e[o + 2], e[o + 3]]) as usize;
            let (len, offset) = (at(8), at(12));
            let blob = &bytes[offset..offset + len];
            Entry {
                width: size(e[0]),
                height: size(e[1]),
                planes: u16::from_le_bytes([e[4], e[5]]),
                bpp: u16::from_le_bytes([e[6], e[7]]),
                png: blob.starts_with(&[0x89, b'P', b'N', b'G']),
                len,
                mask_bits: {
                    let w = size(e[0]);
                    let start = 40 + (w * w * 4) as usize;
                    blob.get(start..)
                        .map_or(0, |m| m.iter().map(|b| b.count_ones()).sum())
                },
            }
        })
        .collect()
}

fn repo(relative: &str) -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join(relative)
}

/// Die Stufenliste aus `SIZES = [...]` in `tools/icon.py`.
fn generator_sizes(source: &str) -> Vec<u32> {
    let start = source.find("SIZES = [").expect("SIZES in tools/icon.py") + "SIZES = [".len();
    let end = start + source[start..].find(']').expect("Klammer hinter SIZES");
    source[start..end]
        .split(',')
        .map(|s| s.trim().parse().expect("Zahl in SIZES"))
        .collect()
}

#[test]
fn icon_stages_match_generator() {
    let bytes = std::fs::read(repo("src-tauri/icons/icon.ico")).unwrap();
    let entries = read_ico(&bytes);
    let sizes = generator_sizes(&std::fs::read_to_string(repo("tools/icon.py")).unwrap());

    let actual: Vec<u32> = entries.iter().map(|e| e.width).collect();
    assert_eq!(
        actual, sizes,
        "icon.ico passt nicht mehr zu tools/icon.py – neu erzeugen: python tools/icon.py"
    );
    for e in &entries {
        assert_eq!(e.width, e.height, "Stufe {} ist nicht quadratisch", e.width);
        assert_eq!(e.bpp, 32, "Stufe {} ohne Alphakanal", e.width);
        assert_eq!(e.planes, 1, "Stufe {}: planes muss 1 sein", e.width);
        // Nur die 256er-Stufe ist komprimiert. Ältere Bildbibliotheken (u. a. GDI+) lesen
        // eine komprimierte Stufe darunter nicht und zeigen dann gar nichts an.
        assert_eq!(
            e.png,
            e.width == 256,
            "Stufe {}: PNG nur für 256, sonst unkomprimiertes DIB",
            e.width
        );
        if !e.png {
            // BITMAPINFOHEADER + BGRA-Bilddaten + UND-Maske; eine zu kurze Stufe bleibt leer.
            let mask = e.width.div_ceil(32) * 4 * e.height;
            let expected = 40 + e.width * e.height * 4 + mask;
            assert_eq!(
                e.len, expected as usize,
                "Stufe {}: DIB hat nicht die volle Länge",
                e.width
            );
            // Die Maske gehört zum Bild: Wer nur sie liest, bekäme sonst einen
            // undurchsichtigen Rand um die abgerundete Platte.
            assert!(
                e.mask_bits > 0,
                "Stufe {}: UND-Maske ist leer, die durchsichtigen Ecken fehlen",
                e.width
            );
        }
    }
}

#[test]
fn window_icon_is_the_48_stage() {
    let bytes = std::fs::read(repo("src-tauri/icons/icon.ico")).unwrap();
    let first = &read_ico(&bytes)[0];
    assert_eq!(
        first.width, 48,
        "Tauri nimmt den ersten Eintrag als Fenstersymbol – eine kleinere Stufe würde hochgerechnet"
    );
}

/// `(x, y)` pairs of `CHECK_POINTS = [...]` in `tools/icon.py` (plate units).
fn generator_check(source: &str) -> Vec<(f64, f64)> {
    let start = source
        .find("CHECK_POINTS = [")
        .expect("CHECK_POINTS in tools/icon.py");
    let line = source[start..].lines().next().expect("CHECK_POINTS line");
    line.split('(')
        .skip(1)
        .map(|pair| {
            let pair = pair.split(')').next().expect("closing parenthesis");
            let (x, y) = pair.split_once(',').expect("x, y");
            (x.trim().parse().expect("x"), y.trim().parse().expect("y"))
        })
        .collect()
}

/// The 1024 PNG (macOS window icon, source for other tooling) has the full size.
#[test]
fn the_png_is_1024() {
    let bytes = std::fs::read(repo("src-tauri/icons/icon.png")).unwrap();
    assert!(
        bytes.starts_with(&[0x89, b'P', b'N', b'G']),
        "icon.png is not a PNG"
    );
    let dim = |o: usize| u32::from_be_bytes([bytes[o], bytes[o + 1], bytes[o + 2], bytes[o + 3]]);
    assert_eq!(
        (dim(16), dim(20)),
        (1024, 1024),
        "icon.png must be 1024 x 1024"
    );
}

/// The ICNS holds exactly the entries the generator writes, all PNG.
#[test]
fn the_icns_matches_the_generator() {
    let bytes = std::fs::read(repo("src-tauri/icons/icon.icns")).unwrap();
    assert_eq!(&bytes[0..4], b"icns", "icon.icns is not an ICNS file");
    let be = |o: usize| u32::from_be_bytes([bytes[o], bytes[o + 1], bytes[o + 2], bytes[o + 3]]);
    assert_eq!(be(4) as usize, bytes.len(), "ICNS length header is wrong");
    let source = std::fs::read_to_string(repo("tools/icon.py")).unwrap();
    let expected = source
        .split("ICNS_ENTRIES = [")
        .nth(1)
        .and_then(|rest| rest.split(']').next())
        .expect("ICNS_ENTRIES in tools/icon.py")
        .matches("('")
        .count();
    let mut offset = 8;
    let mut entries = 0;
    while offset + 8 <= bytes.len() {
        assert!(
            bytes[offset + 8..].starts_with(&[0x89, b'P', b'N', b'G']),
            "ICNS entry {entries} is not a PNG"
        );
        offset += be(offset + 4) as usize;
        entries += 1;
    }
    assert_eq!(
        entries, expected,
        "icon.icns does not match tools/icon.py - run it again"
    );
}

/// The UI's brand mark is the same drawing as the app icon: generated, not hand-made.
#[test]
fn the_ui_mark_is_generated_from_the_same_geometry() {
    let svg = std::fs::read_to_string(repo("ui/src/assets/app-icon.svg")).unwrap();
    assert!(
        svg.contains("Generated by tools/icon.py"),
        "ui/src/assets/app-icon.svg must come from tools/icon.py"
    );
    let source = std::fs::read_to_string(repo("tools/icon.py")).unwrap();
    // macOS geometry: an 824 px plate at a 100 px margin in the 1024 grid.
    let fmt = |v: f64| {
        let s = format!("{v:.1}");
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    };
    let points: Vec<String> = generator_check(&source)
        .iter()
        .map(|(x, y)| format!("{},{}", fmt(100.0 + x * 824.0), fmt(100.0 + y * 824.0)))
        .collect();
    assert_eq!(points.len(), 3, "the check has three points");
    assert!(
        svg.contains(&format!("points=\"{}\"", points.join(" "))),
        "the check in app-icon.svg differs from tools/icon.py - run it again"
    );
}
