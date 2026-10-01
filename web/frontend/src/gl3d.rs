//! Rendu 3D (WebGL2) écrit en Rust : monoplaces stylisées générées par le code (aucun modèle
//! officiel) et circuits en relief reconstitués à partir des positions GPS OpenF1.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use f1x_protocol::TrackMap;
use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::*;
use web_sys::{
    Element, HtmlCanvasElement, HtmlElement, WebGl2RenderingContext as Gl, WebGlProgram,
    WebGlUniformLocation, WebGlVertexArrayObject,
};
use yew::prelude::*;

use crate::i18n::t;

// ---------- Mathématiques ----------

type V3 = [f32; 3];

fn add(a: V3, b: V3) -> V3 {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}
fn sub(a: V3, b: V3) -> V3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn mul(a: V3, k: f32) -> V3 {
    [a[0] * k, a[1] * k, a[2] * k]
}
fn dot(a: V3, b: V3) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn cross(a: V3, b: V3) -> V3 {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}
fn norm(a: V3) -> V3 {
    let l = dot(a, a).sqrt();
    if l < 1e-9 {
        [0.0, 1.0, 0.0]
    } else {
        mul(a, 1.0 / l)
    }
}
fn lerp3(a: V3, b: V3, k: f32) -> V3 {
    add(a, mul(sub(b, a), k))
}

/// Matrice 4×4 en colonnes (convention WebGL).
#[derive(Clone, Copy)]
struct M4([f32; 16]);

impl M4 {
    fn mul(&self, o: &M4) -> M4 {
        let (a, b) = (&self.0, &o.0);
        let mut r = [0.0; 16];
        for c in 0..4 {
            for row in 0..4 {
                r[c * 4 + row] = (0..4).map(|k| a[k * 4 + row] * b[c * 4 + k]).sum();
            }
        }
        M4(r)
    }

    #[rustfmt::skip]
    fn perspective(fovy: f32, aspect: f32, near: f32, far: f32) -> M4 {
        let f = 1.0 / (fovy / 2.0).tan();
        let nf = 1.0 / (near - far);
        M4([
            f / aspect, 0.0, 0.0, 0.0,
            0.0, f, 0.0, 0.0,
            0.0, 0.0, (far + near) * nf, -1.0,
            0.0, 0.0, 2.0 * far * near * nf, 0.0,
        ])
    }

    #[rustfmt::skip]
    fn look_at(eye: V3, target: V3, up: V3) -> M4 {
        let f = norm(sub(target, eye));
        let s = norm(cross(f, up));
        let u = cross(s, f);
        M4([
            s[0], u[0], -f[0], 0.0,
            s[1], u[1], -f[1], 0.0,
            s[2], u[2], -f[2], 0.0,
            -dot(s, eye), -dot(u, eye), dot(f, eye), 1.0,
        ])
    }

    /// Translation × rotation autour de Y × échelle uniforme.
    #[rustfmt::skip]
    fn trs(pos: V3, yaw: f32, scale: f32) -> M4 {
        let (s, c) = yaw.sin_cos();
        M4([
            c * scale, 0.0, -s * scale, 0.0,
            0.0, scale, 0.0, 0.0,
            s * scale, 0.0, c * scale, 0.0,
            pos[0], pos[1], pos[2], 1.0,
        ])
    }

    fn identity() -> M4 {
        M4::trs([0.0; 3], 0.0, 1.0)
    }

    fn project(&self, p: V3) -> Option<(f32, f32)> {
        let m = &self.0;
        let x = m[0] * p[0] + m[4] * p[1] + m[8] * p[2] + m[12];
        let y = m[1] * p[0] + m[5] * p[1] + m[9] * p[2] + m[13];
        let w = m[3] * p[0] + m[7] * p[1] + m[11] * p[2] + m[15];
        (w > 0.01).then(|| (x / w, y / w))
    }
}

// ---------- Maillages ----------

/// Nature d'un sommet : couleur fixe, peinture de l'écurie, accent, ou non éclairé.
const FIXED: f32 = 0.0;
const PAINT: f32 = 1.0;
const ACCENT: f32 = 2.0;
const UNLIT: f32 = 3.0;

/// Position (3) + normale (3) + couleur RGBA (4) + nature (1).
const STRIDE: usize = 11;

type Rgba = [f32; 4];

const CARBON: Rgba = [0.07, 0.07, 0.08, 1.0];
const DARK: Rgba = [0.02, 0.02, 0.02, 1.0];
const TYRE: Rgba = [0.05, 0.05, 0.055, 1.0];
const RIM: Rgba = [0.42, 0.43, 0.46, 1.0];
const STRIPE: Rgba = [0.98, 0.82, 0.1, 1.0];
const WHITE: Rgba = [1.0, 1.0, 1.0, 1.0];
/// Couleur remplacée par la peinture ou l'accent de l'écurie.
const P: Rgba = WHITE;

#[derive(Default)]
struct Mesh {
    v: Vec<f32>,
}

impl Mesh {
    fn vert(&mut self, p: V3, n: V3, c: Rgba, k: f32) {
        self.v.extend_from_slice(&p);
        self.v.extend_from_slice(&n);
        self.v.extend_from_slice(&c);
        self.v.push(k);
    }

    fn count(&self) -> i32 {
        (self.v.len() / STRIDE) as i32
    }

    /// Face plane (3 ou 4 sommets) dont la normale est orientée à l'opposé de `inside`.
    fn face(&mut self, pts: &[V3], inside: V3, c: Rgba, k: f32) {
        let mut n = norm(cross(sub(pts[1], pts[0]), sub(pts[2], pts[0])));
        let centre = mul(
            pts.iter().fold([0.0; 3], |a, p| add(a, *p)),
            1.0 / pts.len() as f32,
        );
        if dot(n, sub(centre, inside)) < 0.0 {
            n = mul(n, -1.0);
        }
        for i in 1..pts.len() - 1 {
            for p in [pts[0], pts[i], pts[i + 1]] {
                self.vert(p, n, c, k);
            }
        }
    }

    /// Hexaèdre : quatre coins d'une extrémité puis les quatre de l'autre, dans le même ordre.
    fn hexa(&mut self, p: [V3; 8], c: Rgba, k: f32) {
        let inside = mul(p.iter().fold([0.0; 3], |a, q| add(a, *q)), 1.0 / 8.0);
        self.face(&[p[0], p[1], p[2], p[3]], inside, c, k);
        self.face(&[p[4], p[5], p[6], p[7]], inside, c, k);
        for i in 0..4 {
            let j = (i + 1) % 4;
            self.face(&[p[i], p[j], p[j + 4], p[i + 4]], inside, c, k);
        }
    }

    fn bx(&mut self, min: V3, max: V3, c: Rgba, k: f32) {
        self.loft(
            min[0],
            Sec::new(
                (min[2] + max[2]) / 2.0,
                (max[2] - min[2]) / 2.0,
                min[1],
                max[1],
            ),
            max[0],
            Sec::new(
                (min[2] + max[2]) / 2.0,
                (max[2] - min[2]) / 2.0,
                min[1],
                max[1],
            ),
            c,
            k,
        );
    }

    /// Volume entre deux sections rectangulaires perpendiculaires à l'axe X.
    fn loft(&mut self, x0: f32, a: Sec, x1: f32, b: Sec, c: Rgba, k: f32) {
        let ring = |x: f32, s: Sec| {
            [
                [x, s.y0, s.zc - s.zh],
                [x, s.y0, s.zc + s.zh],
                [x, s.y1, s.zc + s.zh],
                [x, s.y1, s.zc - s.zh],
            ]
        };
        let (r0, r1) = (ring(x0, a), ring(x1, b));
        self.hexa(
            [r0[0], r0[1], r0[2], r0[3], r1[0], r1[1], r1[2], r1[3]],
            c,
            k,
        );
    }

    /// Poutre de section carrée entre deux points (bras de suspension, halo).
    fn beam(&mut self, a: V3, b: V3, r: f32, c: Rgba, k: f32) {
        let d = norm(sub(b, a));
        let up = if d[1].abs() > 0.9 {
            [1.0, 0.0, 0.0]
        } else {
            [0.0, 1.0, 0.0]
        };
        let s = mul(norm(cross(d, up)), r);
        let u = mul(norm(cross(s, d)), r);
        let corner = |p: V3, i: usize| match i {
            0 => sub(sub(p, s), u),
            1 => sub(add(p, s), u),
            2 => add(add(p, s), u),
            _ => add(sub(p, s), u),
        };
        self.hexa(
            [
                corner(a, 0),
                corner(a, 1),
                corner(a, 2),
                corner(a, 3),
                corner(b, 0),
                corner(b, 1),
                corner(b, 2),
                corner(b, 3),
            ],
            c,
            k,
        );
    }

    fn sphere(&mut self, centre: V3, r: f32, c: Rgba, k: f32) {
        let (rings, segs) = (8, 14);
        let at = |i: usize, j: usize| {
            let th = std::f32::consts::PI * i as f32 / rings as f32;
            let ph = std::f32::consts::TAU * j as f32 / segs as f32;
            [th.sin() * ph.cos(), th.cos(), th.sin() * ph.sin()]
        };
        for i in 0..rings {
            for j in 0..segs {
                let q = [at(i, j), at(i + 1, j), at(i + 1, j + 1), at(i, j + 1)];
                for idx in [0, 1, 2, 0, 2, 3] {
                    self.vert(add(centre, mul(q[idx], r)), q[idx], c, k);
                }
            }
        }
    }

    /// Roue d'axe Z : bande de roulement, flanc, liseré de gomme et jante.
    fn wheel(&mut self, centre: V3, r: f32, width: f32) {
        let segs = 22;
        let hw = width / 2.0;
        let pt = |a: f32, rad: f32, z: f32| {
            [
                centre[0] + a.cos() * rad,
                centre[1] + a.sin() * rad,
                centre[2] + z,
            ]
        };
        for i in 0..segs {
            let (a0, a1) = (
                std::f32::consts::TAU * i as f32 / segs as f32,
                std::f32::consts::TAU * (i + 1) as f32 / segs as f32,
            );
            let (n0, n1) = ([a0.cos(), a0.sin(), 0.0], [a1.cos(), a1.sin(), 0.0]);
            // Bande de roulement (normales lissées).
            let q = [pt(a0, r, -hw), pt(a1, r, -hw), pt(a1, r, hw), pt(a0, r, hw)];
            let n = [n0, n1, n1, n0];
            for idx in [0, 1, 2, 0, 2, 3] {
                self.vert(q[idx], n[idx], TYRE, FIXED);
            }
            for side in [-1.0f32, 1.0] {
                let z = side * hw;
                let nz = [0.0, 0.0, side];
                for (r0, r1, c) in [
                    (r, r * 0.80, TYRE),
                    (r * 0.80, r * 0.74, STRIPE),
                    (r * 0.74, r * 0.66, TYRE),
                    (r * 0.66, r * 0.12, RIM),
                ] {
                    let q = [pt(a0, r0, z), pt(a1, r0, z), pt(a1, r1, z), pt(a0, r1, z)];
                    for idx in [0, 1, 2, 0, 2, 3] {
                        self.vert(q[idx], nz, c, FIXED);
                    }
                }
            }
        }
    }

    /// Ombre douce elliptique au sol (dégradé de transparence).
    fn shadow(&mut self, centre: V3, rx: f32, rz: f32) {
        let segs = 32;
        let up = [0.0, 1.0, 0.0];
        for i in 0..segs {
            let a0 = std::f32::consts::TAU * i as f32 / segs as f32;
            let a1 = std::f32::consts::TAU * (i + 1) as f32 / segs as f32;
            let p = |a: f32, k: f32| {
                [
                    centre[0] + a.cos() * rx * k,
                    centre[1],
                    centre[2] + a.sin() * rz * k,
                ]
            };
            let (inner, outer) = ([0.0, 0.0, 0.0, 0.55], [0.0, 0.0, 0.0, 0.0]);
            self.vert(centre, up, inner, UNLIT);
            self.vert(p(a0, 0.6), up, inner, UNLIT);
            self.vert(p(a1, 0.6), up, inner, UNLIT);
            for (q, c) in [
                (p(a0, 0.6), inner),
                (p(a0, 1.0), outer),
                (p(a1, 1.0), outer),
                (p(a0, 0.6), inner),
                (p(a1, 1.0), outer),
                (p(a1, 0.6), inner),
            ] {
                self.vert(q, up, c, UNLIT);
            }
        }
    }
}

/// Section rectangulaire : centre et demi-largeur en Z, bas et haut en Y.
#[derive(Clone, Copy)]
struct Sec {
    zc: f32,
    zh: f32,
    y0: f32,
    y1: f32,
}

impl Sec {
    fn new(zc: f32, zh: f32, y0: f32, y1: f32) -> Sec {
        Sec { zc, zh, y0, y1 }
    }
    fn c(zh: f32, y0: f32, y1: f32) -> Sec {
        Sec::new(0.0, zh, y0, y1)
    }
}

/// Monoplace stylisée à effet de sol (mètres ; avant = +X, haut = +Y). Inspirée de la
/// silhouette générale des F1 actuelles, sans reproduire aucune voiture réelle.
fn car_mesh() -> Mesh {
    let mut m = Mesh::default();
    // Fond plat et diffuseur.
    m.loft(
        -1.95,
        Sec::c(0.80, 0.04, 0.08),
        1.2,
        Sec::c(0.80, 0.04, 0.08),
        CARBON,
        FIXED,
    );
    m.loft(
        1.2,
        Sec::c(0.80, 0.04, 0.08),
        1.65,
        Sec::c(0.30, 0.05, 0.08),
        CARBON,
        FIXED,
    );
    m.loft(
        -1.95,
        Sec::c(0.50, 0.04, 0.10),
        -2.45,
        Sec::c(0.55, 0.07, 0.30),
        CARBON,
        FIXED,
    );
    // Nez.
    m.loft(
        1.25,
        Sec::c(0.26, 0.20, 0.52),
        2.78,
        Sec::c(0.09, 0.13, 0.24),
        P,
        PAINT,
    );
    // Aileron avant : plans, volet et dérives.
    m.bx([2.45, 0.07, -0.98], [2.98, 0.10, 0.98], CARBON, FIXED);
    m.loft(
        2.36,
        Sec::c(0.95, 0.15, 0.18),
        2.62,
        Sec::c(0.95, 0.11, 0.14),
        P,
        ACCENT,
    );
    for s in [-1.0f32, 1.0] {
        m.bx(
            [2.34, 0.05, s * 0.97 - 0.02],
            [2.99, 0.33, s * 0.97 + 0.02],
            P,
            PAINT,
        );
    }
    // Monocoque et cockpit.
    m.loft(
        -0.25,
        Sec::c(0.40, 0.12, 0.64),
        1.3,
        Sec::c(0.27, 0.18, 0.54),
        P,
        PAINT,
    );
    m.bx([0.0, 0.62, -0.24], [0.78, 0.665, 0.24], DARK, FIXED);
    m.sphere([0.22, 0.72, 0.0], 0.15, P, ACCENT);
    m.bx([0.30, 0.70, -0.11], [0.375, 0.76, 0.11], DARK, FIXED); // visière
    // Halo.
    let hoop: Vec<V3> = (0..=12)
        .map(|i| {
            let a = (-150.0 + 25.0 * i as f32).to_radians();
            [0.28 + 0.45 * a.cos(), 0.86, 0.33 * a.sin()]
        })
        .collect();
    for w in hoop.windows(2) {
        m.beam(w[0], w[1], 0.028, CARBON, FIXED);
    }
    m.beam([0.73, 0.86, 0.0], [0.98, 0.56, 0.0], 0.03, CARBON, FIXED);
    m.beam(hoop[0], [-0.18, 0.62, -0.30], 0.03, CARBON, FIXED);
    m.beam(hoop[12], [-0.18, 0.62, 0.30], 0.03, CARBON, FIXED);
    // Prise d'air, capot moteur, aileron de requin.
    m.loft(
        -0.15,
        Sec::c(0.18, 0.60, 0.99),
        -0.65,
        Sec::c(0.25, 0.45, 0.95),
        P,
        PAINT,
    );
    m.bx([-0.16, 0.79, -0.11], [-0.12, 0.96, 0.11], DARK, FIXED);
    m.loft(
        -0.65,
        Sec::c(0.25, 0.18, 0.95),
        -2.0,
        Sec::c(0.11, 0.18, 0.46),
        P,
        PAINT,
    );
    m.loft(
        -0.65,
        Sec::c(0.012, 0.95, 0.97),
        -1.9,
        Sec::c(0.012, 0.46, 0.70),
        P,
        PAINT,
    );
    // Pontons.
    for s in [-1.0f32, 1.0] {
        m.loft(
            0.75,
            Sec::new(s * 0.60, 0.20, 0.16, 0.56),
            -0.4,
            Sec::new(s * 0.60, 0.22, 0.12, 0.52),
            P,
            PAINT,
        );
        m.loft(
            -0.4,
            Sec::new(s * 0.60, 0.22, 0.12, 0.52),
            -1.65,
            Sec::new(s * 0.40, 0.10, 0.12, 0.28),
            P,
            PAINT,
        );
        m.bx(
            [0.74, 0.30, s * 0.62 - 0.17],
            [0.765, 0.52, s * 0.62 + 0.17],
            DARK,
            FIXED,
        );
        // Rétroviseurs.
        m.bx(
            [0.52, 0.66, s * 0.48 - 0.07],
            [0.6, 0.73, s * 0.48 + 0.07],
            P,
            PAINT,
        );
        m.beam(
            [0.56, 0.6, s * 0.3],
            [0.56, 0.68, s * 0.42],
            0.012,
            CARBON,
            FIXED,
        );
    }
    // Aileron arrière, beam wing, support et feu de pluie.
    for s in [-1.0f32, 1.0] {
        m.bx(
            [-2.58, 0.36, s * 0.515 - 0.016],
            [-2.02, 1.0, s * 0.515 + 0.016],
            P,
            PAINT,
        );
    }
    m.loft(
        -2.52,
        Sec::c(0.5, 0.80, 0.84),
        -2.14,
        Sec::c(0.5, 0.82, 0.87),
        CARBON,
        FIXED,
    );
    m.loft(
        -2.27,
        Sec::c(0.5, 0.91, 0.94),
        -2.05,
        Sec::c(0.5, 0.95, 0.99),
        P,
        ACCENT,
    );
    m.bx([-2.45, 0.42, -0.45], [-2.2, 0.45, 0.45], CARBON, FIXED);
    m.bx([-2.22, 0.28, -0.03], [-1.98, 0.84, 0.03], CARBON, FIXED);
    m.bx(
        [-2.6, 0.30, -0.05],
        [-2.56, 0.36, 0.05],
        [1.0, 0.12, 0.12, 1.0],
        UNLIT,
    );
    // Roues et suspensions.
    for s in [-1.0f32, 1.0] {
        m.wheel([1.75, 0.36, s * 0.83], 0.36, 0.30);
        m.wheel([-1.85, 0.36, s * 0.80], 0.36, 0.40);
        m.beam(
            [1.45, 0.46, s * 0.24],
            [1.75, 0.42, s * 0.68],
            0.022,
            CARBON,
            FIXED,
        );
        m.beam(
            [1.95, 0.24, s * 0.18],
            [1.75, 0.30, s * 0.68],
            0.022,
            CARBON,
            FIXED,
        );
        m.beam(
            [-1.5, 0.46, s * 0.28],
            [-1.85, 0.44, s * 0.6],
            0.022,
            CARBON,
            FIXED,
        );
        m.beam(
            [-1.6, 0.20, s * 0.30],
            [-1.85, 0.28, s * 0.6],
            0.022,
            CARBON,
            FIXED,
        );
    }
    m
}

/// Ligne médiane du circuit dans le repère 3D, avec les temps du tour de référence.
struct Path {
    pts: Vec<V3>,
    t: Vec<f32>,
    lap_time: f32,
    radius: f32,
    centre_y: f32,
}

impl Path {
    /// Position et cap au temps `time` (secondes depuis le début du tour).
    fn at_time(&self, time: f32) -> (V3, f32) {
        let n = self.pts.len();
        let time = time.rem_euclid(self.lap_time.max(1.0));
        let i = self.t.partition_point(|&x| x <= time).clamp(1, n - 1) - 1;
        let j = (i + 1).min(n - 1);
        let span = (self.t[j] - self.t[i]).max(1e-3);
        let k = ((time - self.t[i]) / span).clamp(0.0, 1.0);
        let pos = lerp3(self.pts[i], self.pts[j], k);
        let d = sub(self.pts[(i + 2).min(n - 1)], self.pts[i.saturating_sub(1)]);
        (pos, (-d[2]).atan2(d[0]))
    }
}

/// Exagération du relief (sinon invisible à l'échelle d'un circuit).
const RELIEF: f32 = 4.0;
const HALF_WIDTH: f32 = 8.0;
const CAR_SCALE: f32 = 7.0;

fn speed_rgba(ratio: f32) -> Rgba {
    // Même rampe que la carte 2D : #b3261e → #ffe4de.
    let r = ratio.clamp(0.0, 1.0);
    let l = |a: f32, b: f32| (a + (b - a) * r) / 255.0;
    [l(179.0, 255.0), l(38.0, 228.0), l(30.0, 222.0), 1.0]
}

fn track_mesh(map: &TrackMap) -> Option<(Mesh, Path)> {
    let raw = &map.points;
    if raw.len() < 8 {
        return None;
    }
    let (cx, cz) = (map.width as f32 / 2.0, map.height as f32 / 2.0);
    let pts: Vec<V3> = raw
        .iter()
        .map(|p| [p.x - cx, p.z * RELIEF, p.y - cz])
        .collect();
    let n = pts.len();
    let closed = {
        let d = sub(pts[0], pts[n - 1]);
        dot(d, d).sqrt() < 60.0
    };
    let side = |i: usize| {
        let prev = if i == 0 {
            if closed { n - 1 } else { 0 }
        } else {
            i - 1
        };
        let next = if i + 1 == n {
            if closed { 0 } else { n - 1 }
        } else {
            i + 1
        };
        let d = sub(pts[next], pts[prev]);
        norm(cross([d[0], 0.0, d[2]], [0.0, 1.0, 0.0]))
    };
    let (min_s, max_s) = (map.stats.min_speed as f32, map.stats.top_speed as f32);
    let ground = -6.0;
    let mut m = Mesh::default();
    let up = [0.0, 1.0, 0.0];

    // Sol et quadrillage, pour la profondeur.
    let ext = cx.max(cz) + 160.0;
    let floor = [0.085, 0.085, 0.105, 1.0];
    m.face(
        &[
            [-ext, ground, -ext],
            [ext, ground, -ext],
            [ext, ground, ext],
            [-ext, ground, ext],
        ],
        [0.0, ground - 1.0, 0.0],
        floor,
        FIXED,
    );
    let grid = [0.15, 0.15, 0.18, 1.0];
    let mut g = -ext + 50.0;
    while g < ext {
        let y = ground + 0.1;
        m.face(
            &[
                [g - 0.8, y, -ext],
                [g + 0.8, y, -ext],
                [g + 0.8, y, ext],
                [g - 0.8, y, ext],
            ],
            [g, y - 1.0, 0.0],
            grid,
            FIXED,
        );
        m.face(
            &[
                [-ext, y, g - 0.8],
                [ext, y, g - 0.8],
                [ext, y, g + 0.8],
                [-ext, y, g + 0.8],
            ],
            [0.0, y - 1.0, g],
            grid,
            FIXED,
        );
        g += 100.0;
    }

    let segs = if closed { n } else { n - 1 };
    let wall = [0.20, 0.20, 0.24, 1.0];
    let kerb = [0.62, 0.62, 0.68, 1.0];
    for i in 0..segs {
        let j = (i + 1) % n;
        let (si, sj) = (side(i), side(j));
        let (li, ri) = (
            add(pts[i], mul(si, HALF_WIDTH)),
            sub(pts[i], mul(si, HALF_WIDTH)),
        );
        let (lj, rj) = (
            add(pts[j], mul(sj, HALF_WIDTH)),
            sub(pts[j], mul(sj, HALF_WIDTH)),
        );
        let ratio = (raw[i].speed as f32 - min_s) / (max_s - min_s).max(1.0);
        let below = [pts[i][0], pts[i][1] - 10.0, pts[i][2]];
        m.face(&[li, lj, rj, ri], below, speed_rgba(ratio), FIXED);
        // Liserés blancs le long des bords.
        for (a, b, s) in [(li, lj, 1.0f32), (ri, rj, -1.0)] {
            let (oa, ob) = (add(a, mul(si, s * 1.1)), add(b, mul(sj, s * 1.1)));
            m.face(&[a, b, ob, oa], below, kerb, FIXED);
            // Remblai jusqu'au sol : le relief se lit comme un volume.
            let (ga, gb) = ([oa[0], ground, oa[2]], [ob[0], ground, ob[2]]);
            m.face(&[oa, ob, gb, ga], pts[i], wall, FIXED);
        }
    }
    // Ligne de départ.
    let (s0, p0) = (side(0), pts[0]);
    let d0 = norm(sub(pts[2.min(n - 1)], p0));
    let a = add(p0, [0.0, 0.25, 0.0]);
    let quad = [
        add(add(a, mul(s0, HALF_WIDTH)), mul(d0, -1.6)),
        add(add(a, mul(s0, HALF_WIDTH)), mul(d0, 1.6)),
        add(sub(a, mul(s0, HALF_WIDTH)), mul(d0, 1.6)),
        add(sub(a, mul(s0, HALF_WIDTH)), mul(d0, -1.6)),
    ];
    m.face(&quad, sub(a, up), WHITE, UNLIT);

    let max_y = pts.iter().map(|p| p[1]).fold(0.0f32, f32::max);
    let path = Path {
        t: raw.iter().map(|p| p.t).collect(),
        lap_time: map.lap_time as f32,
        radius: (cx * cx + cz * cz).sqrt(),
        centre_y: max_y / 2.0,
        pts,
    };
    Some((m, path))
}

// ---------- WebGL ----------

const VS: &str = r#"#version 300 es
layout(location=0) in vec3 a_pos;
layout(location=1) in vec3 a_nrm;
layout(location=2) in vec4 a_col;
layout(location=3) in float a_kind;
uniform mat4 u_mvp;
uniform mat4 u_model;
uniform vec3 u_paint;
uniform vec3 u_accent;
out vec3 v_nrm;
out vec4 v_col;
out float v_unlit;
out vec3 v_world;
void main() {
  vec3 c = a_col.rgb;
  if (a_kind > 0.5 && a_kind < 1.5) c = u_paint;
  else if (a_kind > 1.5 && a_kind < 2.5) c = u_accent;
  v_col = vec4(c, a_col.a);
  v_unlit = a_kind > 2.5 ? 1.0 : 0.0;
  v_nrm = mat3(u_model) * a_nrm;
  v_world = (u_model * vec4(a_pos, 1.0)).xyz;
  gl_Position = u_mvp * vec4(a_pos, 1.0);
}"#;

const FS: &str = r#"#version 300 es
precision mediump float;
in vec3 v_nrm;
in vec4 v_col;
in float v_unlit;
in vec3 v_world;
uniform vec3 u_light;
uniform vec3 u_eye;
out vec4 o;
void main() {
  if (v_unlit > 0.5) { o = v_col; return; }
  vec3 n = normalize(v_nrm);
  vec3 v = normalize(u_eye - v_world);
  float d = max(dot(n, u_light), 0.0);
  float s = pow(max(dot(n, normalize(u_light + v)), 0.0), 48.0) * 0.5;
  float rim = pow(1.0 - max(dot(n, v), 0.0), 3.0) * 0.12;
  o = vec4(v_col.rgb * (0.30 + 0.70 * d) + vec3(s + rim), v_col.a);
}"#;

struct Uniforms {
    mvp: Option<WebGlUniformLocation>,
    model: Option<WebGlUniformLocation>,
    paint: Option<WebGlUniformLocation>,
    accent: Option<WebGlUniformLocation>,
    light: Option<WebGlUniformLocation>,
    eye: Option<WebGlUniformLocation>,
}

struct Gpu {
    gl: Gl,
    prog: WebGlProgram,
    u: Uniforms,
}

struct Vao {
    vao: WebGlVertexArrayObject,
    count: i32,
}

impl Gpu {
    fn new(canvas: &HtmlCanvasElement) -> Option<Gpu> {
        let gl: Gl = canvas.get_context("webgl2").ok()??.dyn_into().ok()?;
        let shader = |kind, src: &str| {
            let s = gl.create_shader(kind)?;
            gl.shader_source(&s, src);
            gl.compile_shader(&s);
            gl.get_shader_parameter(&s, Gl::COMPILE_STATUS)
                .as_bool()
                .unwrap_or(false)
                .then_some(s)
        };
        let prog = gl.create_program()?;
        gl.attach_shader(&prog, &shader(Gl::VERTEX_SHADER, VS)?);
        gl.attach_shader(&prog, &shader(Gl::FRAGMENT_SHADER, FS)?);
        gl.link_program(&prog);
        if !gl
            .get_program_parameter(&prog, Gl::LINK_STATUS)
            .as_bool()
            .unwrap_or(false)
        {
            return None;
        }
        let loc = |name| gl.get_uniform_location(&prog, name);
        let u = Uniforms {
            mvp: loc("u_mvp"),
            model: loc("u_model"),
            paint: loc("u_paint"),
            accent: loc("u_accent"),
            light: loc("u_light"),
            eye: loc("u_eye"),
        };
        gl.enable(Gl::DEPTH_TEST);
        gl.enable(Gl::BLEND);
        gl.blend_func_separate(
            Gl::SRC_ALPHA,
            Gl::ONE_MINUS_SRC_ALPHA,
            Gl::ONE,
            Gl::ONE_MINUS_SRC_ALPHA,
        );
        gl.clear_color(0.0, 0.0, 0.0, 0.0);
        Some(Gpu { gl, prog, u })
    }

    fn upload(&self, mesh: &Mesh) -> Option<Vao> {
        let gl = &self.gl;
        let vao = gl.create_vertex_array()?;
        gl.bind_vertex_array(Some(&vao));
        let buf = gl.create_buffer()?;
        gl.bind_buffer(Gl::ARRAY_BUFFER, Some(&buf));
        let data = js_sys::Float32Array::from(mesh.v.as_slice());
        gl.buffer_data_with_array_buffer_view(Gl::ARRAY_BUFFER, &data, Gl::STATIC_DRAW);
        let stride = (STRIDE * 4) as i32;
        for (index, size, offset) in [(0, 3, 0), (1, 3, 3), (2, 4, 6), (3, 1, 10)] {
            gl.enable_vertex_attrib_array(index);
            gl.vertex_attrib_pointer_with_i32(index, size, Gl::FLOAT, false, stride, offset * 4);
        }
        gl.bind_vertex_array(None);
        Some(Vao {
            vao,
            count: mesh.count(),
        })
    }

    fn draw(&self, vao: &Vao, vp: &M4, model: &M4, paint: [f32; 3], accent: [f32; 3]) {
        let gl = &self.gl;
        gl.uniform_matrix4fv_with_f32_array(self.u.mvp.as_ref(), false, &vp.mul(model).0);
        gl.uniform_matrix4fv_with_f32_array(self.u.model.as_ref(), false, &model.0);
        gl.uniform3f(self.u.paint.as_ref(), paint[0], paint[1], paint[2]);
        gl.uniform3f(self.u.accent.as_ref(), accent[0], accent[1], accent[2]);
        gl.bind_vertex_array(Some(&vao.vao));
        gl.draw_arrays(Gl::TRIANGLES, 0, vao.count);
    }
}

pub fn parse_colour(hex: &str) -> [f32; 3] {
    let h = hex.trim_start_matches('#');
    let c = |i: usize| {
        h.get(i..i + 2)
            .and_then(|s| u8::from_str_radix(s, 16).ok())
            .unwrap_or(138) as f32
            / 255.0
    };
    [c(0), c(2), c(4)]
}

fn accent_of(c: [f32; 3]) -> [f32; 3] {
    [c[0] * 0.4 + 0.6, c[1] * 0.4 + 0.6, c[2] * 0.4 + 0.6]
}

// ---------- Scène ----------

/// Ce que montre la vue 3D.
#[derive(Clone, PartialEq)]
pub enum Scene {
    /// Monoplace aux couleurs d'une écurie (hexadécimal).
    Car(AttrValue),
    /// Circuit en relief ; `ghost` = une voiture rejoue le tour de référence à vitesse réelle.
    Track { map: Rc<TrackMap>, ghost: bool },
}

/// Voiture placée sur le circuit (Race Center).
#[derive(Clone, PartialEq)]
pub struct Marker {
    pub key: String,
    pub label: String,
    pub colour: String,
    /// Avancement dans le tour (0–1).
    pub fraction: f32,
}

struct LiveCar {
    key: String,
    colour: [f32; 3],
    target: f32,
    cur: f32,
    label: Option<HtmlElement>,
}

struct Cam {
    yaw: f32,
    pitch: f32,
    zoom: f32,
}

struct State {
    gpu: Gpu,
    car: Vao,
    shadow: Vao,
    track: Option<(Vao, Path)>,
    paint: [f32; 3],
    ghost: bool,
    chase: bool,
    cam: Cam,
    /// Point de visée lissé de la caméra embarquée (œil, cible).
    chase_cam: Option<(V3, V3)>,
    drag: Option<(f64, f64)>,
    idle_since: f64,
    last: f64,
    clock: f64,
    cars: Vec<LiveCar>,
    labels: Option<HtmlElement>,
    canvas: HtmlCanvasElement,
}

fn now() -> f64 {
    web_sys::window()
        .and_then(|w| w.performance())
        .map(|p| p.now())
        .unwrap_or(0.0)
}

impl State {
    fn frame(&mut self) {
        let t = now();
        let dt = ((t - self.last) / 1000.0).clamp(0.0, 0.1) as f32;
        self.last = t;
        let Some(win) = web_sys::window() else { return };
        // Hors de l'écran : rien à dessiner (économie de batterie).
        let rect = self.canvas.get_bounding_client_rect();
        let vh = win
            .inner_height()
            .ok()
            .and_then(|v| v.as_f64())
            .unwrap_or(800.0);
        if rect.bottom() < 0.0 || rect.top() > vh || rect.width() < 1.0 {
            return;
        }
        self.clock += dt as f64;
        let dpr = win.device_pixel_ratio().min(2.0);
        let (w, h) = ((rect.width() * dpr) as u32, (rect.height() * dpr) as u32);
        if self.canvas.width() != w || self.canvas.height() != h {
            self.canvas.set_width(w);
            self.canvas.set_height(h);
        }
        let aspect = rect.width() as f32 / rect.height().max(1.0) as f32;
        if self.drag.is_none() && t - self.idle_since > 3500.0 {
            let speed = if self.track.is_some() { 0.07 } else { 0.35 };
            self.cam.yaw += speed * dt;
        }

        let fov = 0.62f32;
        let (eye, target, near, far) = match &self.track {
            None => {
                let dist = 3.4 / ((fov / 2.0).tan() * aspect.min(1.8)) * self.cam.zoom;
                let target = [0.0, 0.35, 0.0];
                (
                    orbit(target, self.cam.yaw, self.cam.pitch, dist),
                    target,
                    0.1,
                    80.0,
                )
            }
            Some((_, path)) => {
                let dist =
                    path.radius / (fov / 2.0).sin() * 1.08 / aspect.min(1.0).sqrt() * self.cam.zoom;
                if self.chase && self.ghost {
                    let (pos, heading) = path.at_time(self.clock as f32);
                    let fwd = [heading.cos(), 0.0, -heading.sin()];
                    let want_eye = add(add(pos, mul(fwd, -75.0)), [0.0, 26.0, 0.0]);
                    let want_target = add(add(pos, mul(fwd, 30.0)), [0.0, 4.0, 0.0]);
                    let k = (dt * 4.0).min(1.0);
                    let (e, tg) = self.chase_cam.unwrap_or((want_eye, want_target));
                    let cam = (lerp3(e, want_eye, k), lerp3(tg, want_target, k));
                    self.chase_cam = Some(cam);
                    (cam.0, cam.1, 1.0, path.radius * 6.0)
                } else {
                    self.chase_cam = None;
                    let target = [0.0, path.centre_y, 0.0];
                    (
                        orbit(target, self.cam.yaw, self.cam.pitch, dist),
                        target,
                        dist * 0.02,
                        dist * 4.0,
                    )
                }
            }
        };
        let vp =
            M4::perspective(fov, aspect, near, far).mul(&M4::look_at(eye, target, [0.0, 1.0, 0.0]));

        let gpu = &self.gpu;
        let gl = &gpu.gl;
        gl.viewport(0, 0, w as i32, h as i32);
        gl.clear(Gl::COLOR_BUFFER_BIT | Gl::DEPTH_BUFFER_BIT);
        gl.use_program(Some(&gpu.prog));
        let light = norm([0.45, 0.9, 0.35]);
        gl.uniform3f(gpu.u.light.as_ref(), light[0], light[1], light[2]);
        gl.uniform3f(gpu.u.eye.as_ref(), eye[0], eye[1], eye[2]);
        let id = M4::identity();
        let paint = self.paint;
        match &self.track {
            None => {
                gl.depth_mask(false);
                gpu.draw(&self.shadow, &vp, &id, paint, paint);
                gl.depth_mask(true);
                gpu.draw(&self.car, &vp, &id, paint, accent_of(paint));
            }
            Some((vao, path)) => {
                gpu.draw(vao, &vp, &id, paint, paint);
                if self.ghost {
                    let (pos, heading) = path.at_time(self.clock as f32);
                    let model = M4::trs(add(pos, [0.0, 0.6, 0.0]), heading, CAR_SCALE);
                    gpu.draw(&self.car, &vp, &model, paint, accent_of(paint));
                }
                let (cw, ch) = (rect.width() as f32, rect.height() as f32);
                for car in &mut self.cars {
                    // Avance en douceur vers la dernière position connue (tour bouclé).
                    let diff = (car.target - car.cur + 0.5).rem_euclid(1.0) - 0.5;
                    car.cur = (car.cur + diff * (dt * 2.5).min(1.0)).rem_euclid(1.0);
                    let (pos, heading) = path.at_time(car.cur * path.lap_time);
                    let model = M4::trs(add(pos, [0.0, 0.6, 0.0]), heading, CAR_SCALE);
                    gpu.draw(&self.car, &vp, &model, car.colour, accent_of(car.colour));
                    if let Some(el) = &car.label {
                        let style = el.style();
                        match vp.project(add(pos, [0.0, 14.0, 0.0])) {
                            Some((x, y)) if x.abs() < 1.05 && y.abs() < 1.05 => {
                                let (px, py) = ((x + 1.0) / 2.0 * cw, (1.0 - y) / 2.0 * ch);
                                let _ = style.set_property(
                                    "transform",
                                    &format!(
                                        "translate({px:.0}px,{py:.0}px) translate(-50%,-100%)"
                                    ),
                                );
                                let _ = style.set_property("visibility", "visible");
                            }
                            _ => {
                                let _ = style.set_property("visibility", "hidden");
                            }
                        }
                    }
                }
            }
        }
    }

    fn set_markers(&mut self, markers: &[Marker]) {
        let doc = web_sys::window().and_then(|w| w.document());
        let mut next = Vec::with_capacity(markers.len());
        for m in markers {
            let colour = parse_colour(&m.colour);
            match self.cars.iter().position(|c| c.key == m.key) {
                Some(i) => {
                    let mut car = self.cars.swap_remove(i);
                    car.target = m.fraction;
                    car.colour = colour;
                    if let Some(el) = &car.label {
                        el.set_text_content(Some(&m.label));
                    }
                    next.push(car);
                }
                None => {
                    let label = match (&doc, &self.labels) {
                        (Some(doc), Some(parent)) => doc
                            .create_element("span")
                            .ok()
                            .and_then(|e| e.dyn_into::<HtmlElement>().ok())
                            .inspect(|el| {
                                el.set_class_name("scene-label");
                                el.set_text_content(Some(&m.label));
                                let _ = el.style().set_property(
                                    "--team",
                                    &format!("#{}", m.colour.trim_start_matches('#')),
                                );
                                let _ = parent.append_child(el);
                            }),
                        _ => None,
                    };
                    next.push(LiveCar {
                        key: m.key.clone(),
                        colour,
                        target: m.fraction,
                        cur: m.fraction,
                        label,
                    });
                }
            }
        }
        for gone in self.cars.drain(..) {
            if let Some(el) = gone.label {
                el.remove();
            }
        }
        self.cars = next;
    }
}

fn orbit(target: V3, yaw: f32, pitch: f32, dist: f32) -> V3 {
    add(
        target,
        [
            dist * pitch.cos() * yaw.cos(),
            dist * pitch.sin(),
            dist * pitch.cos() * yaw.sin(),
        ],
    )
}

/// Rappel requestAnimationFrame.
type Frame = Closure<dyn FnMut()>;

/// Vue 3D active : la boucle d'animation s'arrête quand elle est détruite.
struct Viewer {
    state: Rc<RefCell<State>>,
    alive: Rc<Cell<bool>>,
}

impl Viewer {
    fn new(
        canvas: HtmlCanvasElement,
        labels: Option<HtmlElement>,
        scene: &Scene,
    ) -> Option<Viewer> {
        let gpu = Gpu::new(&canvas)?;
        let car = gpu.upload(&car_mesh())?;
        let mut sh = Mesh::default();
        sh.shadow([0.0, 0.002, 0.0], 3.3, 1.35);
        let shadow = gpu.upload(&sh)?;
        let (paint, track, ghost) = match scene {
            Scene::Car(colour) => (parse_colour(colour), None, false),
            Scene::Track { map, ghost } => {
                let (mesh, path) = track_mesh(map)?;
                (
                    parse_colour(&map.colour),
                    Some((gpu.upload(&mesh)?, path)),
                    *ghost,
                )
            }
        };
        let pitch = if track.is_some() { 0.82 } else { 0.32 };
        let t = now();
        let state = Rc::new(RefCell::new(State {
            gpu,
            car,
            shadow,
            track,
            paint,
            ghost,
            chase: false,
            cam: Cam {
                yaw: 0.9,
                pitch,
                zoom: 1.0,
            },
            chase_cam: None,
            drag: None,
            idle_since: t - 10_000.0,
            last: t,
            clock: 0.0,
            cars: Vec::new(),
            labels,
            canvas,
        }));
        let alive = Rc::new(Cell::new(true));

        // Boucle requestAnimationFrame (se libère d'elle-même quand la vue disparaît).
        let tick: Rc<RefCell<Option<Frame>>> = Rc::new(RefCell::new(None));
        let again = Rc::clone(&tick);
        let (st, live) = (Rc::clone(&state), Rc::clone(&alive));
        *tick.borrow_mut() = Some(Closure::new(move || {
            if !live.get() {
                let _ = again.borrow_mut().take();
                return;
            }
            st.borrow_mut().frame();
            if let (Some(win), Some(cb)) = (web_sys::window(), again.borrow().as_ref()) {
                let _ = win.request_animation_frame(cb.as_ref().unchecked_ref());
            }
        }));
        if let (Some(win), Some(cb)) = (web_sys::window(), tick.borrow().as_ref()) {
            let _ = win.request_animation_frame(cb.as_ref().unchecked_ref());
        }
        Some(Viewer { state, alive })
    }
}

impl Drop for Viewer {
    fn drop(&mut self) {
        self.alive.set(false);
        if let Ok(mut st) = self.state.try_borrow_mut() {
            st.set_markers(&[]);
        }
    }
}

// ---------- Composant ----------

#[derive(Properties, PartialEq)]
pub struct ViewProps {
    pub scene: Scene,
    #[prop_or_default]
    pub markers: Option<Rc<Vec<Marker>>>,
}

/// Vue 3D interactive : glisser horizontalement pour tourner (le défilement vertical de la page
/// reste libre), boutons pour zoomer et, sur un circuit, passer en caméra embarquée.
#[function_component]
pub fn View3D(props: &ViewProps) -> Html {
    let canvas = use_node_ref();
    let labels = use_node_ref();
    let viewer = use_mut_ref(|| None::<Viewer>);
    let failed = use_state(|| false);
    let chase = use_state(|| false);
    {
        let (canvas, labels, viewer, failed) = (
            canvas.clone(),
            labels.clone(),
            viewer.clone(),
            failed.clone(),
        );
        use_effect_with(props.scene.clone(), move |scene| {
            let made = canvas
                .cast::<HtmlCanvasElement>()
                .and_then(|c| Viewer::new(c, labels.cast::<HtmlElement>(), scene));
            if made.is_none() {
                failed.set(true);
            }
            *viewer.borrow_mut() = made;
            move || {
                viewer.borrow_mut().take();
            }
        });
    }
    {
        let viewer = viewer.clone();
        use_effect_with(
            (props.scene.clone(), props.markers.clone()),
            move |(_, markers)| {
                if let Some(v) = viewer.borrow().as_ref() {
                    v.state
                        .borrow_mut()
                        .set_markers(markers.as_deref().map(Vec::as_slice).unwrap_or(&[]));
                }
            },
        );
    }
    {
        let viewer = viewer.clone();
        use_effect_with(*chase, move |on| {
            if let Some(v) = viewer.borrow().as_ref() {
                v.state.borrow_mut().chase = *on;
            }
        });
    }
    let with = {
        let viewer = viewer.clone();
        move |f: Box<dyn Fn(&mut State)>| {
            if let Some(v) = viewer.borrow().as_ref() {
                f(&mut v.state.borrow_mut());
            }
        }
    };
    let onpointerdown = {
        let with = with.clone();
        Callback::from(move |e: PointerEvent| {
            if let Some(el) = e.target().and_then(|t| t.dyn_into::<Element>().ok()) {
                let _ = el.set_pointer_capture(e.pointer_id());
            }
            let (x, y) = (e.client_x() as f64, e.client_y() as f64);
            with(Box::new(move |s| s.drag = Some((x, y))));
        })
    };
    let onpointermove = {
        let with = with.clone();
        Callback::from(move |e: PointerEvent| {
            let (x, y) = (e.client_x() as f64, e.client_y() as f64);
            with(Box::new(move |s| {
                if let Some((px, py)) = s.drag {
                    s.cam.yaw += ((x - px) * 0.009) as f32;
                    s.cam.pitch = (s.cam.pitch + ((y - py) * 0.006) as f32).clamp(0.08, 1.45);
                    s.drag = Some((x, y));
                    s.idle_since = now();
                }
            }));
        })
    };
    let onpointerup = {
        let with = with.clone();
        Callback::from(move |_: PointerEvent| {
            with(Box::new(|s| {
                s.drag = None;
                s.idle_since = now();
            }))
        })
    };
    let zoom = |k: f32| {
        let with = with.clone();
        Callback::from(move |_: MouseEvent| {
            with(Box::new(move |s| {
                s.cam.zoom = (s.cam.zoom * k).clamp(0.35, 2.5)
            }))
        })
    };
    let is_track = matches!(props.scene, Scene::Track { .. });
    let ghost = matches!(props.scene, Scene::Track { ghost: true, .. });
    if *failed {
        return html! {
            <p class="muted">{ t("La 3D n'est pas disponible sur cet appareil (WebGL 2 requis).", "3D isn't available on this device (WebGL 2 required).") }</p>
        };
    }
    html! {
        <div class={classes!("scene", is_track.then_some("scene-track"))}>
            <canvas ref={canvas} class="scene-canvas" role="img"
                aria-label={if is_track { t("Circuit en 3D", "3D circuit") } else { t("Monoplace en 3D", "3D car") }}
                onpointerdown={onpointerdown} onpointermove={onpointermove}
                onpointerup={onpointerup.clone()} onpointercancel={onpointerup} />
            <div ref={labels} class="scene-labels" aria-hidden="true"></div>
            <div class="scene-tools">
                if ghost {
                    <button class={classes!("scene-btn", chase.then_some("on"))} aria-pressed={chase.to_string()}
                        onclick={let chase = chase.clone(); move |_| chase.set(!*chase)}>
                        { if *chase { t("Vue d'ensemble", "Overview") } else { t("Caméra embarquée", "Onboard camera") } }
                    </button>
                }
                <button class="scene-btn" aria-label={t("Rapprocher", "Zoom in")} onclick={zoom(0.8)}>{ "+" }</button>
                <button class="scene-btn" aria-label={t("Éloigner", "Zoom out")} onclick={zoom(1.25)}>{ "−" }</button>
            </div>
        </div>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn colours_and_projection() {
        assert_eq!(parse_colour("#FF8000"), [1.0, 128.0 / 255.0, 0.0]);
        let vp = M4::perspective(1.0, 1.0, 0.1, 100.0).mul(&M4::look_at(
            [0.0, 0.0, 5.0],
            [0.0; 3],
            [0.0, 1.0, 0.0],
        ));
        let (x, y) = vp.project([0.0; 3]).unwrap();
        assert!(x.abs() < 1e-5 && y.abs() < 1e-5);
        assert!(vp.project([0.0, 0.0, 10.0]).is_none());
        assert!(car_mesh().count() > 1000);
    }
}

/// Carte « monoplace en 3D » aux couleurs d'une écurie.
pub fn car_card(colour: &str, team: &str) -> Html {
    html! {
        <section class="card">
            <h2>{ t("La monoplace en 3D", "The car in 3D") }</h2>
            <View3D scene={Scene::Car(AttrValue::from(colour.to_string()))} />
            <p class="muted">{ crate::tr!(
                "Monoplace stylisée aux couleurs {} — modèle généré par le code, pas une reproduction officielle. Glisse pour la faire tourner.",
                "Stylised car in {} colours — generated by code, not an official replica. Drag to spin it.",
                team
            ) }</p>
        </section>
    }
}
