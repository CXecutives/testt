//! The app icon files must match their generator (`tools/icon.py`): the ICO stages in the same
//! order, the ICNS entries, the 1024 PNG and the SVG brand mark. Tauri takes the first ICO entry
//! as the window icon (title bar, Alt+Tab, taskbar); a small stage there would be scaled up and
//! look blurry next to the sharp Explorer icon.

use std::path::Path;

/// One stage in the ICO directory.
struct Entry {
    width: u32,
    height: u32,
    planes: u16,
    bpp: u16,
    png: bool,
    len: usize,
    /// Set bits of the AND mask (DIB stages only).
    mask_bits: u32,
}

/// Reads the directory of an ICO file (header 6 bytes, then 16 bytes per stage).
fn read_ico(bytes: &[u8]) -> Vec<Entry> {
    assert_eq!(&bytes[0..4], [0, 0, 1, 0], "not an ICO file");
    let count = usize::from(u16::from_le_bytes([bytes[4], bytes[5]]));
    (0..count)
        .map(|i| {
            let e = &bytes[6 + 16 * i..6 + 16 * (i + 1)];
            // 0 in the width/height byte means 256.
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

/// The list `NAME = [...]` of the generator (numbers or quoted pairs).
fn generator_list<'a>(source: &'a str, name: &str) -> &'a str {
    let head = format!("{name} = [");
    let start = source.find(&head).expect("list in tools/icon.py") + head.len();
    let end = start + source[start..].find(']').expect("closing bracket");
    &source[start..end]
}

/// Width and height of a PNG (IHDR).
fn png_size(png: &[u8]) -> (u32, u32) {
    assert!(png.starts_with(&[0x89, b'P', b'N', b'G']), "not a PNG");
    let be = |o: usize| u32::from_be_bytes([png[o], png[o + 1], png[o + 2], png[o + 3]]);
    (be(16), be(20))
}

#[test]
fn icon_stages_match_generator() {
    let bytes = std::fs::read(repo("src-tauri/icons/icon.ico")).unwrap();
    let entries = read_ico(&bytes);
    let source = std::fs::read_to_string(repo("tools/icon.py")).unwrap();
    let sizes: Vec<u32> = generator_list(&source, "SIZES")
        .split(',')
        .map(|s| s.trim().parse().expect("number in SIZES"))
        .collect();

    let actual: Vec<u32> = entries.iter().map(|e| e.width).collect();
    assert_eq!(
        actual, sizes,
        "icon.ico does not match tools/icon.py any more - regenerate: python tools/icon.py"
    );
    for e in &entries {
        assert_eq!(e.width, e.height, "stage {} is not square", e.width);
        assert_eq!(e.bpp, 32, "stage {} without alpha", e.width);
        assert_eq!(e.planes, 1, "stage {}: planes must be 1", e.width);
        // Only 256 is compressed: older image libraries (GDI+ among them) cannot read a
        // compressed smaller stage and then show nothing.
        assert_eq!(
            e.png,
            e.width == 256,
            "stage {}: PNG only for 256, else an uncompressed DIB",
            e.width
        );
        if !e.png {
            // BITMAPINFOHEADER + BGRA pixels + AND mask; a short stage stays empty.
            let mask = e.width.div_ceil(32) * 4 * e.height;
            let expected = 40 + e.width * e.height * 4 + mask;
            assert_eq!(
                e.len, expected as usize,
                "stage {}: DIB is not complete",
                e.width
            );
            // The mask belongs to the image: whoever reads only the mask would otherwise get
            // an opaque border around the rounded plate.
            assert!(
                e.mask_bits > 0,
                "stage {}: empty AND mask, the transparent corners are missing",
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
        "Tauri takes the first entry as the window icon - a smaller stage would be scaled up"
    );
}

/// The ICNS holds the generator's entries in its order, each a PNG of its size; icon.png is
/// the 1024 entry.
#[test]
fn mac_icons_match_generator() {
    let source = std::fs::read_to_string(repo("tools/icon.py")).unwrap();
    let expected: Vec<(String, u32)> = generator_list(&source, "ICNS_ENTRIES")
        .split(')')
        .filter_map(|pair| {
            let pair = pair.trim_start_matches(|c: char| c == ',' || c.is_whitespace());
            let (ostype, size) = pair.split_once(',')?;
            let ostype = ostype.trim_matches(|c: char| !c.is_ascii_alphanumeric());
            let size = size.trim().parse().ok()?;
            Some((ostype.to_owned(), size))
        })
        .collect();
    assert_eq!(expected.len(), 8, "{expected:?}");
    let icns = std::fs::read(repo("src-tauri/icons/icon.icns")).unwrap();
    assert_eq!(&icns[0..4], b"icns");
    let total = u32::from_be_bytes([icns[4], icns[5], icns[6], icns[7]]) as usize;
    assert_eq!(total, icns.len(), "ICNS length field");
    let mut at = 8;
    let mut found = Vec::new();
    let mut largest = &icns[0..0];
    while at < icns.len() {
        let ostype = String::from_utf8_lossy(&icns[at..at + 4]).into_owned();
        let len =
            u32::from_be_bytes([icns[at + 4], icns[at + 5], icns[at + 6], icns[at + 7]]) as usize;
        let png = &icns[at + 8..at + len];
        let (w, h) = png_size(png);
        assert_eq!(w, h, "{ostype} is not square");
        if w == 1024 {
            largest = png;
        }
        found.push((ostype, w));
        at += len;
    }
    assert_eq!(found, expected, "icon.icns does not match tools/icon.py");
    let png = std::fs::read(repo("src-tauri/icons/icon.png")).unwrap();
    assert_eq!(png_size(&png), (1024, 1024));
    assert_eq!(png, largest, "icon.png is the 1024 entry of the ICNS");
}

/// The brand mark in the UI is the generator's vector: same plate crop, the check cut out of
/// the folder, the coral gradient.
#[test]
fn ui_brand_mark_is_the_generated_vector() {
    let svg = std::fs::read_to_string(repo("ui/src/assets/app-icon.svg")).unwrap();
    assert!(
        svg.contains("Generated by tools/icon.py"),
        "edited by hand?"
    );
    assert!(svg.contains(r#"viewBox="48 48 928 928""#));
    assert!(
        svg.contains(r#"fill-rule="evenodd""#),
        "the check is a cut-out"
    );
    for colour in ["#EB957D", "#D45D3D"] {
        assert!(svg.contains(colour), "gradient colour {colour}");
    }
    assert_eq!(
        svg.matches("<path").count(),
        2,
        "plate and folder with check"
    );
}
