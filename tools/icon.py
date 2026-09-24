"""App icon: coral plate, white folder, coral check mark - the single source of every icon.

Writes
    src-tauri/icons/icon.ico       Windows (stages 48, 16, 24, 32, 64, 256; 48 first)
    src-tauri/icons/icon.icns      macOS bundle (PNG entries, written directly)
    src-tauri/icons/icon.png       1024 px (Tauri's window icon on non-Windows targets)
    ui/src/assets/app-icon.svg     the same geometry as a vector (BrandMark.svelte)

Geometry (all in plate units 0..1, so every output shares it):
- plate: a continuous-corner squircle (superellipse, exponent 5) with a diagonal gradient
  from light coral (top left) to deeper coral (bottom right). macOS and the 1024 PNG use the
  usual macOS body of 824/1024; Windows stages fill the tile up to a 1/16 margin.
- folder: white, corner radius R everywhere; the tab ends in an S-curve (convex, then
  concave, both half the tab height) instead of a hard diagonal.
- check: optically centred in the folder body (a little above its geometric centre),
  round caps and joins, one stroke width.
Small stages (16 and 24 px) are drawn for the pixel grid: whole-pixel folder edges, a
simplified square tab and a bolder check; 32 and 48 px get a slightly bolder check too.

    python tools/icon.py            (needs Pillow)
"""
import math
import struct
from io import BytesIO
from pathlib import Path

from PIL import Image, ImageDraw

ROOT = Path(__file__).resolve().parent.parent

GLOW = (0xE9, 0x8C, 0x72)     # hsl(13 73% 68%) - coral-glow token
VARIANT = (0xD7, 0x66, 0x47)  # hsl(13 64% 56%) - coral-variant token
CHECK = (0xD0, 0x5E, 0x3F)    # a step deeper than VARIANT: contrast on white
WHITE = (255, 255, 255)

SQUIRCLE_N = 5.0
MAC_MARGIN = 100 / 1024       # macOS icon grid: 824 px body in a 1024 canvas

# Folder and check in plate units (derived from the previous icon, then refined).
BODY = (0.1897, 0.3103, 0.8103, 0.7586)  # x0, top, x1, bottom
TAB_Y = 0.2414                            # top of the tab
TAB_X = 0.4526                            # x of the tab's right edge (centre of the S-curve)
RADIUS = 0.0647                           # folder corner radius
CHECK_POINTS = [(0.3588, 0.5135), (0.4580, 0.6126), (0.6412, 0.4273)]
CHECK_WIDTH = 0.0733

# ICO stages. Tauri takes the FIRST entry as window icon (title bar, Alt+Tab, taskbar), so
# it is the 48 stage - Windows scales that down cleanly instead of blowing a small one up.
SIZES = [48, 16, 24, 32, 64, 256]
# Minimum check stroke in px for small stages (bolder than the plain scale-down).
BOLD = {16: 2.0, 24: 2.6, 32: 3.0, 48: 3.8}
SIMPLE_TAB = 24               # up to this size the tab is a plain pixel step
# ICNS entries: OSType and edge length (PNG types only; 16 px is drawn from ic11 = 16@2x).
ICNS_ENTRIES = [('ic11', 32), ('ic12', 64), ('ic07', 128), ('ic13', 256),
                ('ic08', 256), ('ic14', 512), ('ic09', 512), ('ic10', 1024)]


# ------------------------------------------------------------------- geometry

def squircle(x0, y0, size, steps=256):
    """Points of a superellipse filling the square (x0, y0, size)."""
    a = size / 2
    cx, cy = x0 + a, y0 + a
    pts = []
    for i in range(steps):
        t = 2 * math.pi * i / steps
        c, s = math.cos(t), math.sin(t)
        pts.append((cx + a * math.copysign(abs(c) ** (2 / SQUIRCLE_N), c),
                    cy + a * math.copysign(abs(s) ** (2 / SQUIRCLE_N), s)))
    return pts


def folder_segments(x0, top, x1, bottom, tab_y, tab_x, r):
    """The folder outline as segments: ('M'|'L', x, y) and ('A', rx, sweep, x, y)."""
    h = top - tab_y
    k = h / 2  # S-curve radii: convex then concave, together the tab height
    return [
        ('M', x0, tab_y + r),
        ('A', r, 1, x0 + r, tab_y),
        ('L', tab_x - k, tab_y),
        ('A', k, 1, tab_x, tab_y + k),
        ('A', k, 0, tab_x + k, top),
        ('L', x1 - r, top),
        ('A', r, 1, x1, top + r),
        ('L', x1, bottom - r),
        ('A', r, 1, x1 - r, bottom),
        ('L', x0 + r, bottom),
        ('A', r, 1, x0, bottom - r),
    ]


def simple_folder(x0, top, x1, bottom, tab_y, tab_x):
    """Small stages: body rectangle plus a square tab step, all on whole pixels."""
    return [
        ('M', x0, tab_y), ('L', tab_x, tab_y), ('L', tab_x, top), ('L', x1, top),
        ('L', x1, bottom), ('L', x0, bottom),
    ]


def flatten(segments, steps=32):
    """Segments to polygon points (arcs are quarter circles with the given sweep)."""
    pts, cur = [], None
    for seg in segments:
        if seg[0] in 'ML':
            cur = (seg[1], seg[2])
            pts.append(cur)
            continue
        _, r, sweep, x, y = seg
        # A quarter arc: its centre is one of the two free corners of the box spanned by
        # the end points - the one around which the turn has the sweep's direction
        # (sweep 1 = clockwise on screen, as in SVG).
        px, py = cur
        for cx, cy in ((px, y), (x, py)):
            a0 = math.atan2(py - cy, px - cx)
            a1 = math.atan2(y - cy, x - cx)
            d = (a1 - a0 + math.pi) % (2 * math.pi) - math.pi
            if (d > 0) == bool(sweep):
                break
        for i in range(1, steps + 1):
            a = a0 + d * i / steps
            pts.append((cx + r * math.cos(a), cy + r * math.sin(a)))
        cur = (x, y)
    return pts


def layout(s, mac):
    """Geometry of one stage in target pixels."""
    margin = s * MAC_MARGIN if mac else max(1, round(s / 16))
    plate = s - 2 * margin
    u = lambda v: margin + v * plate  # noqa: E731 - plate units to pixels
    small = not mac and s <= SIMPLE_TAB
    snap = round if (not mac and s <= 48) else (lambda v: v)
    x0, top, x1, bottom = (snap(u(v)) for v in BODY)
    tab_y, tab_x = snap(u(TAB_Y)), snap(u(TAB_X))
    if small and tab_y >= top:
        tab_y = top - 1
    width = max(CHECK_WIDTH * plate, BOLD.get(s, 0) if not mac else 0)
    check = [(u(x), u(y)) for x, y in CHECK_POINTS]
    if small:
        folder = simple_folder(x0, top, x1, bottom, tab_y, tab_x)
    else:
        folder = folder_segments(x0, top, x1, bottom, tab_y, tab_x, RADIUS * plate)
    return dict(margin=margin, plate=plate, folder=folder, check=check, width=width)


# ------------------------------------------------------------------ raster

def gradient(size):
    """Diagonal GLOW -> VARIANT; the colour depends on x + y only."""
    span = 2 * (size - 1)
    ramp = Image.new('RGB', (span + 1, 1))
    ramp.putdata([tuple(round(a + (b - a) * (i / span)) for a, b in zip(GLOW, VARIANT))
                  for i in range(span + 1)])
    g = Image.new('RGB', (size, size))
    for y in range(size):
        g.paste(ramp.crop((y, 0, y + size, 1)), (0, y))
    return g


def render(s, mac=False):
    """One stage, supersampled (about 4096 px wide) and box-filtered down."""
    ss = min(16, max(4, 4096 // s))
    L = layout(s, mac)
    S = s * ss
    sc = lambda p: (p[0] * ss, p[1] * ss)  # noqa: E731
    img = Image.new('RGBA', (S, S), (0, 0, 0, 0))
    mask = Image.new('L', (S, S), 0)
    m = L['margin']
    ImageDraw.Draw(mask).polygon([sc(p) for p in squircle(m, m, L['plate'], 720)], fill=255)
    img.paste(gradient(S), (0, 0), mask)
    d = ImageDraw.Draw(img)
    d.polygon([sc(p) for p in flatten(L['folder'])], fill=WHITE)
    pts = [sc(p) for p in L['check']]
    w = L['width'] * ss
    d.line(pts, fill=CHECK, width=round(w), joint='curve')
    for x, y in (pts[0], pts[-1]):
        d.ellipse([x - w / 2, y - w / 2, x + w / 2, y + w / 2], fill=CHECK)
    return img.resize((s, s), Image.BOX)


# --------------------------------------------------------------------- SVG

def svg():
    """The 1024 macOS geometry as a vector, cropped to the plate (fills its box)."""
    L = layout(1024, mac=True)
    m, p = L['margin'], L['plate']
    f = lambda v: f'{v:.1f}'.rstrip('0').rstrip('.')  # noqa: E731
    plate = 'M' + 'L'.join(f'{f(x)} {f(y)}' for x, y in squircle(m, m, p, 180)) + 'Z'
    folder = ''
    for seg in L['folder']:
        if seg[0] in 'ML':
            folder += f'{seg[0]}{f(seg[1])} {f(seg[2])}'
        else:
            _, r, sweep, x, y = seg
            folder += f'A{f(r)} {f(r)} 0 0 {sweep} {f(x)} {f(y)}'
    folder += 'Z'
    check = ' '.join(f'{f(x)},{f(y)}' for x, y in L['check'])
    return f'''<svg xmlns="http://www.w3.org/2000/svg" viewBox="{f(m)} {f(m)} {f(p)} {f(p)}" width="{f(p)}" height="{f(p)}">
  <!-- Generated by tools/icon.py - do not edit. Coral plate, white folder, check mark. -->
  <defs>
    <linearGradient id="plate" x1="0" y1="0" x2="1" y2="1">
      <stop offset="0" stop-color="#{GLOW[0]:02X}{GLOW[1]:02X}{GLOW[2]:02X}"/>
      <stop offset="1" stop-color="#{VARIANT[0]:02X}{VARIANT[1]:02X}{VARIANT[2]:02X}"/>
    </linearGradient>
  </defs>
  <path fill="url(#plate)" d="{plate}"/>
  <path fill="#FFFFFF" d="{folder}"/>
  <polyline points="{check}" fill="none" stroke="#{CHECK[0]:02X}{CHECK[1]:02X}{CHECK[2]:02X}" stroke-width="{f(L['width'])}" stroke-linecap="round" stroke-linejoin="round"/>
</svg>
'''


# -------------------------------------------------------------- containers

def dib(img):
    """A stage as uncompressed DIB: BITMAPINFOHEADER, BGRA bottom-up, then the AND mask.

    Only the 256 stage is stored as PNG: tools that read ICO files (older Windows imaging
    libraries included) cannot always unpack compressed smaller stages. The AND mask comes
    from the alpha channel; a zero mask would give 1-bit readers an opaque square.
    """
    s = img.width
    head = struct.pack('<IiiHHIIiiII', 40, s, s * 2, 1, 32, 0, 0, 0, 0, 0, 0)
    px = img.load()
    rows = list(reversed(range(s)))
    xor = b''.join(bytes(v for x in range(s) for v in
                         (lambda p: (p[2], p[1], p[0], p[3]))(px[x, y]))
                   for y in rows)
    stride = ((s + 31) // 32) * 4
    mask = bytearray()
    for y in rows:
        row = bytearray(stride)
        for x in range(s):
            if px[x, y][3] == 0:
                row[x >> 3] |= 0x80 >> (x & 7)
        mask += row
    return head + xor + bytes(mask)


def build_ico(out):
    frames = []
    for s in SIZES:
        img = render(s)
        if s >= 256:
            buf = BytesIO()
            img.save(buf, 'PNG')
            frames.append((s, buf.getvalue()))
        else:
            frames.append((s, dib(img)))
    header = struct.pack('<HHH', 0, 1, len(frames))
    offset = len(header) + 16 * len(frames)
    entries, blobs = b'', b''
    for s, data in frames:
        entries += struct.pack('<BBBBHHII', s % 256, s % 256, 0, 0, 1, 32, len(data), offset)
        blobs += data
        offset += len(data)
    out.write_bytes(header + entries + blobs)


def build_mac(icns_out, png_out):
    """ICNS (tag, total length, then OSType + length + PNG per entry) and the 1024 PNG."""
    frames = {}
    for _, s in ICNS_ENTRIES:
        if s not in frames:
            buf = BytesIO()
            render(s, mac=True).save(buf, 'PNG')
            frames[s] = buf.getvalue()
    body = b''.join(ostype.encode('ascii') + struct.pack('>I', len(frames[s]) + 8) + frames[s]
                    for ostype, s in ICNS_ENTRIES)
    icns_out.write_bytes(b'icns' + struct.pack('>I', len(body) + 8) + body)
    png_out.write_bytes(frames[1024])


if __name__ == '__main__':
    icons = ROOT / 'src-tauri' / 'icons'
    build_ico(icons / 'icon.ico')
    build_mac(icons / 'icon.icns', icons / 'icon.png')
    (ROOT / 'ui' / 'src' / 'assets' / 'app-icon.svg').write_text(svg(), encoding='utf-8', newline='\n')
