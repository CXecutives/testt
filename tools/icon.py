"""App-Symbol (src-tauri/icons/icon.ico): Koralle mit weißer Projektmappe und Haken.

Jede Stufe liegt auf dem Pixelraster: gerade Kanten auf ganzen Pixeln, Ränder symmetrisch,
gerendert mit 16-facher Überabtastung und Flächenmittel – dadurch bleiben sie scharf.

Die Reihenfolge im ICO ist wichtig: Tauri nimmt für das Fenstersymbol (Taskleiste, Alt+Tab)
stur den ersten Eintrag. Er ist deshalb die 48er-Stufe – Windows rechnet daraus sauber
herunter, statt eine kleine Stufe aufzublasen.

    python tools/icon.py            (braucht Pillow)
"""
import struct
from io import BytesIO
from pathlib import Path

from PIL import Image, ImageDraw

SS = 16
GLOW = (0xE9, 0x8C, 0x72)     # hsl(13 73% 68%)
VARIANT = (0xD7, 0x66, 0x47)  # hsl(13 64% 56%)
CHECK = (0xD0, 0x5E, 0x3F)    # etwas tiefer als VARIANT: Kontrast auf Weiß
WHITE = (255, 255, 255)

# Vorlage im 1024er-Raster: Platte 48…976, Mappe 224…800 × 336…752 (Reiter ab 272,
# Reiterende 436), Haken mit 68 Strichbreite.
PLATE, RADIUS = 48, 212
BODY = (224, 336, 800, 752)
TAB_Y, TAB_X, BODY_R = 272, 436, 60
CHECK_POINTS, CHECK_W = [(390, 548), (482, 640), (652, 468)], 68
SIZES = [48, 16, 20, 24, 32, 40, 64, 96, 256]


def layout(s):
    """Maße einer Stufe in Zielpixeln – waagerecht und senkrecht symmetrisch gerundet."""
    k = s / 1024
    margin = max(1, round(PLATE * k))
    plate = s - 2 * margin
    side = round((BODY[0] - PLATE) / (1024 - 2 * PLATE) * plate)
    top = round((BODY[1] - PLATE) / (1024 - 2 * PLATE) * plate)
    tab = round((TAB_Y - PLATE) / (1024 - 2 * PLATE) * plate)
    body = (margin + side, margin + top, s - margin - side, s - margin - tab)
    tab_x = margin + round((TAB_X - PLATE) / (1024 - 2 * PLATE) * plate)

    def point(x, y):
        """Hakenpunkt: gleiche Lage im Mappenkörper wie in der Vorlage."""
        return (body[0] + (x - BODY[0]) / (BODY[2] - BODY[0]) * (body[2] - body[0]),
                body[1] + (y - BODY[1]) / (BODY[3] - BODY[1]) * (body[3] - body[1]))

    return dict(
        bg=(margin, margin, s - margin, s - margin, RADIUS * k),
        tab_y=margin + tab,
        tab_x=tab_x,
        body=(*body, max(1, BODY_R * k)),
        check=[point(x, y) for x, y in CHECK_POINTS],
        w=max(1.7, CHECK_W * k + 0.6),
    )


def gradient(size):
    g = Image.new('RGB', (size, size))
    px = g.load()
    for y in range(size):
        for x in range(size):
            t = (x + y) / (2 * (size - 1))
            px[x, y] = tuple(round(a + (b - a) * t) for a, b in zip(GLOW, VARIANT))
    return g


def render(s):
    L = layout(s)
    S = s * SS
    sc = lambda v: v * SS
    img = Image.new('RGBA', (S, S), (0, 0, 0, 0))
    mask = Image.new('L', (S, S), 0)
    x0, y0, x1, y1, r = L['bg']
    ImageDraw.Draw(mask).rounded_rectangle([sc(x0), sc(y0), sc(x1) - 1, sc(y1) - 1], radius=sc(r), fill=255)
    img.paste(gradient(S), (0, 0), mask)
    d = ImageDraw.Draw(img)
    bx0, by0, bx1, by1, br = L['body']
    # Reiter: vom linken Rand bis tab_x, dann schräg hinunter zur Körperoberkante
    slant = by0 - L['tab_y']
    d.rounded_rectangle([sc(bx0), sc(L['tab_y']), sc(L['tab_x']) - 1, sc(by0 + br) - 1],
                        radius=sc(min(br, slant)), fill=WHITE, corners=(True, False, False, False))
    d.polygon([(sc(L['tab_x']) - 1, sc(L['tab_y'])), (sc(L['tab_x'] + slant), sc(by0)),
               (sc(L['tab_x'] + slant), sc(by0 + br)), (sc(L['tab_x']) - 1, sc(by0 + br))], fill=WHITE)
    d.rounded_rectangle([sc(bx0), sc(by0), sc(bx1) - 1, sc(by1) - 1], radius=sc(br), fill=WHITE)
    pts = [(sc(x), sc(y)) for x, y in L['check']]
    w = sc(L['w'])
    d.line(pts, fill=CHECK, width=round(w), joint='curve')
    for x, y in (pts[0], pts[-1]):
        d.ellipse([x - w / 2, y - w / 2, x + w / 2, y + w / 2], fill=CHECK)
    return img.resize((s, s), Image.BOX)


def dib(img):
    """Stufe als unkomprimiertes DIB: BITMAPINFOHEADER, BGRA von unten, dann UND-Maske.

    Nur die 256er-Stufe wird als PNG abgelegt: Werkzeuge, die die ICO-Datei auslesen
    (auch ältere Windows-Bildbibliotheken), können komprimierte Stufen darunter nicht
    immer entpacken – Windows selbst zeigt dann nichts an, ohne einen Fehler zu melden.

    Die UND-Maske wird aus dem Alphakanal abgeleitet, nicht mit Nullen gefüllt: Wer nur
    die Maske liest (1-Bit-Anzeigen, Cursor-Umwandlung), bekäme sonst ein Symbol mit
    undurchsichtigem Rand.
    """
    s = img.width
    head = struct.pack('<IiiHHIIiiII', 40, s, s * 2, 1, 32, 0, 0, 0, 0, 0, 0)
    px = img.load()
    rows = list(reversed(range(s)))  # DIB-Zeilen laufen von unten nach oben
    xor = b''.join(bytes(v for x in range(s) for v in
                         (lambda p: (p[2], p[1], p[0], p[3]))(px[x, y]))
                   for y in rows)
    stride = ((s + 31) // 32) * 4  # 1 Bit je Pixel, Zeilen auf 4 Byte aufgefüllt
    mask = bytearray()
    for y in rows:
        row = bytearray(stride)
        for x in range(s):
            if px[x, y][3] == 0:  # gesetztes Bit = durchsichtig
                row[x >> 3] |= 0x80 >> (x & 7)
        mask += row
    return head + xor + bytes(mask)


def build(out):
    """ICO selbst schreiben: Verzeichnis in der Reihenfolge von SIZES."""
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


if __name__ == '__main__':
    build(Path(__file__).resolve().parent.parent / 'src-tauri' / 'icons' / 'icon.ico')
