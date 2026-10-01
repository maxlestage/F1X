//! Rendu 3D (WebGL2) écrit en Rust : monoplaces stylisées générées par le code (aucun modèle
//! officiel) et circuits en relief reconstitués à partir des positions GPS OpenF1.

use std::cell::{Cell, RefCell};
use std::f32::consts::{PI, TAU};
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

/// Nature d'un sommet : couleur fixe, couleurs de la livrée (principale, secondaire, accent)
/// ou non éclairé (ombres, ligne de départ).
const FIXED: f32 = 0.0;
const PAINT: f32 = 1.0;
const ACCENT: f32 = 2.0;
const UNLIT: f32 = 3.0;
const SECOND: f32 = 4.0;

/// Position (3) + normale (3) + couleur RGBA (4) + nature (1).
/// Pour les sommets éclairés, l'alpha porte la brillance du matériau (0 mat → 1 vernis).
const STRIDE: usize = 11;

type Rgba = [f32; 4];

const CARBON: Rgba = [0.05, 0.05, 0.058, 0.25];
const DARK: Rgba = [0.015, 0.015, 0.02, 0.6];
const TYRE: Rgba = [0.045, 0.045, 0.05, 0.06];
const RIM: Rgba = [0.30, 0.31, 0.33, 0.85];
const STRIPE: Rgba = [0.98, 0.80, 0.08, 0.2];
const VISOR: Rgba = [0.02, 0.03, 0.05, 1.0];
const NUT: Rgba = [0.85, 0.08, 0.08, 0.7];
const WHITE: Rgba = [1.0, 1.0, 1.0, 1.0];
/// Couleur remplacée par la livrée ; alpha = brillance de la peinture.
const P: Rgba = [1.0, 1.0, 1.0, 1.0];

#[derive(Default)]
struct Mesh {
    v: Vec<f32>,
}

/// Section d'une carrosserie lissée : position sur l'axe X, centre en Z, demi-largeur, bas, haut.
#[derive(Clone, Copy)]
struct Ring {
    x: f32,
    zc: f32,
    hw: f32,
    y0: f32,
    y1: f32,
}

const fn ring(x: f32, hw: f32, y0: f32, y1: f32) -> Ring {
    Ring {
        x,
        zc: 0.0,
        hw,
        y0,
        y1,
    }
}

const fn side_ring(x: f32, zc: f32, hw: f32, y0: f32, y1: f32) -> Ring {
    Ring { x, zc, hw, y0, y1 }
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

    /// Quadrilatère à normales par sommet.
    fn quad_n(&mut self, p: [V3; 4], n: [V3; 4], c: Rgba, k: f32) {
        for i in [0, 1, 2, 0, 2, 3] {
            self.vert(p[i], n[i], c, k);
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
        self.hexa(
            [
                [min[0], min[1], min[2]],
                [min[0], min[1], max[2]],
                [min[0], max[1], max[2]],
                [min[0], max[1], min[2]],
                [max[0], min[1], min[2]],
                [max[0], min[1], max[2]],
                [max[0], max[1], max[2]],
                [max[0], max[1], min[2]],
            ],
            c,
            k,
        );
    }

    /// Lame (aileron) : profil incliné entre deux bords d'attaque/de fuite, sur une envergure.
    fn blade(
        &mut self,
        front: (f32, f32),
        back: (f32, f32),
        thick: f32,
        (z0, z1): (f32, f32),
        (c, k): (Rgba, f32),
    ) {
        let (xf, yf) = front;
        let (xb, yb) = back;
        self.hexa(
            [
                [xf, yf, z0],
                [xf, yf, z1],
                [xf, yf + thick, z1],
                [xf, yf + thick, z0],
                [xb, yb, z0],
                [xb, yb, z1],
                [xb, yb + thick, z1],
                [xb, yb + thick, z0],
            ],
            c,
            k,
        );
    }

    /// Carrosserie lissée : sections en super-ellipse (exposant `e`, 2 = ellipse, plus = plus
    /// carré) reliées entre elles, normales lissées. `skin` choisit la couleur selon la hauteur
    /// relative (−1 en bas, +1 en haut), pour une livrée bicolore.
    fn smooth(&mut self, rings: &[Ring], e: f32, segs: usize, skin: &dyn Fn(f32) -> (Rgba, f32)) {
        let n = rings.len();
        let shape = |a: f32| {
            let (s, c) = a.sin_cos();
            (
                c.signum() * c.abs().powf(2.0 / e),
                s.signum() * s.abs().powf(2.0 / e),
            )
        };
        let unit: Vec<(f32, f32)> = (0..segs)
            .map(|j| shape(TAU * j as f32 / segs as f32))
            .collect();
        let point = |i: usize, j: usize| {
            let r = rings[i];
            let (u, v) = unit[j % segs];
            [
                r.x,
                (r.y0 + r.y1) / 2.0 + v * (r.y1 - r.y0) / 2.0,
                r.zc + u * r.hw,
            ]
        };
        let centre = |i: usize| {
            let r = rings[i];
            [r.x, (r.y0 + r.y1) / 2.0, r.zc]
        };
        let normal = |i: usize, j: usize| {
            let along = sub(point((i + 1).min(n - 1), j), point(i.saturating_sub(1), j));
            let around = sub(point(i, j + 1), point(i, j + segs - 1));
            let mut nn = norm(cross(along, around));
            if dot(nn, sub(point(i, j), centre(i))) < 0.0 {
                nn = mul(nn, -1.0);
            }
            nn
        };
        for i in 0..n - 1 {
            for j in 0..segs {
                let (c, k) = skin((unit[j].1 + unit[(j + 1) % segs].1) / 2.0);
                self.quad_n(
                    [
                        point(i, j),
                        point(i + 1, j),
                        point(i + 1, j + 1),
                        point(i, j + 1),
                    ],
                    [
                        normal(i, j),
                        normal(i + 1, j),
                        normal(i + 1, j + 1),
                        normal(i, j + 1),
                    ],
                    c,
                    k,
                );
            }
        }
        // Bouchons aux extrémités.
        for (i, dir) in [(0usize, -1.0f32), (n - 1, 1.0)] {
            let nn = norm(mul(
                sub(centre((i + 1).min(n - 1)), centre(i.saturating_sub(1))),
                dir,
            ));
            let (c, k) = skin(0.0);
            for j in 0..segs {
                for p in [centre(i), point(i, j), point(i, j + 1)] {
                    self.vert(p, nn, c, k);
                }
            }
        }
    }

    /// Tube de section ronde le long d'une polyligne (halo, suspensions).
    fn tube(&mut self, pts: &[V3], r: f32, c: Rgba, k: f32) {
        let segs = 8;
        let rings: Vec<Vec<(V3, V3)>> = (0..pts.len())
            .map(|i| {
                let t = norm(sub(
                    pts[(i + 1).min(pts.len() - 1)],
                    pts[i.saturating_sub(1)],
                ));
                let up = if t[1].abs() > 0.9 {
                    [1.0, 0.0, 0.0]
                } else {
                    [0.0, 1.0, 0.0]
                };
                let s = norm(cross(t, up));
                let u = cross(s, t);
                (0..segs)
                    .map(|j| {
                        let a = TAU * j as f32 / segs as f32;
                        let d = add(mul(s, a.cos()), mul(u, a.sin()));
                        (add(pts[i], mul(d, r)), d)
                    })
                    .collect()
            })
            .collect();
        for w in rings.windows(2) {
            for j in 0..segs {
                let jn = (j + 1) % segs;
                self.quad_n(
                    [w[0][j].0, w[1][j].0, w[1][jn].0, w[0][jn].0],
                    [w[0][j].1, w[1][j].1, w[1][jn].1, w[0][jn].1],
                    c,
                    k,
                );
            }
        }
    }

    fn sphere(&mut self, centre: V3, r: V3, c: Rgba, k: f32) {
        let (rings, segs) = (10, 18);
        let at = |i: usize, j: usize| {
            let th = PI * i as f32 / rings as f32;
            let ph = TAU * j as f32 / segs as f32;
            [th.sin() * ph.cos(), th.cos(), th.sin() * ph.sin()]
        };
        for i in 0..rings {
            for j in 0..segs {
                let q = [at(i, j), at(i + 1, j), at(i + 1, j + 1), at(i, j + 1)];
                let p = q.map(|u| add(centre, [u[0] * r[0], u[1] * r[1], u[2] * r[2]]));
                let n = q.map(|u| norm([u[0] / r[0], u[1] / r[1], u[2] / r[2]]));
                self.quad_n(p, n, c, k);
            }
        }
    }

    /// Anneau plat perpendiculaire à Z (flancs de pneus, jantes).
    fn ring_z(&mut self, centre: V3, (r0, r1): (f32, f32), z: f32, nz: f32, (c, k): (Rgba, f32)) {
        let segs = 40;
        let n = [0.0, 0.0, nz];
        for i in 0..segs {
            let (a0, a1) = (
                TAU * i as f32 / segs as f32,
                TAU * (i + 1) as f32 / segs as f32,
            );
            let p = |a: f32, r: f32| {
                [
                    centre[0] + a.cos() * r,
                    centre[1] + a.sin() * r,
                    centre[2] + z,
                ]
            };
            self.quad_n([p(a0, r0), p(a1, r0), p(a1, r1), p(a0, r1)], [n; 4], c, k);
        }
    }

    /// Roue d'axe Z : pneu au profil arrondi (révolution), liseré de gomme, jante, enjoliveur.
    fn wheel(&mut self, centre: V3, r: f32, width: f32) {
        let segs = 40;
        let hw = width / 2.0;
        let (ri, rc) = (r * 0.70, 0.075);
        // Profil (rayon, z, normale radiale, normale z), parcouru autour de la section du pneu.
        let mut prof: Vec<(f32, f32, f32, f32)> = vec![(ri, -hw, 0.0, -1.0)];
        for s in 0..=6 {
            let f = PI / 2.0 * s as f32 / 6.0;
            prof.push((
                r - rc + rc * f.sin(),
                -hw + rc - rc * f.cos(),
                f.sin(),
                -f.cos(),
            ));
        }
        for s in 0..=6 {
            let f = PI / 2.0 + PI / 2.0 * s as f32 / 6.0;
            prof.push((
                r - rc + rc * f.sin(),
                hw - rc - rc * f.cos(),
                f.sin(),
                -f.cos(),
            ));
        }
        prof.push((ri, hw, 0.0, 1.0));
        for i in 0..segs {
            let (a0, a1) = (
                TAU * i as f32 / segs as f32,
                TAU * (i + 1) as f32 / segs as f32,
            );
            for w in prof.windows(2) {
                let p = |a: f32, q: (f32, f32, f32, f32)| {
                    (
                        [
                            centre[0] + a.cos() * q.0,
                            centre[1] + a.sin() * q.0,
                            centre[2] + q.1,
                        ],
                        norm([a.cos() * q.2, a.sin() * q.2, q.3]),
                    )
                };
                let (p0, n0) = p(a0, w[0]);
                let (p1, n1) = p(a1, w[0]);
                let (p2, n2) = p(a1, w[1]);
                let (p3, n3) = p(a0, w[1]);
                self.quad_n([p0, p1, p2, p3], [n0, n1, n2, n3], TYRE, FIXED);
            }
        }
        for side in [-1.0f32, 1.0] {
            let z = side * (hw + 0.002);
            self.ring_z(centre, (r * 0.80, r * 0.845), z, side, (STRIPE, FIXED));
            // Jante en retrait, enjoliveur aux couleurs de l'écurie, écrou central.
            let zr = side * (hw - 0.035);
            self.ring_z(centre, (ri, r * 0.62), zr, side, (RIM, FIXED));
            self.ring_z(centre, (r * 0.62, r * 0.52), zr, side, (P, ACCENT));
            self.ring_z(centre, (r * 0.52, r * 0.12), zr, side, (CARBON, FIXED));
            self.ring_z(
                centre,
                (r * 0.12, 0.0),
                side * (hw - 0.02),
                side,
                (NUT, FIXED),
            );
            // Lèvre de jante entre le flanc et le disque.
            for i in 0..segs {
                let (a0, a1) = (
                    TAU * i as f32 / segs as f32,
                    TAU * (i + 1) as f32 / segs as f32,
                );
                let p = |a: f32, z: f32| {
                    [
                        centre[0] + a.cos() * ri,
                        centre[1] + a.sin() * ri,
                        centre[2] + z,
                    ]
                };
                let n = |a: f32| [-a.cos(), -a.sin(), 0.0];
                self.quad_n(
                    [p(a0, side * hw), p(a1, side * hw), p(a1, zr), p(a0, zr)],
                    [n(a0), n(a1), n(a1), n(a0)],
                    RIM,
                    FIXED,
                );
            }
        }
    }

    /// Ombre douce elliptique au sol (dégradé de transparence).
    fn shadow(&mut self, centre: V3, rx: f32, rz: f32) {
        let segs = 40;
        let up = [0.0, 1.0, 0.0];
        for i in 0..segs {
            let a0 = TAU * i as f32 / segs as f32;
            let a1 = TAU * (i + 1) as f32 / segs as f32;
            let p = |a: f32, k: f32| {
                [
                    centre[0] + a.cos() * rx * k,
                    centre[1],
                    centre[2] + a.sin() * rz * k,
                ]
            };
            let (inner, outer) = ([0.0, 0.0, 0.0, 0.6], [0.0, 0.0, 0.0, 0.0]);
            self.vert(centre, up, inner, UNLIT);
            self.vert(p(a0, 0.55), up, inner, UNLIT);
            self.vert(p(a1, 0.55), up, inner, UNLIT);
            for (q, c) in [
                (p(a0, 0.55), inner),
                (p(a0, 1.0), outer),
                (p(a1, 1.0), outer),
                (p(a0, 0.55), inner),
                (p(a1, 1.0), outer),
                (p(a1, 0.55), inner),
            ] {
                self.vert(q, up, c, UNLIT);
            }
        }
    }
}

/// Peinture principale en haut, couleur secondaire en bas (livrée bicolore).
fn two_tone(rel: f32) -> (Rgba, f32) {
    if rel < -0.2 { (P, SECOND) } else { (P, PAINT) }
}

/// Monoplace stylisée à effet de sol (mètres ; avant = +X, haut = +Y). Inspirée de la
/// silhouette générale des F1 actuelles, sans reproduire aucune voiture réelle.
fn car_mesh() -> Mesh {
    let mut m = Mesh::default();

    // Fond plat, bords du fond, planche et diffuseur.
    m.hexa(
        [
            [1.62, 0.035, -0.28],
            [1.62, 0.035, 0.28],
            [1.62, 0.065, 0.28],
            [1.62, 0.065, -0.28],
            [1.05, 0.035, -0.80],
            [1.05, 0.035, 0.80],
            [1.05, 0.065, 0.80],
            [1.05, 0.065, -0.80],
        ],
        CARBON,
        FIXED,
    );
    m.bx([-1.95, 0.035, -0.80], [1.05, 0.065, 0.80], CARBON, FIXED);
    for s in [-1.0f32, 1.0] {
        m.blade(
            (0.9, 0.065),
            (-1.6, 0.065),
            0.09,
            (s * 0.80, s * 0.785),
            (CARBON, FIXED),
        );
        m.blade(
            (0.2, 0.10),
            (-1.2, 0.10),
            0.015,
            (s * 0.80, s * 0.70),
            (P, SECOND),
        );
    }
    m.hexa(
        [
            [-1.95, 0.035, -0.55],
            [-1.95, 0.035, 0.55],
            [-1.95, 0.11, 0.55],
            [-1.95, 0.11, -0.55],
            [-2.45, 0.08, -0.56],
            [-2.45, 0.08, 0.56],
            [-2.45, 0.30, 0.56],
            [-2.45, 0.30, -0.56],
        ],
        CARBON,
        FIXED,
    );

    // Châssis : nez, monocoque et capot moteur d'un seul tenant.
    m.smooth(
        &[
            ring(2.88, 0.05, 0.17, 0.23),
            ring(2.75, 0.08, 0.15, 0.28),
            ring(2.45, 0.12, 0.14, 0.36),
            ring(2.05, 0.17, 0.14, 0.44),
            ring(1.6, 0.23, 0.13, 0.52),
            ring(1.15, 0.30, 0.12, 0.59),
            ring(0.75, 0.36, 0.10, 0.64),
            ring(0.3, 0.40, 0.09, 0.66),
            ring(-0.1, 0.41, 0.09, 0.70),
            ring(-0.5, 0.36, 0.10, 0.78),
            ring(-0.95, 0.27, 0.12, 0.68),
            ring(-1.45, 0.19, 0.14, 0.55),
            ring(-1.9, 0.12, 0.16, 0.44),
            ring(-2.2, 0.07, 0.18, 0.36),
        ],
        3.2,
        28,
        &two_tone,
    );
    // Habitacle, pilote (casque et visière), appui-tête.
    m.sphere([0.38, 0.655, 0.0], [0.44, 0.03, 0.22], DARK, FIXED);
    m.sphere([0.22, 0.73, 0.0], [0.145, 0.14, 0.125], P, ACCENT);
    m.sphere([0.30, 0.735, 0.0], [0.08, 0.045, 0.11], VISOR, FIXED);
    m.smooth(
        &[ring(0.02, 0.2, 0.58, 0.70), ring(-0.08, 0.22, 0.58, 0.74)],
        3.0,
        16,
        &|_| (P, SECOND),
    );

    // Prise d'air au-dessus du pilote, aileron de requin et caméra.
    m.smooth(
        &[
            ring(-0.04, 0.10, 0.70, 0.96),
            ring(-0.18, 0.15, 0.66, 1.0),
            ring(-0.45, 0.17, 0.62, 0.95),
            ring(-0.85, 0.12, 0.58, 0.80),
            ring(-1.2, 0.05, 0.55, 0.66),
        ],
        2.6,
        20,
        &|_| (P, PAINT),
    );
    m.sphere([-0.035, 0.85, 0.0], [0.02, 0.085, 0.075], DARK, FIXED);
    m.hexa(
        [
            [-0.8, 0.80, -0.006],
            [-0.8, 0.80, 0.006],
            [-0.8, 0.92, 0.006],
            [-0.8, 0.92, -0.006],
            [-1.9, 0.44, -0.006],
            [-1.9, 0.44, 0.006],
            [-1.9, 0.62, 0.006],
            [-1.9, 0.62, -0.006],
        ],
        P,
        ACCENT,
    );
    m.bx([-0.12, 1.0, -0.06], [0.0, 1.04, 0.06], P, ACCENT);

    // Pontons (avec entrées d'air) et rétroviseurs.
    for s in [-1.0f32, 1.0] {
        m.smooth(
            &[
                side_ring(0.98, s * 0.55, 0.11, 0.20, 0.44),
                side_ring(0.82, s * 0.58, 0.19, 0.14, 0.54),
                side_ring(0.35, s * 0.59, 0.22, 0.11, 0.56),
                side_ring(-0.25, s * 0.53, 0.20, 0.11, 0.49),
                side_ring(-0.85, s * 0.43, 0.14, 0.12, 0.37),
                side_ring(-1.45, s * 0.31, 0.08, 0.14, 0.26),
            ],
            3.6,
            24,
            &two_tone,
        );
        m.sphere([0.985, 0.33, s * 0.55], [0.01, 0.09, 0.08], DARK, FIXED);
        m.tube(
            &[[0.6, 0.62, s * 0.36], [0.58, 0.70, s * 0.47]],
            0.012,
            CARBON,
            FIXED,
        );
        m.smooth(
            &[
                side_ring(0.63, s * 0.52, 0.07, 0.68, 0.76),
                side_ring(0.53, s * 0.52, 0.075, 0.67, 0.77),
            ],
            3.0,
            14,
            &|_| (P, PAINT),
        );
        m.bx(
            [0.528, 0.685, s * 0.52 - 0.06],
            [0.532, 0.755, s * 0.52 + 0.06],
            [0.6, 0.65, 0.7, 1.0],
            FIXED,
        );
    }

    // Halo.
    let hoop: Vec<V3> = (0..=16)
        .map(|i| {
            let a = (-150.0 + 300.0 * i as f32 / 16.0).to_radians();
            [
                0.30 + 0.44 * a.cos(),
                0.87 - 0.03 * a.cos().abs(),
                0.32 * a.sin(),
            ]
        })
        .collect();
    m.tube(&hoop, 0.026, CARBON, FIXED);
    m.tube(
        &[[0.74, 0.84, 0.0], [0.86, 0.72, 0.0], [0.98, 0.57, 0.0]],
        0.03,
        CARBON,
        FIXED,
    );
    m.tube(
        &[hoop[0], [-0.12, 0.74, -0.24], [-0.16, 0.66, -0.28]],
        0.026,
        CARBON,
        FIXED,
    );
    m.tube(
        &[hoop[16], [-0.12, 0.74, 0.24], [-0.16, 0.66, 0.28]],
        0.026,
        CARBON,
        FIXED,
    );

    // Aileron avant : plan principal, trois volets étagés, dérives.
    m.blade(
        (2.98, 0.075),
        (2.55, 0.085),
        0.025,
        (-0.97, 0.97),
        (CARBON, FIXED),
    );
    for s in [-1.0f32, 1.0] {
        let (z0, z1) = (s * 0.26, s * 0.95);
        m.blade((2.70, 0.115), (2.47, 0.15), 0.02, (z0, z1), (CARBON, FIXED));
        m.blade((2.56, 0.165), (2.38, 0.215), 0.018, (z0, z1), (P, SECOND));
        m.blade((2.45, 0.235), (2.33, 0.285), 0.016, (z0, z1), (P, ACCENT));
        m.hexa(
            [
                [2.99, 0.05, s * 0.95],
                [2.99, 0.05, s * 0.985],
                [2.99, 0.20, s * 0.985],
                [2.99, 0.20, s * 0.95],
                [2.32, 0.05, s * 0.95],
                [2.32, 0.05, s * 0.985],
                [2.32, 0.32, s * 0.985],
                [2.32, 0.32, s * 0.95],
            ],
            P,
            PAINT,
        );
    }
    m.blade(
        (2.62, 0.13),
        (2.55, 0.10),
        0.03,
        (-0.012, 0.012),
        (CARBON, FIXED),
    );

    // Aileron arrière : dérives, plan principal, volet DRS, beam wing, support et feu de pluie.
    for s in [-1.0f32, 1.0] {
        m.hexa(
            [
                [-2.0, 0.42, s * 0.50],
                [-2.0, 0.42, s * 0.53],
                [-2.0, 0.98, s * 0.53],
                [-2.0, 0.98, s * 0.50],
                [-2.62, 0.30, s * 0.50],
                [-2.62, 0.30, s * 0.53],
                [-2.62, 1.0, s * 0.53],
                [-2.62, 1.0, s * 0.50],
            ],
            P,
            PAINT,
        );
    }
    m.blade(
        (-2.12, 0.80),
        (-2.52, 0.86),
        0.035,
        (-0.5, 0.5),
        (CARBON, FIXED),
    );
    m.blade(
        (-2.10, 0.91),
        (-2.34, 0.97),
        0.025,
        (-0.5, 0.5),
        (P, ACCENT),
    );
    m.blade(
        (-2.20, 0.42),
        (-2.45, 0.46),
        0.025,
        (-0.45, 0.45),
        (CARBON, FIXED),
    );
    m.bx([-2.30, 0.30, -0.025], [-2.05, 0.82, 0.025], CARBON, FIXED);
    m.bx(
        [-2.66, 0.30, -0.05],
        [-2.62, 0.36, 0.05],
        [1.0, 0.12, 0.12, 1.0],
        UNLIT,
    );

    // Roues et suspensions.
    for s in [-1.0f32, 1.0] {
        m.wheel([1.75, 0.36, s * 0.84], 0.36, 0.30);
        m.wheel([-1.85, 0.36, s * 0.80], 0.36, 0.40);
        for (a, b) in [
            ([1.45, 0.47, s * 0.22], [1.76, 0.44, s * 0.69]),
            ([1.98, 0.43, s * 0.20], [1.76, 0.44, s * 0.69]),
            ([1.40, 0.20, s * 0.24], [1.74, 0.24, s * 0.69]),
            ([2.00, 0.18, s * 0.20], [1.74, 0.24, s * 0.69]),
            ([-1.50, 0.47, s * 0.26], [-1.84, 0.46, s * 0.62]),
            ([-1.95, 0.42, s * 0.22], [-1.84, 0.46, s * 0.62]),
            ([-1.55, 0.20, s * 0.30], [-1.86, 0.24, s * 0.62]),
        ] {
            m.tube(&[a, b], 0.016, CARBON, FIXED);
        }
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
const CAR_SCALE: f32 = 5.0;

fn speed_rgba(ratio: f32) -> Rgba {
    // Même rampe que la carte 2D : #b3261e → #ffe4de.
    let r = ratio.clamp(0.0, 1.0);
    let l = |a: f32, b: f32| (a + (b - a) * r) / 255.0;
    [l(179.0, 255.0), l(38.0, 228.0), l(30.0, 222.0), 0.12]
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
    let floor = [0.085, 0.085, 0.105, 0.05];
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
    let grid = [0.15, 0.15, 0.18, 0.05];
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
    let wall = [0.20, 0.20, 0.24, 0.1];
    let kerb = [0.62, 0.62, 0.68, 0.1];
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
        // Liserés le long des bords, et remblai jusqu'au sol : le relief se lit comme un volume.
        for (a, b, s) in [(li, lj, 1.0f32), (ri, rj, -1.0)] {
            let (oa, ob) = (add(a, mul(si, s * 1.1)), add(b, mul(sj, s * 1.1)));
            m.face(&[a, b, ob, oa], below, kerb, FIXED);
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
uniform vec3 u_second;
uniform vec3 u_accent;
out vec3 v_nrm;
out vec4 v_col;
out float v_unlit;
out vec3 v_world;
void main() {
  vec3 c = a_col.rgb;
  if (a_kind > 0.5 && a_kind < 1.5) c = u_paint;
  else if (a_kind > 1.5 && a_kind < 2.5) c = u_accent;
  else if (a_kind > 3.5) c = u_second;
  v_col = vec4(c, a_col.a);
  v_unlit = (a_kind > 2.5 && a_kind < 3.5) ? 1.0 : 0.0;
  v_nrm = mat3(u_model) * a_nrm;
  v_world = (u_model * vec4(a_pos, 1.0)).xyz;
  gl_Position = u_mvp * vec4(a_pos, 1.0);
}"#;

/// Éclairage « studio » : lumière principale, contre-jour, ciel/sol, reflets de vernis
/// (Fresnel + environnement), puis compression des hautes lumières et correction gamma.
const FS: &str = r#"#version 300 es
precision highp float;
in vec3 v_nrm;
in vec4 v_col;
in float v_unlit;
in vec3 v_world;
uniform vec3 u_light;
uniform vec3 u_eye;
uniform vec4 u_fog;
out vec4 o;
void main() {
  if (v_unlit > 0.5) { o = v_col; return; }
  vec3 base = pow(v_col.rgb, vec3(2.2));
  float gloss = v_col.a;
  vec3 n = normalize(v_nrm);
  vec3 v = normalize(u_eye - v_world);
  vec3 fill = normalize(vec3(-0.6, 0.35, -0.55));
  float d1 = max(dot(n, u_light), 0.0);
  float d2 = max(dot(n, fill), 0.0);
  vec3 hemi = mix(vec3(0.035, 0.035, 0.045), vec3(0.30, 0.32, 0.38), n.y * 0.5 + 0.5);
  vec3 col = base * (hemi + vec3(1.05, 1.0, 0.94) * d1 + vec3(0.30, 0.36, 0.48) * d2);
  float shine = mix(6.0, 140.0, gloss);
  float g2 = gloss * gloss;
  float spec = pow(max(dot(n, normalize(u_light + v)), 0.0), shine) * g2 * 1.4;
  spec += pow(max(dot(n, normalize(fill + v)), 0.0), shine) * g2 * 0.3;
  vec3 r = reflect(-v, n);
  vec3 env = mix(vec3(0.02, 0.02, 0.03), vec3(0.42, 0.46, 0.55), smoothstep(-0.15, 0.45, r.y));
  env += vec3(1.2) * smoothstep(0.80, 0.93, r.y) * smoothstep(0.9, 0.2, abs(r.x));
  float fres = 0.04 + 0.96 * pow(1.0 - max(dot(n, v), 0.0), 5.0);
  col += vec3(spec) + env * fres * g2;
  col = col / (col + vec3(0.55)) * 1.55;
  vec3 outc = pow(col, vec3(1.0 / 2.2));
  float fd = length(u_eye - v_world) * u_fog.a;
  o = vec4(mix(outc, u_fog.rgb, 1.0 - exp(-fd * fd)), 1.0);
}"#;

struct Uniforms {
    mvp: Option<WebGlUniformLocation>,
    model: Option<WebGlUniformLocation>,
    paint: Option<WebGlUniformLocation>,
    second: Option<WebGlUniformLocation>,
    accent: Option<WebGlUniformLocation>,
    light: Option<WebGlUniformLocation>,
    eye: Option<WebGlUniformLocation>,
    fog: Option<WebGlUniformLocation>,
}

struct Gpu {
    gl: Gl,
    prog: WebGlProgram,
    u: Uniforms,
}

struct Vao {
    vao: WebGlVertexArrayObject,
    count: i32,
    /// Dessin indexé (indices u32) plutôt que des triangles à plat.
    indexed: bool,
}

/// Couleurs d'une livrée (RVB 0–1).
#[derive(Clone, Copy, PartialEq)]
struct Paint {
    primary: [f32; 3],
    second: [f32; 3],
    accent: [f32; 3],
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
            second: loc("u_second"),
            accent: loc("u_accent"),
            light: loc("u_light"),
            eye: loc("u_eye"),
            fog: loc("u_fog"),
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
            indexed: false,
        })
    }

    /// Maillage indexé (modèle de monoplace détaillé).
    fn upload_indexed(&self, verts: &[f32], indices: &[u32]) -> Option<Vao> {
        let gl = &self.gl;
        let vao = gl.create_vertex_array()?;
        gl.bind_vertex_array(Some(&vao));
        let buf = gl.create_buffer()?;
        gl.bind_buffer(Gl::ARRAY_BUFFER, Some(&buf));
        let data = js_sys::Float32Array::from(verts);
        gl.buffer_data_with_array_buffer_view(Gl::ARRAY_BUFFER, &data, Gl::STATIC_DRAW);
        let stride = (STRIDE * 4) as i32;
        for (index, size, offset) in [(0, 3, 0), (1, 3, 3), (2, 4, 6), (3, 1, 10)] {
            gl.enable_vertex_attrib_array(index);
            gl.vertex_attrib_pointer_with_i32(index, size, Gl::FLOAT, false, stride, offset * 4);
        }
        let ibuf = gl.create_buffer()?;
        gl.bind_buffer(Gl::ELEMENT_ARRAY_BUFFER, Some(&ibuf));
        let idx = js_sys::Uint32Array::from(indices);
        gl.buffer_data_with_array_buffer_view(Gl::ELEMENT_ARRAY_BUFFER, &idx, Gl::STATIC_DRAW);
        gl.bind_vertex_array(None);
        Some(Vao {
            vao,
            count: indices.len() as i32,
            indexed: true,
        })
    }

    fn draw(&self, vao: &Vao, vp: &M4, model: &M4, paint: &Paint) {
        let gl = &self.gl;
        gl.uniform_matrix4fv_with_f32_array(self.u.mvp.as_ref(), false, &vp.mul(model).0);
        gl.uniform_matrix4fv_with_f32_array(self.u.model.as_ref(), false, &model.0);
        for (loc, c) in [
            (&self.u.paint, paint.primary),
            (&self.u.second, paint.second),
            (&self.u.accent, paint.accent),
        ] {
            gl.uniform3f(loc.as_ref(), c[0], c[1], c[2]);
        }
        gl.bind_vertex_array(Some(&vao.vao));
        if vao.indexed {
            gl.draw_elements_with_i32(Gl::TRIANGLES, vao.count, Gl::UNSIGNED_INT, 0);
        } else {
            gl.draw_arrays(Gl::TRIANGLES, 0, vao.count);
        }
    }
}

// ---------- Modèle détaillé (fichier partagé avec l'app iOS) ----------

/// Décode `car.bin` (voir tools/carmodel/build.py) en sommets (format STRIDE) et indices.
fn parse_car(b: &[u8]) -> Option<CarData> {
    let rd = |o: usize, n: usize| b.get(o..o + n);
    if rd(0, 4)? != b"F1XC" {
        return None;
    }
    let u32_at = |o: usize| Some(u32::from_le_bytes(rd(o, 4)?.try_into().ok()?));
    // Version 1 : échelle fixe (monoplace) ; version 2 : échelle en tête (décor de circuit).
    let (scale, groups, mut off) = if u32_at(4)? >= 2 {
        let sc = f32::from_le_bytes(rd(8, 4)?.try_into().ok()?);
        (sc, u32_at(12)? as usize, 16)
    } else {
        (4096.0, u32_at(8)? as usize, 12)
    };
    let (mut verts, mut indices) = (Vec::new(), Vec::new());
    for _ in 0..groups {
        let h = rd(off, 8)?;
        let kind = match h[0] {
            1 => PAINT,
            2 => SECOND,
            3 => ACCENT,
            _ => FIXED,
        };
        let colour = [
            h[1] as f32 / 255.0,
            h[2] as f32 / 255.0,
            h[3] as f32 / 255.0,
            h[4] as f32 / 100.0,
        ];
        let nv = u32_at(off + 8)? as usize;
        let ni = u32_at(off + 12)? as usize;
        off += 16;
        let base = (verts.len() / STRIDE) as u32;
        let pos = rd(off, nv * 6)?;
        let nrm = rd(off + nv * 6, nv * 3)?;
        for i in 0..nv {
            for k in 0..3 {
                let v = i16::from_le_bytes([pos[i * 6 + k * 2], pos[i * 6 + k * 2 + 1]]);
                verts.push(v as f32 / scale);
            }
            for k in 0..3 {
                verts.push(nrm[i * 3 + k] as i8 as f32 / 127.0);
            }
            verts.extend_from_slice(&colour);
            verts.push(kind);
        }
        off += nv * 9;
        let idx = rd(off, ni * 2)?;
        for i in 0..ni {
            indices.push(base + u16::from_le_bytes([idx[i * 2], idx[i * 2 + 1]]) as u32);
        }
        off += ni * 2;
    }
    Some((verts, indices))
}

/// Sommets (format STRIDE) et indices du modèle détaillé.
type CarData = (Vec<f32>, Vec<u32>);

thread_local! {
    static CAR_FILE: RefCell<Option<Rc<CarData>>> = const { RefCell::new(None) };
}

/// Télécharge (une fois) le modèle détaillé ; adresse fournie par la page (`window.__f1xCar`).
async fn load_car() -> Option<Rc<CarData>> {
    if let Some(hit) = CAR_FILE.with(|c| c.borrow().clone()) {
        return Some(hit);
    }
    let url = web_sys::window()
        .and_then(|w| js_sys::Reflect::get(&w, &"__f1xCar".into()).ok())
        .and_then(|v| v.as_string())
        .unwrap_or_else(|| "/static/car.bin".into());
    let bytes = gloo_net::http::Request::get(&url)
        .send()
        .await
        .ok()?
        .binary()
        .await
        .ok()?;
    let parsed = Rc::new(parse_car(&bytes)?);
    CAR_FILE.with(|c| *c.borrow_mut() = Some(parsed.clone()));
    Some(parsed)
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

/// Livrée déduite d'une seule couleur (voitures du direct) : bas assombri, accent clair.
fn paint_of(c: [f32; 3]) -> Paint {
    Paint {
        primary: c,
        second: c.map(|v| v * 0.28),
        accent: c.map(|v| v * 0.35 + 0.65),
    }
}

/// Livrée stylisée de chaque écurie : couleur principale, secondaire (bas de caisse, volets)
/// et accent (casque, enjoliveurs, volet DRS). Couleurs seulement, aucun logo.
pub fn livery(constructor_id: &str) -> Livery {
    let (p, s, a) = match constructor_id {
        "mercedes" => ("#A3AAB1", "#141618", "#27F4D2"),
        "ferrari" => ("#D8001F", "#141414", "#F2F2F2"),
        "red_bull" => ("#1C2A63", "#0E1636", "#E2141C"),
        "mclaren" => ("#FF8000", "#1E1E20", "#56C3F0"),
        "aston_martin" => ("#00594F", "#0A2E29", "#CEDC00"),
        "alpine" => ("#1667C9", "#0B1A3A", "#FF87BC"),
        "williams" => ("#0C2D6B", "#06173A", "#64C4FF"),
        "rb" => ("#F1F2F4", "#1634CB", "#E1061B"),
        "haas" => ("#ECEDEF", "#1A1A1C", "#E10600"),
        "sauber" => ("#1B1B1D", "#0E0E0F", "#52E252"),
        "audi" => ("#A9ADB1", "#151517", "#F50537"),
        "cadillac" => ("#151517", "#E7E7EA", "#C9A96E"),
        other => {
            let c = crate::util::team_color(other);
            return Livery::from_colour(c);
        }
    };
    Livery {
        primary: p.into(),
        second: s.into(),
        accent: a.into(),
    }
}

/// Livrée (couleurs hexadécimales) passée au composant.
#[derive(Clone, PartialEq)]
pub struct Livery {
    pub primary: AttrValue,
    pub second: AttrValue,
    pub accent: AttrValue,
}

impl Livery {
    pub fn from_colour(hex: &str) -> Livery {
        let p = paint_of(parse_colour(hex));
        let to_hex = |c: [f32; 3]| {
            AttrValue::from(format!(
                "#{:02x}{:02x}{:02x}",
                (c[0] * 255.0) as u8,
                (c[1] * 255.0) as u8,
                (c[2] * 255.0) as u8
            ))
        };
        Livery {
            primary: to_hex(p.primary),
            second: to_hex(p.second),
            accent: to_hex(p.accent),
        }
    }

    fn paint(&self) -> Paint {
        Paint {
            primary: parse_colour(&self.primary),
            second: parse_colour(&self.second),
            accent: parse_colour(&self.accent),
        }
    }
}

// ---------- Scène ----------

/// Ce que montre la vue 3D.
#[derive(Clone, PartialEq)]
pub enum Scene {
    /// Monoplace dans une livrée.
    Car(Livery),
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
    paint: Paint,
    target: f32,
    cur: f32,
    label: Option<HtmlElement>,
}

struct Cam {
    yaw: f32,
    pitch: f32,
    zoom: f32,
    /// Décalage du point visé (déplacement à deux doigts).
    pan: V3,
}

impl Cam {
    fn initial(track: bool) -> Cam {
        Cam {
            yaw: 0.9,
            pitch: if track { 0.82 } else { 0.28 },
            zoom: 1.0,
            pan: [0.0; 3],
        }
    }
}

/// Doigts / pointeurs posés sur la vue : identifiant et dernière position.
type Pointers = Vec<(i32, f64, f64)>;

struct State {
    gpu: Gpu,
    car: Vao,
    shadow: Vao,
    track: Option<(Vao, Path)>,
    paint: Paint,
    ghost: bool,
    chase: bool,
    cam: Cam,
    /// Point de visée lissé de la caméra embarquée (œil, cible).
    chase_cam: Option<(V3, V3)>,
    pointers: Pointers,
    /// L'utilisateur a pris la main : plus de rotation automatique.
    touched: bool,
    last_tap: f64,
    /// Distance caméra–cible et hauteur de la vue (px), pour convertir les gestes.
    dist: f32,
    view_h: f32,
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
        self.view_h = rect.height() as f32;
        let aspect = rect.width() as f32 / rect.height().max(1.0) as f32;
        if !self.touched {
            let speed = if self.track.is_some() { 0.07 } else { 0.3 };
            self.cam.yaw += speed * dt;
        }

        let fov = 0.62f32;
        let (eye, target, near, far) = match &self.track {
            None => {
                let dist = 3.0 / ((fov / 2.0).tan() * aspect.min(1.8))
                    * self.cam.zoom
                    * if aspect < 0.8 { 0.85 } else { 1.0 };
                self.dist = dist;
                let target = add([0.0, 0.35, 0.0], self.cam.pan);
                (
                    orbit(target, self.cam.yaw, self.cam.pitch, dist),
                    target,
                    0.05,
                    80.0,
                )
            }
            Some((_, path)) => {
                let dist =
                    path.radius / (fov / 2.0).sin() * 1.08 / aspect.min(1.0).sqrt() * self.cam.zoom;
                self.dist = dist;
                if self.chase && self.ghost {
                    let (pos, heading) = path.at_time(self.clock as f32);
                    let fwd = [heading.cos(), 0.0, -heading.sin()];
                    let back = 75.0 * self.cam.zoom;
                    let want_eye = add(add(pos, mul(fwd, -back)), [0.0, 26.0 * self.cam.zoom, 0.0]);
                    let want_target = add(add(pos, mul(fwd, 30.0)), [0.0, 4.0, 0.0]);
                    let k = (dt * 4.0).min(1.0);
                    let (e, tg) = self.chase_cam.unwrap_or((want_eye, want_target));
                    let cam = (lerp3(e, want_eye, k), lerp3(tg, want_target, k));
                    self.chase_cam = Some(cam);
                    (cam.0, cam.1, 1.0, path.radius * 6.0)
                } else {
                    self.chase_cam = None;
                    let target = add([0.0, path.centre_y, 0.0], self.cam.pan);
                    (
                        orbit(target, self.cam.yaw, self.cam.pitch, dist),
                        target,
                        dist * 0.01,
                        dist * 4.0 + path.radius * 2.0,
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
        // Brouillard de distance sur les circuits (profondeur), aucun sur la monoplace seule.
        let fog = match &self.track {
            Some((_, path)) => 1.0 / (path.radius * 7.0),
            None => 0.0,
        };
        gl.uniform4f(gpu.u.fog.as_ref(), 0.121, 0.165, 0.243, fog);
        let id = M4::identity();
        match &self.track {
            None => {
                gl.depth_mask(false);
                gpu.draw(&self.shadow, &vp, &id, &self.paint);
                gl.depth_mask(true);
                gpu.draw(&self.car, &vp, &id, &self.paint);
            }
            Some((vao, path)) => {
                gpu.draw(vao, &vp, &id, &self.paint);
                if self.ghost {
                    let (pos, heading) = path.at_time(self.clock as f32);
                    let model = M4::trs(add(pos, [0.0, 0.6, 0.0]), heading, CAR_SCALE);
                    gpu.draw(&self.car, &vp, &model, &self.paint);
                }
                let (cw, ch) = (rect.width() as f32, rect.height() as f32);
                for car in &mut self.cars {
                    // Avance en douceur vers la dernière position connue (tour bouclé).
                    let diff = (car.target - car.cur + 0.5).rem_euclid(1.0) - 0.5;
                    car.cur = (car.cur + diff * (dt * 2.5).min(1.0)).rem_euclid(1.0);
                    let (pos, heading) = path.at_time(car.cur * path.lap_time);
                    let model = M4::trs(add(pos, [0.0, 0.6, 0.0]), heading, CAR_SCALE);
                    gpu.draw(&self.car, &vp, &model, &car.paint);
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

    /// Décor détaillé du circuit (calculé par le serveur), remplace le tracé simplifié.
    fn set_scenery(&mut self, data: &CarData) {
        if let Some(vao) = self.gpu.upload_indexed(&data.0, &data.1) {
            if let Some(t) = self.track.as_mut() {
                t.0 = vao;
            }
        }
    }

    fn set_car(&mut self, car: &CarData) {
        if let Some(vao) = self.gpu.upload_indexed(&car.0, &car.1) {
            self.car = vao;
        }
    }

    fn set_markers(&mut self, markers: &[Marker]) {
        let doc = web_sys::window().and_then(|w| w.document());
        let mut next = Vec::with_capacity(markers.len());
        for m in markers {
            let paint = paint_of(parse_colour(&m.colour));
            match self.cars.iter().position(|c| c.key == m.key) {
                Some(i) => {
                    let mut car = self.cars.swap_remove(i);
                    car.target = m.fraction;
                    car.paint = paint;
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
                        paint,
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

    // ----- Gestes -----

    fn pointer_down(&mut self, id: i32, x: f64, y: f64) {
        self.touched = true;
        self.pointers.retain(|p| p.0 != id);
        self.pointers.push((id, x, y));
        if self.pointers.len() == 1 {
            // Double toucher : recentrer la vue.
            let t = now();
            if t - self.last_tap < 320.0 {
                self.reset();
            }
            self.last_tap = t;
        }
    }

    fn pointer_move(&mut self, id: i32, x: f64, y: f64) {
        let Some(i) = self.pointers.iter().position(|p| p.0 == id) else {
            return;
        };
        if self.pointers.len() == 1 {
            let (_, px, py) = self.pointers[0];
            self.cam.yaw += ((x - px) * 0.008) as f32;
            let (lo, hi) = if self.track.is_some() {
                (0.12, 1.5)
            } else {
                (-0.05, 1.45)
            };
            self.cam.pitch = (self.cam.pitch + ((y - py) * 0.006) as f32).clamp(lo, hi);
            self.pointers[0] = (id, x, y);
            return;
        }
        // Deux doigts : pincer pour zoomer, glisser pour déplacer.
        let before = (self.pointers[0], self.pointers[1]);
        self.pointers[i] = (id, x, y);
        let after = (self.pointers[0], self.pointers[1]);
        let span = |a: (i32, f64, f64), b: (i32, f64, f64)| ((a.1 - b.1).hypot(a.2 - b.2)).max(1.0);
        let mid = |a: (i32, f64, f64), b: (i32, f64, f64)| ((a.1 + b.1) / 2.0, (a.2 + b.2) / 2.0);
        let ratio = span(before.0, before.1) / span(after.0, after.1);
        self.zoom_by(ratio as f32);
        let (m0, m1) = (mid(before.0, before.1), mid(after.0, after.1));
        self.pan_by((m1.0 - m0.0) as f32, (m1.1 - m0.1) as f32);
    }

    fn pointer_up(&mut self, id: i32) {
        self.pointers.retain(|p| p.0 != id);
    }

    fn zoom_by(&mut self, k: f32) {
        self.touched = true;
        self.cam.zoom = (self.cam.zoom * k).clamp(0.2, 3.0);
    }

    /// Déplace le point visé dans le plan horizontal, au rythme du doigt.
    fn pan_by(&mut self, dx: f32, dy: f32) {
        let px = self.dist * 2.0 * (0.31f32).tan() / self.view_h.max(1.0);
        let (s, c) = self.cam.yaw.sin_cos();
        let right = [-s, 0.0, c];
        let fwd = [-c, 0.0, -s];
        let delta = add(mul(right, -dx * px), mul(fwd, dy * px));
        let limit = match &self.track {
            Some((_, path)) => path.radius,
            None => 3.0,
        };
        let pan = add(self.cam.pan, delta);
        let len = dot(pan, pan).sqrt();
        self.cam.pan = if len > limit {
            mul(pan, limit / len)
        } else {
            pan
        };
    }

    fn reset(&mut self) {
        self.cam = Cam::initial(self.track.is_some());
        self.touched = false;
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
        let car = match CAR_FILE.with(|c| c.borrow().clone()) {
            Some(file) => gpu.upload_indexed(&file.0, &file.1)?,
            None => gpu.upload(&car_mesh())?,
        };
        let mut sh = Mesh::default();
        sh.shadow([0.0, 0.002, 0.0], 3.4, 1.4);
        let shadow = gpu.upload(&sh)?;
        let (paint, track, ghost) = match scene {
            Scene::Car(livery) => (livery.paint(), None, false),
            Scene::Track { map, ghost } => {
                let (mesh, path) = track_mesh(map)?;
                (
                    paint_of(parse_colour(&map.colour)),
                    Some((gpu.upload(&mesh)?, path)),
                    *ghost,
                )
            }
        };
        let t = now();
        let state = Rc::new(RefCell::new(State {
            gpu,
            car,
            shadow,
            cam: Cam::initial(track.is_some()),
            track,
            paint,
            ghost,
            chase: false,
            chase_cam: None,
            pointers: Vec::new(),
            touched: false,
            last_tap: 0.0,
            dist: 1.0,
            view_h: 1.0,
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

/// Bloque le défilement de la page tant que la vue est en plein écran.
fn lock_scroll(lock: bool) {
    if let Some(body) = web_sys::window()
        .and_then(|w| w.document())
        .and_then(|d| d.body())
    {
        let _ = body
            .style()
            .set_property("overflow", if lock { "hidden" } else { "" });
    }
}

/// Vue 3D interactive.
/// - Dans la page : glisser horizontalement pour tourner (le défilement vertical reste libre).
/// - En plein écran : un doigt pour tourner, deux doigts pour zoomer et déplacer,
///   double toucher pour recentrer. Molette / pincement du pavé tactile sur ordinateur.
#[function_component]
pub fn View3D(props: &ViewProps) -> Html {
    let canvas = use_node_ref();
    let labels = use_node_ref();
    let viewer = use_mut_ref(|| None::<Viewer>);
    let failed = use_state(|| false);
    let chase = use_state(|| false);
    let full = use_state(|| false);
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
            if let Scene::Track { map, .. } = scene {
                // Décor du circuit : chargé en arrière-plan, remplace le tracé simplifié.
                let viewer = viewer.clone();
                let url = format!("/api/track3d/{}", map.circuit_id);
                wasm_bindgen_futures::spawn_local(async move {
                    let Ok(resp) = gloo_net::http::Request::get(&url).send().await else {
                        return;
                    };
                    if !resp.ok() {
                        return;
                    }
                    let Ok(bytes) = resp.binary().await else {
                        return;
                    };
                    if let Some(data) = parse_car(&bytes) {
                        if let Some(v) = viewer.borrow().as_ref() {
                            v.state.borrow_mut().set_scenery(&data);
                        }
                    }
                });
            }
            {
                // Modèle détaillé : chargé en arrière-plan, remplace la version simplifiée.
                let viewer = viewer.clone();
                wasm_bindgen_futures::spawn_local(async move {
                    if let Some(car) = load_car().await {
                        if let Some(v) = viewer.borrow().as_ref() {
                            v.state.borrow_mut().set_car(&car);
                        }
                    }
                });
            }
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
    use_effect_with(*full, |on| {
        lock_scroll(*on);
        || lock_scroll(false)
    });
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
            let (id, x, y) = (e.pointer_id(), e.client_x() as f64, e.client_y() as f64);
            with(Box::new(move |s| s.pointer_down(id, x, y)));
        })
    };
    let onpointermove = {
        let with = with.clone();
        Callback::from(move |e: PointerEvent| {
            let (id, x, y) = (e.pointer_id(), e.client_x() as f64, e.client_y() as f64);
            with(Box::new(move |s| s.pointer_move(id, x, y)));
        })
    };
    let onpointerup = {
        let with = with.clone();
        Callback::from(move |e: PointerEvent| {
            let id = e.pointer_id();
            with(Box::new(move |s| s.pointer_up(id)));
        })
    };
    let onwheel = {
        let with = with.clone();
        let full = *full;
        Callback::from(move |e: WheelEvent| {
            // Dans la page, la molette fait défiler ; elle zoome en plein écran (ou au pincement
            // du pavé tactile, signalé par ctrlKey).
            if full || e.ctrl_key() {
                e.prevent_default();
                let k = (e.delta_y() as f32 * 0.0015).exp();
                with(Box::new(move |s| s.zoom_by(k)));
            }
        })
    };
    let zoom = |k: f32| {
        let with = with.clone();
        Callback::from(move |_: MouseEvent| with(Box::new(move |s| s.zoom_by(k))))
    };
    let recentre = {
        let with = with.clone();
        Callback::from(move |_: MouseEvent| with(Box::new(|s| s.reset())))
    };
    let toggle_full = {
        let full = full.clone();
        Callback::from(move |_: MouseEvent| full.set(!*full))
    };
    let is_track = matches!(props.scene, Scene::Track { .. });
    let ghost = matches!(props.scene, Scene::Track { ghost: true, .. });
    if *failed {
        return html! {
            <p class="muted">{ t("La 3D n'est pas disponible sur cet appareil (WebGL 2 requis).", "3D isn't available on this device (WebGL 2 required).") }</p>
        };
    }
    html! {
        <div class={classes!("scene", is_track.then_some("scene-track"), full.then_some("scene-full"))}>
            <canvas ref={canvas} class="scene-canvas" role="img"
                aria-label={if is_track { t("Circuit en 3D", "3D circuit") } else { t("Monoplace en 3D", "3D car") }}
                {onpointerdown} {onpointermove} onpointerup={onpointerup.clone()} onpointercancel={onpointerup} {onwheel} />
            <div ref={labels} class="scene-labels" aria-hidden="true"></div>
            if *full {
                <p class="scene-hint">{ t(
                    "1 doigt : tourner · 2 doigts : zoomer et déplacer · double toucher : recentrer",
                    "1 finger: rotate · 2 fingers: zoom and move · double tap: recentre",
                ) }</p>
            }
            <div class="scene-tools">
                if ghost {
                    <button class={classes!("scene-btn", chase.then_some("on"))} aria-pressed={chase.to_string()}
                        onclick={let chase = chase.clone(); move |_| chase.set(!*chase)}>
                        { if *chase { t("Vue d'ensemble", "Overview") } else { t("Caméra embarquée", "Onboard camera") } }
                    </button>
                }
                if *full {
                    <button class="scene-btn" aria-label={t("Recentrer", "Recentre")} onclick={recentre}>{ "⟲" }</button>
                    <button class="scene-btn" aria-label={t("Rapprocher", "Zoom in")} onclick={zoom(0.8)}>{ "+" }</button>
                    <button class="scene-btn" aria-label={t("Éloigner", "Zoom out")} onclick={zoom(1.25)}>{ "−" }</button>
                    <button class="scene-btn on" aria-label={t("Quitter le plein écran", "Exit full screen")} onclick={toggle_full}>{ "✕" }</button>
                } else {
                    <button class="scene-btn" onclick={toggle_full}>{ t("⛶ Plein écran", "⛶ Full screen") }</button>
                }
            </div>
        </div>
    }
}

/// Carte « monoplace en 3D » aux couleurs d'une écurie.
pub fn car_card(constructor_id: &str, team: &str) -> Html {
    html! {
        <section class="card">
            <h2>{ t("La monoplace en 3D", "The car in 3D") }</h2>
            <View3D scene={Scene::Car(livery(constructor_id))} />
            <p class="muted">{ crate::tr!(
                "Monoplace stylisée aux couleurs {} — modèle généré par le code, pas une reproduction officielle. Glisse pour la faire tourner, ou passe en plein écran pour zoomer et la déplacer à deux doigts.",
                "Stylised car in {} colours — generated by code, not an official replica. Drag to spin it, or go full screen to zoom and move it with two fingers.",
                team
            ) }</p>
        </section>
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
