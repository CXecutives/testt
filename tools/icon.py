"""App icon: coral plate, white folder, check cut out of the folder.

One geometry, every output:
  src-tauri/icons/icon.ico    Windows: 48 first (Tauri takes the first entry as the window
                              icon; Windows scales it down cleanly), then 16 20 24 32 40 64 96 256
  src-tauri/icons/icon.icns   macOS: plate 824 of 1024 with margin, as the system icons
  src-tauri/icons/icon.png    1024, macOS layout (window icon on the other targets)
  ui/src/assets/app-icon.svg  the same paths as a vector (brand mark in the title bar)

Geometry on the 1024 grid (Windows layout; macOS scales everything with its smaller plate):
- Plate 48..976, corner radius 212 with 60 % corner smoothing (as iOS and Figma): the curve
  leaves the straight edge at 1.6 r with zero curvature, the circular middle keeps radius r.
  The 45 degree point therefore stays where the plain rounded square had it and the outline
  lies inside the plain one - softer, never boxier.
- Folder 224..800 wide; tab 252..316 (45 degree slope 436 -> 500), body 316..732. One radius
  (60) everywhere: the body and tab corners with the same smoothing, the slope with two
  circular fillets. Lifted 20 above the old position: its mass centre sits at the optical
  centre of the plate (slightly above the middle).
- Check: one stroke width (72), round caps and join, cut out of the folder (even-odd), so the
  plate gradient shows through. Optically centred in the body: the box is centred and moved
  up by half the distance between box centre and mass centre (the heavy bottom vertex).
- Gradient coral-glow hsl(13 73% 68%) top left -> coral-variant hsl(13 64% 56%) bottom right.

Small stages are hinted: straight edges on whole pixels (proportional positions, rounded
symmetrically), check vertices on half pixels, the check bolder up to 40 px.

    python tools/icon.py                       writes all four files (needs Pillow)
    python tools/icon.py --compare OUT OLD.py  comparison sheet against an older generator
"""
import math
import struct
import sys
from io import BytesIO
from pathlib import Path

from PIL import Image, ImageDraw

GLOW = (0xE9, 0x8C, 0x72)     # hsl(13 73% 68%)
VARIANT = (0xD7, 0x66, 0x47)  # hsl(13 64% 56%)
WHITE = (255, 255, 255)
SMOOTHING = 0.6

# 1024 grid, Windows layout.
PLATE, PLATE_R = 48, 212
FOLDER_X0, FOLDER_X1 = 224, 800
TAB_Y, BODY_Y0, BODY_Y1 = 252, 316, 732
SLOPE_X0, SLOPE_X1 = 436, 500
FOLDER_R = 60
CHECK_POINTS = [(381, 511), (473, 603), (643, 431)]
CHECK_W = 72
# macOS: plate 824 of 1024 (margin 100) with the corner of the old macOS plate (184 of 820,
# close to Apple's template).
MAC_PLATE = 100
MAC_PLATE_R = 184 * (1024 - 2 * MAC_PLATE) / 820

SIZES = [48, 16, 20, 24, 32, 40, 64, 96, 256]
# Check width in pixels at the small stages (bolder than the plain scale, which would be
# 1.1 px at 16).
MIN_CHECK_W = {16: 2.0, 20: 2.25, 24: 2.5, 32: 2.8, 40: 3.0}
# ICNS entries: OSType and edge length. 256 and 512 appear twice (plain and @2x), as Apple's
# iconutil writes them; PNG types only (macOS draws 16 from ic11 = 16@2x).
ICNS_ENTRIES = [('ic11', 32), ('ic12', 64), ('ic07', 128), ('ic13', 256),
                ('ic08', 256), ('ic14', 512), ('ic09', 512), ('ic10', 1024)]
SS = 16  # supersampling of the raster stages


# ----------------------------------------------------------------------------- paths

class Path2:
    """A closed outline of lines, cubic Beziers and circular arcs - written as SVG path data
    and flattened into a polygon for the raster stages, so both use the same curves."""

    def __init__(self, start):
        self.start = start
        self.segs = []
        self.pos = start

    def line(self, p):
        self.segs.append(('L', p))
        self.pos = p

    def cubic(self, c1, c2, p):
        self.segs.append(('C', c1, c2, p))
        self.pos = p

    def arc(self, center, r, a0, a1):
        """Arc around `center` from angle a0 to a1 (radians, y down: increasing = clockwise)."""
        p = (center[0] + r * math.cos(a1), center[1] + r * math.sin(a1))
        self.segs.append(('A', center, r, a0, a1, p))
        self.pos = p

    def svg(self, fmt):
        out = [f'M{fmt(self.start[0])} {fmt(self.start[1])}']
        for seg in self.segs:
            if seg[0] == 'L':
                out.append(f'L{fmt(seg[1][0])} {fmt(seg[1][1])}')
            elif seg[0] == 'C':
                out.append('C' + ' '.join(f'{fmt(x)} {fmt(y)}' for x, y in seg[1:]))
            else:
                _, _, r, a0, a1, p = seg
                large = 1 if abs(a1 - a0) > math.pi else 0
                sweep = 1 if a1 > a0 else 0
                out.append(f'A{fmt(r)} {fmt(r)} 0 {large} {sweep} {fmt(p[0])} {fmt(p[1])}')
        return ''.join(out) + 'Z'

    def points(self):
        pts = [self.start]
        pos = self.start
        for seg in self.segs:
            if seg[0] == 'L':
                pts.append(seg[1])
            elif seg[0] == 'C':
                (x0, y0), (x1, y1), (x2, y2), (x3, y3) = pos, *seg[1:]
                for i in range(1, 49):
                    t = i / 48
                    u = 1 - t
                    pts.append((u**3 * x0 + 3 * u * u * t * x1 + 3 * u * t * t * x2 + t**3 * x3,
                                u**3 * y0 + 3 * u * u * t * y1 + 3 * u * t * t * y2 + t**3 * y3))
            else:
                _, (cx, cy), r, a0, a1, _ = seg
                n = max(2, math.ceil(abs(a1 - a0) / math.radians(1.5)))
                for i in range(1, n + 1):
                    a = a0 + (a1 - a0) * i / n
                    pts.append((cx + r * math.cos(a), cy + r * math.sin(a)))
            pos = seg[-1]
        return pts


def add(p, *terms):
    x, y = p
    for k, v in terms:
        x += k * v[0]
        y += k * v[1]
    return (x, y)


def smooth_corner(path, v, e, f, r, xi=SMOOTHING):
    """A 90 degree corner at `v` (incoming direction e, outgoing f, clockwise) with radius r and
    corner smoothing xi: straight -> cubic (curvature from 0) -> circular arc -> cubic ->
    straight, symmetric about the diagonal. The path must stand at v - p*e."""
    p = (1 + xi) * r
    arc_deg = 90 * (1 - xi)
    s = math.sin(math.radians(arc_deg / 2)) * r * math.sqrt(2)  # arc chord per axis
    alpha = (90 - arc_deg) / 2
    p3p4 = r * math.tan(math.radians(alpha / 2))
    beta = 45 * xi
    c = p3p4 * math.cos(math.radians(beta))
    d = c * math.tan(math.radians(beta))
    b = (p - s - c - d) / 3
    a = 2 * b
    start = add(v, (-p, e))
    path.cubic(add(start, (a, e)), add(start, (a + b, e)), add(start, (a + b + c, e), (d, f)))
    center = add(v, (-r, e), (r, f))
    mid0 = path.pos
    arc_end = add(mid0, (s, e), (s, f))
    for q in (mid0, arc_end):
        assert abs(math.dist(q, center) - r) < 1e-6 * max(r, 1), 'arc points off the circle'
    a0 = math.atan2(mid0[1] - center[1], mid0[0] - center[0])
    a1 = math.atan2(arc_end[1] - center[1], arc_end[0] - center[0])
    if a1 < a0:  # clockwise on screen = increasing angle
        a1 += 2 * math.pi
    path.arc(center, r, a0, a1)
    path.cubic(add(arc_end, (d, e), (c, f)), add(arc_end, (d, e), (b + c, f)),
               add(arc_end, (d, e), (a + b + c, f)))
    return p


def fillet(path, v, e, f, r):
    """A circular fillet of radius r at `v` between directions e and f (any angle, either
    turn). The path must stand at v - t*e with t = fillet_length(e, f, r)."""
    cross = e[0] * f[1] - e[1] * f[0]
    phi = math.acos(max(-1, min(1, e[0] * f[0] + e[1] * f[1])))
    t = r * math.tan(phi / 2)
    t1 = add(v, (-t, e))
    n = (-e[1], e[0]) if cross > 0 else (e[1], -e[0])
    center = add(t1, (r, n))
    t2 = add(v, (t, f))
    a0 = math.atan2(t1[1] - center[1], t1[0] - center[0])
    a1 = math.atan2(t2[1] - center[1], t2[0] - center[0])
    if cross > 0 and a1 < a0:
        a1 += 2 * math.pi
    if cross < 0 and a1 > a0:
        a1 -= 2 * math.pi
    path.arc(center, r, a0, a1)


def fillet_length(e, f, r):
    phi = math.acos(max(-1, min(1, e[0] * f[0] + e[1] * f[1])))
    return r * math.tan(phi / 2)


def unit(dx, dy):
    n = math.hypot(dx, dy)
    return (dx / n, dy / n)


RIGHT, DOWN, LEFT, UP = (1, 0), (0, 1), (-1, 0), (0, -1)


def squircle(x0, y0, x1, y1, r):
    p = (1 + SMOOTHING) * r
    path = Path2((x0 + p, y0))
    for v, e, f in [((x1, y0), RIGHT, DOWN), ((x1, y1), DOWN, LEFT),
                    ((x0, y1), LEFT, UP), ((x0, y0), UP, RIGHT)]:
        path.line(add(v, (-p, e)))
        smooth_corner(path, v, e, f, r)
    return path


def folder(g):
    """Folder outline, clockwise from the tab's top edge."""
    r = g['folder_r']
    x0, x1, tab_y, y0, y1 = g['x0'], g['x1'], g['tab_y'], g['y0'], g['y1']
    s0, s1 = g['slope_x0'], g['slope_x1']
    slope = unit(s1 - s0, y0 - tab_y)
    p = (1 + SMOOTHING) * r
    path = Path2((x0 + p, tab_y))
    t_top = fillet_length(RIGHT, slope, r)
    path.line(add((s0, tab_y), (-t_top, RIGHT)))
    fillet(path, (s0, tab_y), RIGHT, slope, r)
    t_bottom = fillet_length(slope, RIGHT, r)
    path.line(add((s1, y0), (-t_bottom, slope)))
    fillet(path, (s1, y0), slope, RIGHT, r)
    for v, e, f in [((x1, y0), RIGHT, DOWN), ((x1, y1), DOWN, LEFT), ((x0, y1), LEFT, UP)]:
        path.line(add(v, (-p, e)))
        smooth_corner(path, v, e, f, r)
    path.line(add((x0, tab_y), (-p, UP)))
    smooth_corner(path, (x0, tab_y), UP, RIGHT, r)
    return path


def check(g):
    """Outline of the check stroke (round caps, round outer join), clockwise."""
    (ax, ay), (bx, by), (cx, cy) = g['check']
    h = g['check_w'] / 2
    u1, u2 = unit(bx - ax, by - ay), unit(cx - bx, cy - by)
    # Left normals in screen coordinates (y down): rotate the direction by -90 degrees.
    n1, n2 = (u1[1], -u1[0]), (u2[1], -u2[0])
    a_l, b_l1 = add((ax, ay), (h, n1)), add((bx, by), (h, n1))
    b_l2, c_l = add((bx, by), (h, n2)), add((cx, cy), (h, n2))
    # Inner corner: the two left offset lines meet.
    den = u1[0] * u2[1] - u1[1] * u2[0]
    t = ((b_l2[0] - a_l[0]) * u2[1] - (b_l2[1] - a_l[1]) * u2[0]) / den
    inner = add(a_l, (t, u1))
    ang = lambda v: math.atan2(v[1], v[0])
    path = Path2(a_l)
    path.line(inner)
    path.line(c_l)
    # Cap at C: half circle from the left side over the tip to the right side.
    a0 = ang(n2)
    path.arc((cx, cy), h, a0, a0 + math.pi)
    path.line(add((bx, by), (-h, n2)))
    # Outer join at B: from the right side of the second arm back to that of the first.
    a0, a1 = ang((-n2[0], -n2[1])), ang((-n1[0], -n1[1]))
    if a1 < a0:
        a1 += 2 * math.pi
    path.arc((bx, by), h, a0, a1)
    path.line(add((ax, ay), (-h, n1)))
    a0 = ang((-n1[0], -n1[1]))
    path.arc((ax, ay), h, a0, a0 + math.pi)
    return path


# --------------------------------------------------------------------------- layout

def layout(s, mac=False):
    """Geometry of one stage in target pixels. Straight edges are placed proportionally on
    the plate and rounded symmetrically to whole pixels; radii and the check scale freely
    (check vertices on half pixels at small sizes)."""
    k = s / 1024
    inset = MAC_PLATE if mac else PLATE
    margin = max(1, round(inset * k)) if s < 1024 else inset
    plate = s - 2 * margin
    unit_ = plate / (1024 - 2 * PLATE)  # one grid unit of the Windows plate

    def pos(v):
        return margin + (v - PLATE) * unit_

    def snap(v):
        return round(pos(v)) if s <= 256 else pos(v)

    side = snap(FOLDER_X0) - margin
    g = dict(
        s=s,
        plate=(margin, margin, s - margin, s - margin),
        plate_r=(MAC_PLATE_R if mac else PLATE_R) * plate / (1024 - 2 * inset),
        x0=margin + side,
        x1=s - margin - side,
        tab_y=snap(TAB_Y),
        y0=snap(BODY_Y0),
        y1=snap(BODY_Y1),
        slope_x0=pos(SLOPE_X0),
        folder_r=FOLDER_R * unit_,
    )
    g['slope_x1'] = g['slope_x0'] + (g['y0'] - g['tab_y'])  # keep the slope at 45 degrees

    def point(x, y):
        """Same place in the (snapped) folder body as on the grid."""
        px = g['x0'] + (x - FOLDER_X0) / (FOLDER_X1 - FOLDER_X0) * (g['x1'] - g['x0'])
        py = g['y0'] + (y - BODY_Y0) / (BODY_Y1 - BODY_Y0) * (g['y1'] - g['y0'])
        if s <= 64:
            px, py = round(px * 2) / 2, round(py * 2) / 2
        return (px, py)

    g['check'] = [point(x, y) for x, y in CHECK_POINTS]
    g['check_w'] = max(CHECK_W * unit_, MIN_CHECK_W.get(s, 0))
    # The check stays a check: its arms keep the 45 degree directions after snapping.
    (ax, ay), (bx, by), (cx, cy) = g['check']
    g['check'][0] = (bx - (by - ay), ay)
    g['check'][2] = (bx + (by - cy), cy)
    return g


# --------------------------------------------------------------------------- raster

def gradient(size, box):
    """Diagonal gradient over the plate box (the colour depends on x+y only)."""
    x0, y0, x1, _ = box
    span = 2 * (x1 - x0)
    ramp = Image.new('RGB', (2 * size, 1))
    ramp.putdata([tuple(round(a + (b - a) * min(1, max(0, (i + 1 - x0 - y0) / span)))
                        for a, b in zip(GLOW, VARIANT)) for i in range(2 * size)])
    g = Image.new('RGB', (size, size))
    for y in range(size):
        g.paste(ramp.crop((y, 0, y + size, 1)), (0, y))
    return g


def render(s, mac=False, ss=None):
    ss = ss or min(SS, max(4, 4096 // s))
    g = layout(s, mac)
    S = s * ss
    scale = lambda pts: [(x * ss, y * ss) for x, y in pts]
    plate_mask = Image.new('L', (S, S), 0)
    ImageDraw.Draw(plate_mask).polygon(scale(squircle(*g['plate'], g['plate_r']).points()), fill=255)
    folder_mask = Image.new('L', (S, S), 0)
    draw = ImageDraw.Draw(folder_mask)
    draw.polygon(scale(folder(g).points()), fill=255)
    draw.polygon(scale(check(g).points()), fill=0)
    img = Image.new('RGBA', (S, S), (0, 0, 0, 0))
    img.paste(gradient(S, tuple(v * ss for v in g['plate'])), (0, 0), plate_mask)
    img.paste(Image.new('RGB', (S, S), WHITE), (0, 0), folder_mask)
    return img.resize((s, s), Image.BOX)


def dib(img):
    """A stage as an uncompressed DIB: BITMAPINFOHEADER, BGRA bottom up, then the AND mask
    (from the alpha channel; tools that only read the mask would see an opaque border
    otherwise). Only 256 is stored as PNG: older image libraries cannot read smaller
    compressed stages, and Windows then shows nothing without an error."""
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
    """ICNS (a plain container: `icns`, total length, then per entry OSType, length incl. the
    8 header bytes and a complete PNG) and the 1024 PNG."""
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


def build_svg(out):
    g = layout(1024)
    fmt = lambda v: f'{v:.2f}'.rstrip('0').rstrip('.')
    x0, y0, x1, y1 = g['plate']
    hexa = lambda c: '#' + ''.join(f'{v:02X}' for v in c)
    out.write_text(f'''<svg xmlns="http://www.w3.org/2000/svg" viewBox="{x0} {y0} {x1 - x0} {y1 - y0}" width="{x1 - x0}" height="{y1 - y0}">
  <!-- Generated by tools/icon.py - do not edit. The app icon as a vector: the same paths as
       icon.ico, icon.icns and icon.png; the check is cut out of the folder (even-odd). -->
  <defs>
    <linearGradient id="plate" x1="{x0}" y1="{y0}" x2="{x1}" y2="{y1}" gradientUnits="userSpaceOnUse">
      <stop offset="0" stop-color="{hexa(GLOW)}"/>
      <stop offset="1" stop-color="{hexa(VARIANT)}"/>
    </linearGradient>
  </defs>
  <path fill="url(#plate)" d="{squircle(x0, y0, x1, y1, g['plate_r']).svg(fmt)}"/>
  <path fill="{hexa(WHITE)}" fill-rule="evenodd" d="{folder(g).svg(fmt)}{check(g).svg(fmt)}"/>
</svg>
''', encoding='utf-8', newline='\n')


# ----------------------------------------------------------------------- comparison

def compare(out, old_generator):
    """Old vs new at 16, 24, 32, 48 (1:1 and x4), 256 and 512 (Windows and macOS layout), on
    white and on the app's cream background."""
    import importlib.util
    spec = importlib.util.spec_from_file_location('icon_old', old_generator)
    old = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(old)
    cream = (248, 245, 241)  # hsl(32 33% 96%)
    small = [16, 24, 32, 48]
    gap = 24

    def stages(mod, new):
        imgs = [mod.render(s) for s in small]
        imgs += [i.resize((i.width * 4, i.width * 4), Image.NEAREST) for i in imgs]
        imgs.append(mod.render(256))
        big_mac = mod.render(1024, mac=True, ss=4) if new else mod.mac_stage(1024)
        imgs += [mod.render(1024, ss=4).resize((512, 512), Image.LANCZOS),
                 big_mac.resize((512, 512), Image.LANCZOS)]
        return imgs

    rows = [('old', stages(old, False)), ('new', stages(sys.modules[__name__], True))]
    width = gap + sum(i.width + gap for i in rows[0][1])
    row_h = 512 + 2 * gap
    sheet = Image.new('RGB', (width, 4 * row_h), WHITE)
    y = 0
    for bg in (WHITE, cream):
        for _, imgs in rows:
            band = Image.new('RGB', (width, row_h), bg)
            x = gap
            for img in imgs:
                band.paste(img, (x, gap + (512 - img.height) // 2), img)
                x += img.width + gap
            sheet.paste(band, (0, y))
            y += row_h
    sheet.save(out)


if __name__ == '__main__':
    root = Path(__file__).resolve().parent.parent
    if len(sys.argv) == 4 and sys.argv[1] == '--compare':
        compare(sys.argv[2], sys.argv[3])
    else:
        icons = root / 'src-tauri' / 'icons'
        build_ico(icons / 'icon.ico')
        build_mac(icons / 'icon.icns', icons / 'icon.png')
        build_svg(root / 'ui' / 'src' / 'assets' / 'app-icon.svg')
