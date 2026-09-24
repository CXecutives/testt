//! The app icon files must match their generator (`tools/icon.py`): the ICO stages in the same
//! order, the ICNS entries, the 1024 PNG and the SVG brand mark. Tauri takes the first ICO entry
//! as the window icon (title bar, Alt+Tab, taskbar); a small stage there would be scaled up and
//! look blurry next to the sharp Explorer icon.
//!
//! The ICO stages carry straight alpha and the plate colour in every pixel, the transparent ones
//! included (colour bleed): the Windows shell scales a stage without premultiplying when it has
//! no stage of the wanted size, and a transparent black pixel then becomes a dark fringe around
//! the plate on the desktop.

use std::io::Read;
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
    /// Straight RGBA, top row first.
    pixels: Vec<[u8; 4]>,
    /// The AND mask per pixel, top row first (DIB stages only; true = transparent).
    mask: Option<Vec<bool>>,
}

/// Reads an ICO file (header 6 bytes, then 16 bytes per stage) and decodes every stage.
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
            let png = blob.starts_with(&[0x89, b'P', b'N', b'G']);
            let w = size(e[0]);
            let (pixels, mask) = if png {
                (decode_png(blob), None)
            } else {
                let (pixels, mask) = decode_dib(blob, w as usize);
                (pixels, Some(mask))
            };
            Entry {
                width: w,
                height: size(e[1]),
                planes: u16::from_le_bytes([e[4], e[5]]),
                bpp: u16::from_le_bytes([e[6], e[7]]),
                png,
                len,
                mask_bits: {
                    let start = 40 + (w * w * 4) as usize;
                    blob.get(start..)
                        .map_or(0, |m| m.iter().map(|b| b.count_ones()).sum())
                },
                pixels,
                mask,
            }
        })
        .collect()
}

/// Pixels (RGBA, top row first) and AND mask of a 32 bpp DIB stage: BITMAPINFOHEADER, BGRA rows
/// bottom up, then the mask rows bottom up, each padded to 4 bytes.
fn decode_dib(blob: &[u8], s: usize) -> (Vec<[u8; 4]>, Vec<bool>) {
    let head = u32::from_le_bytes([blob[0], blob[1], blob[2], blob[3]]) as usize;
    let xor = &blob[head..head + s * s * 4];
    let and = &blob[head + s * s * 4..];
    let stride = s.div_ceil(32) * 4;
    let mut pixels = Vec::with_capacity(s * s);
    let mut mask = Vec::with_capacity(s * s);
    for y in 0..s {
        let row = s - 1 - y;
        for x in 0..s {
            let p = &xor[(row * s + x) * 4..][..4];
            pixels.push([p[2], p[1], p[0], p[3]]);
            mask.push(and[row * stride + x / 8] & (0x80 >> (x % 8)) != 0);
        }
    }
    (pixels, mask)
}

/// Pixels (RGBA, top row first) of a PNG as the generator writes it: 8 bit RGBA, not interlaced.
fn decode_png(png: &[u8]) -> Vec<[u8; 4]> {
    let (w, h) = png_size(png);
    assert_eq!(
        (png[24], png[25], png[28]),
        (8, 6, 0),
        "PNG stage must be 8 bit RGBA, not interlaced"
    );
    let mut idat = Vec::new();
    let mut at = 8;
    while at + 8 <= png.len() {
        let len = u32::from_be_bytes([png[at], png[at + 1], png[at + 2], png[at + 3]]) as usize;
        if &png[at + 4..at + 8] == b"IDAT" {
            idat.extend_from_slice(&png[at + 8..at + 8 + len]);
        }
        at += 12 + len;
    }
    let mut raw = Vec::new();
    flate2::read::ZlibDecoder::new(idat.as_slice())
        .read_to_end(&mut raw)
        .expect("PNG data inflates");
    let stride = w as usize * 4;
    let mut out = vec![0u8; stride * h as usize];
    for y in 0..h as usize {
        let filter = raw[y * (stride + 1)];
        let line = &raw[y * (stride + 1) + 1..(y + 1) * (stride + 1)];
        for (x, &value) in line.iter().enumerate() {
            let left = if x >= 4 { out[y * stride + x - 4] } else { 0 };
            let up = if y > 0 { out[(y - 1) * stride + x] } else { 0 };
            let up_left = if x >= 4 && y > 0 {
                out[(y - 1) * stride + x - 4]
            } else {
                0
            };
            let predictor = match filter {
                0 => 0,
                1 => left,
                2 => up,
                3 => left.midpoint(up), // rounds down, as PNG's average filter
                4 => paeth(left, up, up_left),
                other => panic!("unknown PNG filter {other}"),
            };
            out[y * stride + x] = value.wrapping_add(predictor);
        }
    }
    out.chunks_exact(4)
        .map(|p| [p[0], p[1], p[2], p[3]])
        .collect()
}

fn paeth(left: u8, up: u8, up_left: u8) -> u8 {
    let estimate = i16::from(left) + i16::from(up) - i16::from(up_left);
    let distance = |v: u8| (estimate - i16::from(v)).abs();
    if distance(left) <= distance(up) && distance(left) <= distance(up_left) {
        left
    } else if distance(up) <= distance(up_left) {
        up
    } else {
        up_left
    }
}

/// A colour tuple of the generator, e.g. `VARIANT = (0xD4, 0x5D, 0x3D)`.
fn generator_colour(source: &str, name: &str) -> [u8; 3] {
    let head = format!("{name} = (");
    let start = source.find(&head).expect("colour in tools/icon.py") + head.len();
    let end = start + source[start..].find(')').expect("closing parenthesis");
    let channels: Vec<u8> = source[start..end]
        .split(',')
        .map(|c| u8::from_str_radix(c.trim().trim_start_matches("0x"), 16).expect("hex channel"))
        .collect();
    channels.try_into().expect("three channels")
}

/// A number constant of the generator, e.g. `MASK_ALPHA = 128`.
fn generator_number(source: &str, name: &str) -> u8 {
    let head = format!("\n{name} = ");
    let start = source.find(&head).expect("constant in tools/icon.py") + head.len();
    source[start..]
        .split(|c: char| !c.is_ascii_digit())
        .next()
        .and_then(|n| n.parse().ok())
        .expect("number")
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

/// The AND mask is the plate rounded to whole pixels: transparent exactly where the alpha is
/// below the generator's threshold (half coverage).
#[test]
fn and_mask_follows_the_alpha() {
    let source = std::fs::read_to_string(repo("tools/icon.py")).unwrap();
    let threshold = generator_number(&source, "MASK_ALPHA");
    let bytes = std::fs::read(repo("src-tauri/icons/icon.ico")).unwrap();
    for e in read_ico(&bytes) {
        let Some(mask) = &e.mask else { continue };
        let wrong = e
            .pixels
            .iter()
            .zip(mask)
            .filter(|(p, transparent)| **transparent != (p[3] < threshold))
            .count();
        assert_eq!(
            wrong, 0,
            "stage {}: {wrong} AND mask bits do not match the alpha",
            e.width
        );
    }
}

/// Every size the shell asks for at 100/125/150/175/200 % (Explorer, desktop, taskbar, Start,
/// Alt+Tab) has its own stage, so Windows draws it 1:1 instead of scaling another one.
#[test]
fn ico_has_a_stage_for_every_windows_scale() {
    let bytes = std::fs::read(repo("src-tauri/icons/icon.ico")).unwrap();
    let sizes: Vec<u32> = read_ico(&bytes).iter().map(|e| e.width).collect();
    for wanted in [16, 20, 24, 30, 32, 36, 40, 48, 60, 64, 72, 80, 96, 128, 256] {
        assert!(
            sizes.contains(&wanted),
            "icon.ico has no {wanted} stage ({sizes:?})"
        );
    }
}

/// No pixel of any ICO stage - transparent ones included - is darker than the plate's darkest
/// colour. A transparent black pixel (0,0,0,0) is invisible 1:1 but turns into a dark fringe
/// around the plate as soon as Windows scales the stage without premultiplying; premultiplied
/// storage darkens the edge the same way; a macOS drop shadow would show here too.
#[test]
fn ico_stages_have_no_dark_fringe() {
    let source = std::fs::read_to_string(repo("tools/icon.py")).unwrap();
    let darkest = generator_colour(&source, "VARIANT");
    let tolerance = generator_number(&source, "FRINGE_TOLERANCE");
    assert!(tolerance <= 4, "the fringe tolerance must stay small");
    let floor = darkest.map(|c| c.saturating_sub(tolerance));
    let bytes = std::fs::read(repo("src-tauri/icons/icon.ico")).unwrap();
    for e in read_ico(&bytes) {
        let dark: Vec<(usize, usize, [u8; 4])> = e
            .pixels
            .iter()
            .enumerate()
            .filter(|(_, p)| (0..3).any(|k| p[k] < floor[k]))
            .map(|(i, p)| (i % e.width as usize, i / e.width as usize, *p))
            .collect();
        assert!(
            dark.is_empty(),
            "stage {}: {} pixels darker than the plate {darkest:?} (first at x, y, rgba: {:?}) - \
             transparent pixels must carry the plate colour: python tools/icon.py",
            e.width,
            dark.len(),
            dark[0]
        );
    }
}

/// icon.png is the macOS 1024 with its drop shadow; Windows takes everything from icon.ico.
#[test]
fn windows_uses_only_the_ico() {
    for config in [
        "src-tauri/tauri.conf.json",
        "src-tauri/tauri.windows.conf.json",
    ] {
        let text = std::fs::read_to_string(repo(config)).unwrap();
        let json: serde_json::Value = serde_json::from_str(&text).unwrap();
        let icons = json["bundle"]["icon"].as_array().expect("bundle.icon");
        assert!(
            icons.iter().all(|i| i.as_str().is_some_and(|p| Path::new(p)
                .extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("ico")))),
            "{config}: Windows must not use the macOS PNG or ICNS: {icons:?}"
        );
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
