"""App icon: coral plate, white folder, check cut out of the folder.

One geometry, every output:
  src-tauri/icons/icon.ico    Windows only: Windows layout, no shadow. One stage for every size
                              the shell asks for at 100 to 200 % (see SIZES), 48 first: Tauri
                              takes the first entry as the window icon
  src-tauri/icons/icon.icns   macOS: plate 824 of 1024 with margin and a soft drop shadow, as
                              the system icons
  src-tauri/icons/icon.png    the 1024 macOS entry (macOS bundle icon, window and Dock icon of
                              `tauri dev`). Deliberately macOS only: no Windows config lists it,
                              Windows takes every size from icon.ico
  ui/src/assets/app-icon.svg  the same paths as a vector (brand mark in the title bar)

Edges, in every raster output: colour and coverage are rendered apart, so a partly covered
pixel carries the plate colour under it (straight alpha), and every fully transparent pixel
takes the colour of its nearest visible neighbours (colour bleed). A scaler that filters
without premultiplying - the Windows shell whenever it has no stage of the wanted size - then
mixes plate colour into the edge instead of black: no dark fringe on any background.

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

    python tools/icon.py                        writes all four files, then checks icon.ico
                                                (needs Pillow)
    python tools/icon.py --compare OUT OLD.py   comparison sheet against an older generator
    python tools/icon.py --fringe-sheet OUT [OLD.ico]
                                                edge check of icon.ico on dark, grey and white:
                                                1:1, zoomed and scaled as the shell scales;
                                                with OLD.ico as the row before
"""
import math
import struct
import sys
from io import BytesIO
from pathlib import Path

from PIL import Image, ImageChops, ImageDraw, ImageFilter, ImageFont

GLOW = (0xEB, 0x95, 0x7D)     # hsl(13 73% 70.5%) - lighter than the coral-glow token
VARIANT = (0xD4, 0x5D, 0x3D)  # hsl(13 64% 53.5%) - deeper: a 5 % stronger gradient
WHITE = (255, 255, 255)
SMOOTHING = 0.6
# The plate has the macOS app-icon shape: straight sides and Apple's continuous corners
# (see `apple_corner`) with a radius of 22.37 % of the plate.
PLATE_SMOOTHING = 0.6  # used by the folder corners only

# 1024 grid, Windows layout.
PLATE, PLATE_R = 48, round(0.2237 * (1024 - 2 * 48))
FOLDER_X0, FOLDER_X1 = 224, 800
TAB_Y, BODY_Y0, BODY_Y1 = 252, 316, 732
SLOPE_X0, SLOPE_X1 = 436, 500
FOLDER_R = 60
CHECK_POINTS = [(381, 511), (473, 603), (643, 431)]
CHECK_W = 72
# macOS: plate 824 of 1024 (margin 100) with the corner of the old macOS plate (184 of 820,
# close to Apple's template).
MAC_PLATE = 100
MAC_PLATE_R = 0.2237 * (1024 - 2 * MAC_PLATE)

# ICO stages. The shell asks for these at 100/125/150/175/200 %: small icons 16 20 24 32,
# taskbar 24 30 36 48, desktop 48 60 72 96, Start and Alt+Tab in between, large and extra large
# 96 128 256. An exact stage is drawn 1:1; for the rest (28, 42, 84) the shell scales the next
# larger one. 48 comes first: Tauri takes the first entry as the window icon.
SIZES = [48, 16, 20, 24, 30, 32, 36, 40, 60, 64, 72, 80, 96, 128, 256]
# Check width in pixels at the small stages (bolder than the plain scale, which would be
# 1.1 px at 16).
MIN_CHECK_W = {16: 2.0, 20: 2.25, 24: 2.5, 30: 2.7, 32: 2.8, 36: 2.9, 40: 3.0}
# AND mask of the ICO stages: a pixel counts as transparent below half coverage (as icotool's
# default threshold), so a reader that ignores alpha sees the plate in its right size.
MASK_ALPHA = 128
# No pixel of a Windows stage - transparent or not - may be darker than the plate's darkest
# colour by more than this (per channel): a darker one is a fringe once the shell scales.
FRINGE_TOLERANCE = 2
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


# Apple's continuous-corner curve (the path UIKit draws for rounded rectangles since iOS 7),
# as distances (back along the incoming edge, forward along the outgoing edge) in units of r.
APPLE_CORNER = [
    ((1.08849323, 0.0), (0.86840689, 0.0), (0.63149399, 0.07491100)),
    ((0.37282392, 0.16905899), (0.16905883, 0.37282401), (0.07491176, 0.63149399)),
    ((0.0, 0.86840701), (0.0, 1.08849299), (0.0, 1.52866483)),
]
APPLE_EXTENT = 1.52866483  # the curve starts this many r before the corner


def apple_corner(path, v, e, f, r):
    """One corner of the plate with Apple's continuous curvature; the path stands at
    v - APPLE_EXTENT*r*e (incoming direction e, outgoing f)."""
    def pt(u, w):
        return add(v, (-u * r, e), (w * r, f))
    for c1, c2, end in APPLE_CORNER:
        path.cubic(pt(*c1), pt(*c2), pt(*end))


def squircle(x0, y0, x1, y1, r):
    """The plate: long straight sides, only the corners curve (iOS/macOS app-icon shape)."""
    p = APPLE_EXTENT * r
    path = Path2((x0 + p, y0))
    for v, e, f in [((x1, y0), RIGHT, DOWN), ((x1, y1), DOWN, LEFT),
                    ((x0, y1), LEFT, UP), ((x0, y0), UP, RIGHT)]:
        path.line(add(v, (-p, e)))
        apple_corner(path, v, e, f, r)
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
    """One stage as straight (not premultiplied) RGBA. Colour and coverage are rendered apart:
    the colour layer is opaque everywhere (the plate gradient over the whole canvas, the folder
    in white), so a partly covered edge pixel gets the plate colour under it, never a mix with
    the black of an empty canvas. Transparent pixels are coloured by `bleed`."""
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
    colour = gradient(S, tuple(v * ss for v in g['plate']))
    colour.paste(Image.new('RGB', (S, S), WHITE), (0, 0), folder_mask)
    rgb = colour.resize((s, s), Image.BOX)
    cover = plate_mask.resize((s, s), Image.BOX)
    if not mac:
        img = rgb.convert('RGBA')
        img.putalpha(cover)
        return bleed(img)
    # macOS icons carry a soft drop shadow inside their canvas, like the Dock's icons: black at
    # 28 %, blurred and moved down by about 1 % of the canvas. The plate mask is binary, so
    # plate over shadow covers the lighter of the two.
    shadow = plate_mask.point(lambda a: a * 0.28).filter(ImageFilter.GaussianBlur(S * 0.0098))
    moved = Image.new('L', (S, S), 0)
    moved.paste(shadow, (0, round(S * 0.0098)))
    alpha = ImageChops.lighter(plate_mask, moved).resize((s, s), Image.BOX)
    # Straight colour of the plate over a black shadow: the plate's share of the coverage.
    img = Image.new('RGBA', (s, s))
    img.putdata([(round(r * c / a), round(g_ * c / a), round(b * c / a), a) if a else (r, g_, b, 0)
                 for (r, g_, b), c, a in zip(pixels(rgb), pixels(cover), pixels(alpha))])
    return bleed(img)


def pixels(img):
    """The pixel values in row order (Pillow 12 replaced `getdata`)."""
    flat = getattr(img, 'get_flattened_data', None)
    return flat() if flat else tuple(img.getdata())


def bleed(img):
    """Colour bleed: every fully transparent pixel takes the mean colour of its visible
    8-neighbours, ring by ring outwards from the visible pixels; alpha stays 0. The nearest
    visible pixel is a plate pixel (or, on macOS, the black shadow), so a filter that mixes a
    transparent pixel into the edge mixes in that colour, never the black of an empty canvas."""
    s = img.width
    px = pixels(img)
    colour = [p[:3] if p[3] else None for p in px]

    def around(i):
        x, y = i % s, i // s
        for dy in (-1, 0, 1):
            for dx in (-1, 0, 1):
                if (dx or dy) and 0 <= x + dx < s and 0 <= y + dy < s:
                    yield i + dy * s + dx

    ring = {i for i, c in enumerate(colour)
            if c is None and any(colour[j] is not None for j in around(i))}
    while ring:
        fill = {}
        for i in ring:
            near = [colour[j] for j in around(i) if colour[j] is not None]
            fill[i] = tuple(round(sum(c[k] for c in near) / len(near)) for k in range(3))
        for i, c in fill.items():
            colour[i] = c
        ring = {j for i in fill for j in around(i) if colour[j] is None}
    img.putdata([(*c, p[3]) if c is not None else p for c, p in zip(colour, px)])
    return img


def dib(img):
    """A stage as an uncompressed DIB: BITMAPINFOHEADER, straight BGRA bottom up (Windows
    premultiplies itself), then the AND mask: 1 where the pixel is less than half covered, so
    a reader that uses only the mask gets the plate in its size - with the bled colour, not
    black, at the edge. Only 256 is stored as PNG: older image libraries cannot read smaller
    compressed stages, and Windows then shows nothing without an error."""
    s = img.width
    head = struct.pack('<IiiHHIIiiII', 40, s, s * 2, 1, 32, 0, 0, 0, 0, 0, 0)
    bottom_up = img.transpose(Image.FLIP_TOP_BOTTOM)
    r, g, b, a = bottom_up.split()
    xor = Image.merge('RGBA', (b, g, r, a)).tobytes()  # byte order B G R A
    alpha = a.tobytes()
    stride = ((s + 31) // 32) * 4
    mask = bytearray()
    for y in range(s):
        row = bytearray(stride)
        for x in range(s):
            if alpha[y * s + x] < MASK_ALPHA:
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


def read_ico(path):
    """The stages of an ICO file in file order: (size, straight RGBA image, AND mask as rows of
    booleans - None for a PNG stage)."""
    data = Path(path).read_bytes()
    count = struct.unpack_from('<H', data, 4)[0]
    stages = []
    for i in range(count):
        w, _, _, _, _, _, length, offset = struct.unpack_from('<BBBBHHII', data, 6 + 16 * i)
        s = w or 256
        blob = data[offset:offset + length]
        if blob.startswith(b'\x89PNG'):
            img = Image.open(BytesIO(blob))
            img.load()
            stages.append((s, img.convert('RGBA'), None))
            continue
        head, bpp = struct.unpack_from('<I', blob)[0], struct.unpack_from('<H', blob, 14)[0]
        assert bpp == 32, f'stage {s}: {bpp} bpp'
        pixels = blob[head:head + s * s * 4]
        b, g, r, a = Image.frombytes('RGBA', (s, s), pixels).split()  # stored as B G R A
        img = Image.merge('RGBA', (r, g, b, a)).transpose(Image.FLIP_TOP_BOTTOM)
        stride = ((s + 31) // 32) * 4
        and_mask = blob[head + s * s * 4:]
        mask = [[bool(and_mask[(s - 1 - y) * stride + (x >> 3)] & (0x80 >> (x & 7)))
                 for x in range(s)] for y in range(s)]
        stages.append((s, img, mask))
    return stages


def dark_pixels(img, visible_only=False):
    """Pixels darker than the plate's darkest colour (any channel below VARIANT by more than
    FRINGE_TOLERANCE): (x, y, rgba). Every colour of the Windows icon - gradient, white and
    their mixes - lies at or above VARIANT in each channel."""
    floor = [v - FRINGE_TOLERANCE for v in VARIANT]
    s = img.width
    return [(i % s, i // s, p) for i, p in enumerate(pixels(img))
            if (p[3] or not visible_only) and any(p[k] < floor[k] for k in range(3))]


def check_ico(path):
    """Problems of the written ICO (empty when it is right): stage order, dark pixels (a
    transparent one bleeds into the edge as soon as the shell scales), AND mask."""
    stages = read_ico(path)
    problems = []
    if [s for s, _, _ in stages] != SIZES:
        problems.append(f'stages {[s for s, _, _ in stages]} instead of {SIZES}')
    for s, img, mask in stages:
        dark = dark_pixels(img)
        if dark:
            problems.append(f'stage {s}: {len(dark)} pixels darker than the plate, e.g. {dark[0]}')
        if mask is not None:
            alpha = img.getchannel('A').load()
            wrong = sum(mask[y][x] != (alpha[x, y] < MASK_ALPHA) for y in range(s) for x in range(s))
            if wrong:
                problems.append(f'stage {s}: {wrong} AND mask bits do not match the alpha')
    return problems


def shell_scale(img, size):
    """Scale as a filter that ignores alpha does (no premultiplication) - the Windows shell
    when it has no stage of the wanted size: transparent pixels mix their RGB into the edge."""
    return Image.merge('RGBA', [c.resize((size, size), Image.BILINEAR) for c in img.split()])


def fringe_sheet(out, ico, before=None):
    """The ICO stages 16 24 32 48 64 96 256 on dark, mid grey and white: 1:1, zoomed, and
    scaled as the shell scales (256 -> 48, 64 -> 60), each zoomed x4; one row per version.
    Prints the dark pixels per stage."""
    shown = [16, 24, 32, 48, 64, 96, 256]
    scaled = [(256, 48), (64, 60)]
    versions = ([('before', before)] if before else []) + [('after', ico)]
    backgrounds = [('dark', (0x20, 0x20, 0x20)), ('grey', (0x80, 0x80, 0x80)), ('white', WHITE)]
    gap, label_w, zoom_to = 16, 120, 192
    font = ImageFont.load_default(size=14)
    rows = []
    for name, path in versions:
        stages = {s: img for s, img, _ in read_ico(path)}
        tiles = [stages[s] for s in shown]
        tiles += [stages[s].resize((s * (zoom_to // s),) * 2, Image.NEAREST) for s in shown if s < 256]
        for src, size in scaled:
            small = shell_scale(stages[src], size)
            tiles += [small, small.resize((size * 4,) * 2, Image.NEAREST)]
            dark = dark_pixels(small, visible_only=True)
            darkest = f', darkest {min(p for _, _, p in dark)}' if dark else ''
            print(f'{name}: {src} scaled to {size} as the shell scales: {len(dark)} visible '
                  f'pixels darker than the plate{darkest}')
        for s in shown:
            visible = dark_pixels(stages[s], visible_only=True)
            hidden = len(dark_pixels(stages[s])) - len(visible)
            print(f'{name}: stage {s}: {len(visible)} visible and {hidden} transparent pixels '
                  f'darker than the plate')
        rows.append((name, tiles))
    width = label_w + sum(t.width + gap for t in rows[0][1]) + gap
    row_h = 256 + 2 * gap
    head_h = 28
    sheet = Image.new('RGB', (width, head_h + row_h * len(rows) * len(backgrounds)), WHITE)
    draw = ImageDraw.Draw(sheet)
    captions = [f'{s}' for s in shown] + [f'{s} x{zoom_to // s}' for s in shown if s < 256]
    captions += [c for src, size in scaled for c in (f'shell {src}>{size}, 1:1 and x4', '')]
    x = label_w
    for caption, tile in zip(captions, rows[0][1]):
        draw.text((x, 6), caption, fill=(0, 0, 0), font=font)
        x += tile.width + gap
    y = head_h
    for bg_name, bg in backgrounds:
        for name, tiles in rows:
            band = Image.new('RGB', (width, row_h), bg)
            ImageDraw.Draw(band).text((gap, gap), f'{name}\n{bg_name}',
                                      fill=WHITE if bg != WHITE else (0, 0, 0), font=font)
            x = label_w
            for tile in tiles:
                band.paste(tile, (x, gap + (256 - tile.height) // 2), tile)
                x += tile.width + gap
            sheet.paste(band, (0, y))
            y += row_h
    sheet.save(out)


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
    icons = root / 'src-tauri' / 'icons'
    args = sys.argv[1:]
    if len(args) == 3 and args[0] == '--compare':
        compare(args[1], args[2])
    elif len(args) in (2, 3) and args[0] == '--fringe-sheet':
        fringe_sheet(args[1], icons / 'icon.ico', args[2] if len(args) == 3 else None)
    elif not args:
        build_ico(icons / 'icon.ico')
        build_mac(icons / 'icon.icns', icons / 'icon.png')
        build_svg(root / 'ui' / 'src' / 'assets' / 'app-icon.svg')
        problems = check_ico(icons / 'icon.ico')
        if problems:
            sys.exit('icon.ico failed its check:\n  ' + '\n  '.join(problems))
    else:
        sys.exit(__doc__)
