# Logo Mixtape : cookie lavande (Material 3 Expressive) + cassette + bande ondulée.
import numpy as np, zlib, struct, sys

def hexc(h): return np.array([int(h[i:i+2], 16) for i in (1, 3, 5)], dtype=np.float32) / 255

DARK = hexc('#2e1a5c'); LAV = hexc('#d0bcff'); LAV_HI = hexc('#ece2ff'); LAV_LO = hexc('#9d84ec'); CONT = hexc('#5a4a86')

def rounded_rect(x, y, x0, y0, x1, y1, r):
    cx = np.clip(x, x0 + r, x1 - r); cy = np.clip(y, y0 + r, y1 - r)
    return ((x - cx) ** 2 + (y - cy) ** 2 <= r * r) & (x >= x0) & (x <= x1) & (y >= y0) & (y <= y1)

def render(n, ss=4, simple=False):
    m = n * ss
    c = (np.arange(m) + 0.5) / m
    x, y = np.meshgrid(c, c)
    rgb = np.zeros((m, m, 3), np.float32); a = np.zeros((m, m), np.float32)
    def paint(mask, color):
        rgb[mask] = color if np.ndim(color) == 1 else color[mask]
        a[mask] = 1.0
    # Cookie à 9 lobes, dégradé diagonal.
    dx, dy = x - 0.5, y - 0.5
    th = np.arctan2(dy, dx); rr = np.hypot(dx, dy)
    amp = 0.055
    R = 0.47 * (1 + amp * np.cos(9 * (th + 0.17))) / (1 + amp)
    t = np.clip((x + y) / 2, 0, 1)[..., None]
    grad = LAV_HI * (1 - t) + LAV_LO * t
    paint(rr <= R, grad)
    # Petites tailles : cassette agrandie (coordonnées zoomées autour du centre).
    if simple:
        x = 0.5 + (x - 0.5) / 1.18
        y = 0.5 + (y - 0.5) / 1.18
    # Ombre douce sous la cassette.
    sh = rounded_rect(x, y, 0.235, 0.345, 0.765, 0.695, 0.075)
    rgb[sh] = rgb[sh] * 0.82
    # Corps de la cassette.
    paint(rounded_rect(x, y, 0.235, 0.325, 0.765, 0.675, 0.075), DARK)
    # Encoche trapézoïdale du bas (tête de lecture).
    yt, yb = 0.598, 0.675
    k = np.clip((y - yt) / (yb - yt), 0, 1)
    half = 0.155 - 0.04 * k
    paint((y >= yt) & (y <= yb) & (np.abs(x - 0.5) <= half), CONT)
    # Fenêtre de bande reliant les bobines.
    paint(rounded_rect(x, y, 0.315, 0.452, 0.685, 0.548, 0.048), CONT)
    # Bobines (moyeu à 6 crans).
    for cx in (0.385, 0.615):
        d = np.hypot(x - cx, y - 0.50)
        paint(d <= 0.062, LAV)
        if not simple:
            ang = np.arctan2(y - 0.50, x - cx)
            hub = d <= 0.024 + 0.006 * (np.cos(6 * ang) > 0.3)
            paint(hub, DARK)
    # Étiquette avec la vague (signature de l'app).
    paint(rounded_rect(x, y, 0.29, 0.36, 0.71, 0.418, 0.029), LAV)
    if not simple:
        x0, x1, yc, am, wl, th2 = 0.33, 0.67, 0.389, 0.009, 0.06, 0.013
        wave_y = yc + am * np.sin((x - x0) / wl * 2 * np.pi)
        band = (np.abs(y - wave_y) <= th2 / 2) & (x >= x0) & (x <= x1)
        for ex in (x0, x1):
            ey = yc + am * np.sin((ex - x0) / wl * 2 * np.pi)
            band |= np.hypot(x - ex, y - ey) <= th2 / 2
        paint(band, DARK)
    # Réduction (moyenne prémultipliée) → anticrénelage.
    pre = rgb * a[..., None]
    pre = pre.reshape(n, ss, n, ss, 3).mean(axis=(1, 3)); al = a.reshape(n, ss, n, ss).mean(axis=(1, 3))
    out = np.zeros((n, n, 4), np.float32)
    nz = al > 0
    out[..., :3][nz] = pre[nz] / al[nz][..., None]
    out[..., 3] = al
    return (np.clip(out, 0, 1) * 255 + 0.5).astype(np.uint8)

def png(img):
    h, w, _ = img.shape
    raw = b''.join(b'\x00' + img[r].tobytes() for r in range(h))
    def chunk(t, d): return struct.pack('>I', len(d)) + t + d + struct.pack('>I', zlib.crc32(t + d) & 0xffffffff)
    return b'\x89PNG\r\n\x1a\n' + chunk(b'IHDR', struct.pack('>IIBBBBB', w, h, 8, 6, 0, 0, 0)) + chunk(b'IDAT', zlib.compress(raw, 9)) + chunk(b'IEND', b'')

out = sys.argv[1]
big = png(render(512)); open(f'{out}/icon.png', 'wb').write(big)
open(f'{out}/logo-256.png', 'wb').write(png(render(256)))
sizes = [16, 20, 24, 32, 40, 48, 64, 128, 256]
imgs = [png(render(s, ss=8 if s <= 48 else 4, simple=s <= 24)) for s in sizes]
ico = struct.pack('<HHH', 0, 1, len(imgs)); off = 6 + 16 * len(imgs)
for s, d in zip(sizes, imgs):
    ico += struct.pack('<BBBBHHII', s % 256, s % 256, 0, 0, 1, 32, len(d), off); off += len(d)
open(f'{out}/icon.ico', 'wb').write(ico + b''.join(imgs))
print('ok')
