#!/usr/bin/env python3
"""Génère la monoplace 3D de F1X (modèle stylisé original, aucune reproduction officielle).

Sortie : un fichier binaire compact partagé par le site (WebGL) et l'app iOS (SceneKit) :

    "F1XC" | u32 version | u32 groupes
    pour chaque groupe :
        u8 nature (0 couleur fixe, 1 peinture, 2 secondaire, 3 accent)
        u8 r, g, b | u8 brillance (0-100) | 3 octets de bourrage
        u32 sommets | u32 indices
        i16 positions[sommets*3]   (mètres × 4096)
        i8  normales[sommets*3]    (× 127)
        u16 indices[indices]

Repère : mètres, avant = +X, haut = +Y, côté droit = +Z. Usage : python3 build.py sortie.bin [...]
"""

import math
import struct
import sys

# ---------------------------------------------------------------- vecteurs

def add(a, b): return (a[0] + b[0], a[1] + b[1], a[2] + b[2])
def sub(a, b): return (a[0] - b[0], a[1] - b[1], a[2] - b[2])
def mul(a, k): return (a[0] * k, a[1] * k, a[2] * k)
def dot(a, b): return a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
def cross(a, b): return (a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0])
def norm(a):
    l = math.sqrt(dot(a, a))
    return (0.0, 1.0, 0.0) if l < 1e-12 else (a[0] / l, a[1] / l, a[2] / l)
def lerp(a, b, t): return a + (b - a) * t


def catmull(points, steps):
    """Courbe lisse passant par les points (Catmull-Rom), `steps` sous-divisions par segment."""
    out = []
    n = len(points)
    for i in range(n - 1):
        p0 = points[max(i - 1, 0)]
        p1, p2 = points[i], points[i + 1]
        p3 = points[min(i + 2, n - 1)]
        for s in range(steps):
            t = s / steps
            t2, t3 = t * t, t * t * t
            out.append(tuple(
                0.5 * ((2 * p1[k]) + (-p0[k] + p2[k]) * t + (2 * p0[k] - 5 * p1[k] + 4 * p2[k] - p3[k]) * t2
                       + (-p0[k] + 3 * p1[k] - 3 * p2[k] + p3[k]) * t3)
                for k in range(len(p1))))
    out.append(tuple(points[-1]))
    return out

# ---------------------------------------------------------------- matériaux

PAINT, SECOND, ACCENT = (1, 0, 0, 0, 100), (2, 0, 0, 0, 100), (3, 0, 0, 0, 100)
def fixed(hexc, gloss):
    return (0, (hexc >> 16) & 255, (hexc >> 8) & 255, hexc & 255, gloss)

CARBON = fixed(0x09090B, 20)
CARBON_GLOSS = fixed(0x16161A, 70)
DARK = fixed(0x050506, 40)
TYRE = fixed(0x0C0C0D, 8)
TYRE_TEXT = fixed(0xEDEDED, 10)
STRIPE = fixed(0xF5C518, 15)
RIM = fixed(0x5A5D63, 90)
VISOR = fixed(0x0A0F1A, 100)
NUT = fixed(0xD81B1B, 70)
MIRROR = fixed(0xAEB8C4, 100)
RAIN = fixed(0xFF2A2A, 10)
TITANIUM = fixed(0x8A8D92, 85)

# ---------------------------------------------------------------- maillage

class Model:
    def __init__(self):
        self.groups = {}  # mat -> [verts, normals, indices]

    def g(self, mat):
        return self.groups.setdefault(mat, [[], [], []])

    def tri(self, mat, a, b, c, na, nb, nc):
        v, n, idx = self.g(mat)
        # Sens trigonométrique vu de l'extérieur.
        if dot(cross(sub(b, a), sub(c, a)), add(add(na, nb), nc)) < 0:
            b, c, nb, nc = c, b, nc, nb
        base = len(v)
        v += [a, b, c]
        n += [na, nb, nc]
        idx += [base, base + 1, base + 2]

    def grid(self, pts, nrm, mats, closed_u=True):
        """Surface en grille pts[i][j] (i le long, j autour) avec normales et matériau par quad."""
        rows, cols = len(pts), len(pts[0])
        # Sommets partagés par groupe de matériau.
        cache = {}
        def vid(mat, i, j):
            key = (mat, i, j % cols)
            if key not in cache:
                v, n, _ = self.g(mat)
                cache[key] = len(v)
                v.append(pts[i][j % cols])
                n.append(nrm[i][j % cols])
            return cache[key]
        jmax = cols if closed_u else cols - 1
        for i in range(rows - 1):
            for j in range(jmax):
                mat = mats(i, j)
                a, b, c, d = vid(mat, i, j), vid(mat, i + 1, j), vid(mat, i + 1, j + 1), vid(mat, i, j + 1)
                _, n, idx = self.g(mat)
                v = self.g(mat)[0]
                face = cross(sub(v[b], v[a]), sub(v[c], v[a]))
                if dot(face, add(n[a], n[c])) < 0:
                    idx += [a, c, b, a, d, c]
                else:
                    idx += [a, b, c, a, c, d]

    def flat_poly(self, mat, pts, normal):
        """Polygone convexe plan (éventail)."""
        c = mul(pts[0], 0)
        for p in pts:
            c = add(c, p)
        c = mul(c, 1 / len(pts))
        for i in range(len(pts)):
            self.tri(mat, c, pts[i], pts[(i + 1) % len(pts)], normal, normal, normal)

    # ----- primitives

    def loft(self, rings, mats, cap=True):
        """Relie des anneaux de points (même nombre) avec normales lissées."""
        rows, cols = len(rings), len(rings[0])
        centres = []
        for r in rings:
            c = (0.0, 0.0, 0.0)
            for p in r:
                c = add(c, p)
            centres.append(mul(c, 1 / cols))
        nrm = []
        for i in range(rows):
            row = []
            for j in range(cols):
                along = sub(rings[min(i + 1, rows - 1)][j], rings[max(i - 1, 0)][j])
                around = sub(rings[i][(j + 1) % cols], rings[i][(j - 1) % cols])
                nn = norm(cross(along, around))
                if dot(nn, sub(rings[i][j], centres[i])) < 0:
                    nn = mul(nn, -1)
                row.append(nn)
            nrm.append(row)
        self.grid(rings, nrm, mats)
        if cap:
            for i, sgn in ((0, -1), (rows - 1, 1)):
                axis = norm(mul(sub(centres[min(i + 1, rows - 1)], centres[max(i - 1, 0)]), sgn))
                m = mats(min(i, rows - 2), 0)
                for j in range(cols):
                    self.tri(m, centres[i], rings[i][j], rings[i][(j + 1) % cols], axis, axis, axis)

    def extrude(self, mat, outline, z0, z1):
        """Plaque : contour (x, y) convexe extrudé entre z0 et z1."""
        a = [(x, y, z0) for x, y in outline]
        b = [(x, y, z1) for x, y in outline]
        nz = 1 if z1 > z0 else -1
        self.flat_poly(mat, a, (0, 0, -nz))
        self.flat_poly(mat, b, (0, 0, nz))
        n = len(outline)
        cx = sum(p[0] for p in outline) / n
        cy = sum(p[1] for p in outline) / n
        for i in range(n):
            j = (i + 1) % n
            e = sub(b[j], a[i])
            nn = norm((outline[j][1] - outline[i][1], -(outline[j][0] - outline[i][0]), 0))
            mid = ((outline[i][0] + outline[j][0]) / 2 - cx, (outline[i][1] + outline[j][1]) / 2 - cy, 0)
            if dot(nn, mid) < 0:
                nn = mul(nn, -1)
            self.tri(mat, a[i], a[j], b[j], nn, nn, nn)
            self.tri(mat, a[i], b[j], b[i], nn, nn, nn)

    def plate_xz(self, mat, outline, y0, y1):
        """Plaque horizontale : contour (x, z) convexe entre y0 et y1."""
        lo = [(x, y0, z) for x, z in outline]
        hi = [(x, y1, z) for x, z in outline]
        self.flat_poly(mat, lo, (0, -1, 0))
        self.flat_poly(mat, hi, (0, 1, 0))
        n = len(outline)
        cx = sum(p[0] for p in outline) / n
        cz = sum(p[1] for p in outline) / n
        for i in range(n):
            j = (i + 1) % n
            nn = norm((outline[j][1] - outline[i][1], 0, -(outline[j][0] - outline[i][0])))
            mid = ((outline[i][0] + outline[j][0]) / 2 - cx, 0, (outline[i][1] + outline[j][1]) / 2 - cz)
            if dot(nn, mid) < 0:
                nn = mul(nn, -1)
            self.tri(mat, lo[i], lo[j], hi[j], nn, nn, nn)
            self.tri(mat, lo[i], hi[j], hi[i], nn, nn, nn)

    def tube(self, mat, pts, r, segs=12, smooth=6):
        path = catmull(pts, smooth) if len(pts) > 2 else pts
        rings = []
        for i in range(len(path)):
            t = norm(sub(path[min(i + 1, len(path) - 1)], path[max(i - 1, 0)]))
            up = (1, 0, 0) if abs(t[1]) > 0.9 else (0, 1, 0)
            s = norm(cross(t, up))
            u = cross(s, t)
            rr = r(i / max(len(path) - 1, 1)) if callable(r) else r
            rings.append([add(path[i], add(mul(s, math.cos(a) * rr), mul(u, math.sin(a) * rr)))
                          for a in [2 * math.pi * k / segs for k in range(segs)]])
        self.loft(rings, lambda i, j: mat)

    def ellipsoid(self, centre, radii, mats, rings_n=14, segs=24):
        rings = []
        for i in range(1, rings_n):
            th = math.pi * i / rings_n
            rings.append([(centre[0] + math.cos(th) * radii[0],
                           centre[1] + math.sin(th) * math.sin(ph) * radii[1],
                           centre[2] + math.sin(th) * math.cos(ph) * radii[2])
                          for ph in [2 * math.pi * k / segs for k in range(segs)]])
        self.loft(rings, mats)

# ---------------------------------------------------------------- sections

def section(x, zc, hw, y0, y1, e_top=2.6, e_bot=4.0, top_w=0.8, segs=40):
    """Anneau en super-ellipse asymétrique : dessus arrondi et plus étroit, dessous plat."""
    yc, hh = (y0 + y1) / 2, (y1 - y0) / 2
    pts = []
    for k in range(segs):
        a = 2 * math.pi * k / segs
        u, v = math.cos(a), math.sin(a)
        e = e_top if v > 0 else e_bot
        sx = math.copysign(abs(u) ** (2 / e), u)
        sy = math.copysign(abs(v) ** (2 / e), v)
        w = hw * (1 + (top_w - 1) * max(sy, 0))
        pts.append((x, yc + sy * hh, zc + sx * w))
    return pts


def body_loft(model, keys, steps, segs, skin, **kw):
    """Carrosserie : sections clés interpolées en spline (lissage dans la longueur)."""
    params = catmull(keys, steps)
    rings = [section(*p[:5], segs=segs, **kw) for p in params]
    xs = [p[0] for p in params]
    rel = [math.sin(2 * math.pi * k / segs) for k in range(segs)]

    def mats(i, j):
        return skin((xs[i] + xs[i + 1]) / 2, (rel[j] + rel[(j + 1) % segs]) / 2)
    model.loft(rings, mats)


def airfoil(le, chord, angle_deg, thick, z, n=14, camber=0.06):
    """Profil d'aile (NACA à cambrure) dans le plan XY, bord d'attaque `le`, incliné vers le bas à l'arrière."""
    a = math.radians(angle_deg)
    upper, lower = [], []
    for k in range(n + 1):
        t = (1 - math.cos(math.pi * k / n)) / 2
        yt = 5 * thick * (0.2969 * math.sqrt(t) - 0.126 * t - 0.3516 * t ** 2 + 0.2843 * t ** 3 - 0.1036 * t ** 4)
        yc = camber * 4 * t * (1 - t)
        # Aile inversée (appui) : cambrure vers le bas.
        upper.append((t, -yc + yt))
        lower.append((t, -yc - yt))
    pts = upper[::-1] + lower[1:-1]
    out = []
    for t, y in pts:
        x0, y0 = -t * chord, y * chord
        out.append((le[0] + x0 * math.cos(a) - y0 * math.sin(a), le[1] - x0 * math.sin(a) + y0 * math.cos(a), z))
    return out


def wing(model, mat, span_params):
    """Aile lissée le long de l'envergure : liste de (z, x_le, y_le, corde, angle, épaisseur)."""
    rings = [airfoil((x, y), c, ang, th, z) for z, x, y, c, ang, th in span_params]
    model.loft(rings, lambda i, j: mat)

# ---------------------------------------------------------------- monoplace

def build():
    m = Model()

    def body_skin(x, rel):
        if rel < -0.3:
            return SECOND
        if 0.08 < rel < 0.30 and x > 0.55:
            return ACCENT  # bande sur les flancs du nez
        return PAINT

    # Nez, monocoque et capot moteur (sections clés).
    body = [
        (2.80, 0.0, 0.045, 0.175, 0.235),
        (2.62, 0.0, 0.085, 0.150, 0.285),
        (2.32, 0.0, 0.125, 0.135, 0.355),
        (1.96, 0.0, 0.172, 0.128, 0.428),
        (1.56, 0.0, 0.228, 0.118, 0.505),
        (1.16, 0.0, 0.288, 0.108, 0.578),
        (0.80, 0.0, 0.340, 0.100, 0.628),
        (0.45, 0.0, 0.385, 0.095, 0.655),
        (0.10, 0.0, 0.408, 0.090, 0.668),
        (-0.22, 0.0, 0.398, 0.090, 0.702),
        (-0.58, 0.0, 0.338, 0.100, 0.752),
        (-0.98, 0.0, 0.258, 0.118, 0.676),
        (-1.38, 0.0, 0.190, 0.138, 0.560),
        (-1.76, 0.0, 0.130, 0.158, 0.458),
        (-2.06, 0.0, 0.084, 0.178, 0.388),
        (-2.24, 0.0, 0.048, 0.198, 0.330),
    ]
    body_loft(m, body, 6, 44, body_skin, e_top=2.5, e_bot=4.5, top_w=0.78)

    # Prise d'air au-dessus du pilote (arceau) et capot.
    airbox = [
        (0.02, 0.0, 0.085, 0.655, 0.925),
        (-0.12, 0.0, 0.128, 0.640, 0.995),
        (-0.36, 0.0, 0.158, 0.620, 0.972),
        (-0.72, 0.0, 0.150, 0.600, 0.860),
        (-1.10, 0.0, 0.092, 0.580, 0.720),
        (-1.42, 0.0, 0.040, 0.570, 0.630),
    ]
    body_loft(m, airbox, 6, 32, lambda x, r: PAINT, e_top=2.2, e_bot=3.0, top_w=0.7)
    # Entrée d'air (ovale sombre) et caméra embarquée.
    m.ellipsoid((0.025, 0.86, 0.0), (0.012, 0.075, 0.07), lambda i, j: DARK, 8, 16)
    m.plate_xz(ACCENT, [(-0.04, -0.05), (-0.04, 0.05), (-0.16, 0.05), (-0.16, -0.05)], 0.985, 1.035)
    # Aileron de requin (aux couleurs de l'accent).
    m.extrude(PAINT, [(-0.62, 0.93), (-0.62, 0.86), (-1.30, 0.66), (-2.04, 0.60), (-2.04, 0.74)], -0.005, 0.005)

    # Habitacle : ouverture sombre, appuie-tête, pilote (casque et visière).
    m.ellipsoid((0.36, 0.656, 0.0), (0.40, 0.028, 0.205), lambda i, j: DARK, 10, 28)
    for s in (-1, 1):
        m.tube(SECOND, [(0.05, 0.67, s * 0.20), (0.25, 0.675, s * 0.215), (0.5, 0.67, s * 0.20)], 0.035, 10, 4)
    helmet_c, helmet_r = (0.21, 0.745, 0.0), (0.145, 0.138, 0.124)
    def helmet(i, j):
        th = math.pi * (i + 1.5) / 16
        ph = 2 * math.pi * (j + 0.5) / 28
        nx, ny = math.cos(th), math.sin(th) * math.sin(ph)
        if nx > 0.35 and -0.15 < ny < 0.45:
            return VISOR
        if ny > 0.72:
            return PAINT
        return ACCENT
    m.ellipsoid(helmet_c, helmet_r, helmet, 16, 28)

    # Pontons (avec entrées d'air) : sous-coupe à l'avant, rampe vers l'arrière.
    def pod_skin(x, rel):
        if rel < -0.25:
            return SECOND
        if 0.45 < rel < 0.68 and x > -0.9:
            return ACCENT  # bande d'accent sur l'épaule du ponton
        return PAINT
    for s in (-1, 1):
        pods = [
            (1.02, s * 0.535, 0.095, 0.270, 0.470),
            (0.88, s * 0.585, 0.185, 0.205, 0.555),
            (0.58, s * 0.605, 0.222, 0.150, 0.580),
            (0.18, s * 0.585, 0.212, 0.128, 0.548),
            (-0.32, s * 0.522, 0.180, 0.120, 0.470),
            (-0.82, s * 0.432, 0.130, 0.128, 0.370),
            (-1.30, s * 0.335, 0.082, 0.140, 0.272),
            (-1.62, s * 0.268, 0.048, 0.150, 0.222),
        ]
        body_loft(m, pods, 6, 32, pod_skin, e_top=2.6, e_bot=5.0, top_w=0.72)
        # Entrée d'air sombre.
        m.ellipsoid((1.02, 0.37, s * 0.535), (0.012, 0.085, 0.078), lambda i, j: DARK, 8, 16)
        # Rétroviseur sur bras.
        m.tube(CARBON, [(0.62, 0.60, s * 0.37), (0.60, 0.66, s * 0.44), (0.58, 0.70, s * 0.50)], 0.011, 8, 3)
        mirror = [section(x, s * 0.52, hw, 0.665, 0.765, 2.4, 2.4, 1.0, 20)
                  for x, hw in ((0.64, 0.045), (0.62, 0.075), (0.56, 0.08), (0.53, 0.07))]
        m.loft(mirror, lambda i, j: PAINT)
        m.extrude(MIRROR, [(0.528, 0.675), (0.528, 0.755), (0.531, 0.755), (0.531, 0.675)], s * 0.465, s * 0.575)

    # Halo (titane, tube lissé) et pilier central.
    hoop = [(-0.16, 0.655, -0.275), (-0.08, 0.80, -0.285), (0.10, 0.87, -0.29), (0.38, 0.885, -0.24),
            (0.58, 0.88, -0.11), (0.64, 0.878, 0.0), (0.58, 0.88, 0.11), (0.38, 0.885, 0.24),
            (0.10, 0.87, 0.29), (-0.08, 0.80, 0.285), (-0.16, 0.655, 0.275)]
    m.tube(SECOND, hoop, 0.026, 12, 6)
    m.tube(SECOND, [(0.63, 0.875, 0.0), (0.78, 0.78, 0.0), (0.92, 0.66, 0.0), (1.0, 0.585, 0.0)],
           lambda t: 0.03 + 0.012 * t, 12, 4)

    # Fond plat, bords du fond et diffuseur avec ailettes.
    floor = [(1.55, -0.26), (1.55, 0.26), (1.18, 0.62), (0.92, 0.80), (-1.58, 0.80), (-1.95, 0.70),
             (-1.95, -0.70), (-1.58, -0.80), (0.92, -0.80), (1.18, -0.62)]
    m.plate_xz(CARBON, floor, 0.030, 0.058)
    for s in (-1, 1):
        m.extrude(CARBON, [(0.90, 0.058), (0.90, 0.085), (-1.10, 0.13), (-1.55, 0.12), (-1.55, 0.058)], s * 0.785, s * 0.80)
        m.tube(SECOND, [(0.70, 0.07, s * 0.79), (-0.30, 0.10, s * 0.79), (-1.30, 0.115, s * 0.79)], 0.012, 8, 4)
    m.extrude(CARBON, [(-1.60, 0.03), (-1.60, 0.10), (-2.46, 0.33), (-2.46, 0.06)], -0.56, 0.56)
    for z in (-0.40, -0.14, 0.14, 0.40):
        m.extrude(CARBON, [(-1.70, 0.06), (-2.46, 0.06), (-2.46, 0.33), (-1.70, 0.12)], z - 0.006, z + 0.006)

    # Aileron avant : plan principal continu, trois volets cambrés qui se relèvent vers l'extérieur.
    def span(z_in, z_out, f, n=10):
        out = []
        for k in range(n + 1):
            t = k / n
            z = lerp(z_in, z_out, t)
            out.append((z,) + f(t))
        return out
    wing(m, CARBON_GLOSS, span(-0.935, 0.935, lambda t: (2.99, 0.085 + 0.012 * abs(2 * t - 1), 0.43 - 0.06 * abs(2 * t - 1), 5, 0.10), 16))
    for s in (-1, 1):
        def side(f):
            return span(s * 0.21, s * 0.925, f)
        wing(m, PAINT, side(lambda t: (2.66 - 0.05 * t, 0.120 + 0.035 * t, 0.27 - 0.05 * t, 16 + 6 * t, 0.10)))
        wing(m, SECOND, side(lambda t: (2.50 - 0.04 * t, 0.170 + 0.060 * t, 0.21 - 0.03 * t, 26 + 6 * t, 0.10)))
        wing(m, ACCENT, side(lambda t: (2.38 - 0.03 * t, 0.225 + 0.090 * t, 0.16 - 0.02 * t, 36 + 6 * t, 0.10)))
        # Dérive courbée.
        m.extrude(PAINT, [(3.00, 0.05), (3.00, 0.16), (2.80, 0.27), (2.50, 0.34), (2.22, 0.36), (2.20, 0.06)],
                  s * 0.925, s * 0.94)
        # Pilier nez → aileron.
        m.extrude(CARBON, [(2.70, 0.10), (2.55, 0.10), (2.50, 0.15), (2.66, 0.16)], s * 0.07 - 0.006, s * 0.07 + 0.006)

    # Aileron arrière « cuillère » : plan principal, volet DRS, beam wing, dérives, col de cygne.
    wing(m, CARBON_GLOSS, span(-0.505, 0.505, lambda t: (-2.12, 0.80 - 0.025 * (1 - abs(2 * t - 1)), 0.36 + 0.06 * (1 - abs(2 * t - 1)), 10, 0.12), 12))
    wing(m, ACCENT, span(-0.505, 0.505, lambda t: (-2.30, 0.905, 0.23, 30, 0.10), 6))
    wing(m, CARBON, span(-0.43, 0.43, lambda t: (-2.20, 0.43, 0.24, 14, 0.12), 6))
    for s in (-1, 1):
        m.extrude(PAINT, [(-2.00, 0.34), (-2.00, 0.96), (-2.12, 1.01), (-2.62, 1.01), (-2.68, 0.60), (-2.56, 0.30)],
                  s * 0.505, s * 0.525)
        m.tube(CARBON, [(-1.98, 0.62, s * 0.04), (-2.10, 0.86, s * 0.04), (-2.22, 0.99, s * 0.04), (-2.36, 0.985, s * 0.04)], 0.018, 8, 4)
    m.extrude(RAIN, [(-2.66, 0.30), (-2.66, 0.37), (-2.70, 0.37), (-2.70, 0.30)], -0.05, 0.05)
    m.tube(TITANIUM, [(-2.10, 0.42, 0.0), (-2.30, 0.43, 0.0)], 0.045, 14, 1)

    # Roues : pneu bombé, liserés, jante, enjoliveur, écrou ; disque de frein derrière.
    def wheel(c, r, w):
        hw, ri = w / 2, r * 0.70
        prof = []
        prof.append((ri, -hw))
        for k in range(9):
            f = math.pi / 2 * k / 8
            prof.append((r - 0.07 + 0.07 * math.sin(f), -hw + 0.07 - 0.07 * math.cos(f) - 0.012 * math.sin(f * 2)))
        for k in range(9):
            f = math.pi / 2 + math.pi / 2 * k / 8
            prof.append((r - 0.07 + 0.07 * math.sin(f), hw - 0.07 - 0.07 * math.cos(f) + 0.012 * math.sin(f * 2 - math.pi)))
        prof.append((ri, hw))
        segs = 56
        rings = []
        for rad, z in prof:
            rings.append([(c[0] + math.cos(a) * rad, c[1] + math.sin(a) * rad, c[2] + z)
                          for a in [2 * math.pi * k / segs for k in range(segs)]])
        m.loft(rings, lambda i, j: TYRE, cap=False)

        def disc(r0, r1, z, mat, nz, segs=56):
            ring0 = [(c[0] + math.cos(a) * r0, c[1] + math.sin(a) * r0, c[2] + z) for a in [2 * math.pi * k / segs for k in range(segs)]]
            ring1 = [(c[0] + math.cos(a) * r1, c[1] + math.sin(a) * r1, c[2] + z) for a in [2 * math.pi * k / segs for k in range(segs)]]
            n = (0, 0, nz)
            m.grid([ring0, ring1], [[n] * segs, [n] * segs], lambda i, j: mat)

        for sd in (-1, 1):
            zs = sd * (hw + 0.0015)
            disc(r * 0.80, r * 0.835, zs, STRIPE, sd)
            disc(r * 0.88, r * 0.90, zs, TYRE_TEXT, sd)
            zr = sd * (hw - 0.03)
            disc(ri, r * 0.64, zr, RIM, sd)
            disc(r * 0.64, r * 0.56, zr - sd * 0.004, ACCENT, sd)
            disc(r * 0.56, r * 0.15, zr - sd * 0.008, CARBON_GLOSS, sd)
            disc(r * 0.15, 0.0, sd * (hw - 0.012), NUT, sd)
            # Lèvre de jante.
            ring_a = [(c[0] + math.cos(a) * ri, c[1] + math.sin(a) * ri, c[2] + sd * hw) for a in [2 * math.pi * k / segs for k in range(segs)]]
            ring_b = [(c[0] + math.cos(a) * ri, c[1] + math.sin(a) * ri, c[2] + zr) for a in [2 * math.pi * k / segs for k in range(segs)]]
            inward = [(-math.cos(a), -math.sin(a), 0) for a in [2 * math.pi * k / segs for k in range(segs)]]
            m.grid([ring_a, ring_b], [inward, inward], lambda i, j: RIM)

    for s in (-1, 1):
        wheel((1.75, 0.36, s * 0.835), 0.36, 0.30)
        wheel((-1.85, 0.36, s * 0.795), 0.365, 0.40)
        # Suspensions (bras profilés) : avant à poussoir, arrière à tirant.
        for a, b in (((1.42, 0.47, s * 0.22), (1.77, 0.44, s * 0.69)), ((2.02, 0.44, s * 0.20), (1.77, 0.44, s * 0.69)),
                     ((1.40, 0.21, s * 0.24), (1.74, 0.25, s * 0.69)), ((2.04, 0.19, s * 0.21), (1.74, 0.25, s * 0.69)),
                     ((1.62, 0.52, s * 0.25), (1.74, 0.27, s * 0.66)), ((2.06, 0.32, s * 0.22), (1.80, 0.33, s * 0.69)),
                     ((-1.50, 0.47, s * 0.26), (-1.84, 0.46, s * 0.61)), ((-2.00, 0.43, s * 0.22), (-1.84, 0.46, s * 0.61)),
                     ((-1.52, 0.20, s * 0.30), (-1.86, 0.24, s * 0.61)), ((-2.02, 0.18, s * 0.26), (-1.86, 0.24, s * 0.61)),
                     ((-1.70, 0.18, s * 0.27), (-1.84, 0.44, s * 0.58))):
            m.tube(CARBON, [a, b], 0.017, 8, 1)
        # Porte-moyeu et écope de frein.
        m.ellipsoid((1.75, 0.36, s * 0.66), (0.10, 0.12, 0.05), lambda i, j: CARBON, 8, 16)
        m.ellipsoid((-1.85, 0.36, s * 0.58), (0.12, 0.13, 0.06), lambda i, j: CARBON, 8, 16)
    return m


def export(model, path):
    order = [PAINT, SECOND, ACCENT] + sorted(k for k in model.groups if k[0] == 0)
    groups = [(k, model.groups[k]) for k in order if k in model.groups]
    out = bytearray(b"F1XC")
    out += struct.pack("<II", 1, len(groups))
    total_v = total_i = 0
    for (kind, r, g, b, gloss), (verts, nrms, idx) in groups:
        assert len(verts) < 65536, f"groupe trop grand : {len(verts)}"
        out += struct.pack("<BBBBBxxx", kind, r, g, b, gloss)
        out += struct.pack("<II", len(verts), len(idx))
        for v in verts:
            out += struct.pack("<hhh", *(max(-32767, min(32767, round(c * 4096))) for c in v))
        for n in nrms:
            out += struct.pack("<bbb", *(max(-127, min(127, round(c * 127))) for c in n))
        out += struct.pack(f"<{len(idx)}H", *idx)
        total_v += len(verts)
        total_i += len(idx)
    with open(path, "wb") as f:
        f.write(out)
    return total_v, total_i // 3, len(out)


if __name__ == "__main__":
    model = build()
    for p in sys.argv[1:] or ["car.bin"]:
        v, t, size = export(model, p)
        print(f"{p}: {v} sommets, {t} triangles, {size / 1024:.0f} Ko")
