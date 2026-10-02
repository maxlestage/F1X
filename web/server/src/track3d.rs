//! Décor 3D d'un circuit, calculé une fois par le serveur à partir du tracé GPS OpenF1 et
//! partagé par le site (WebGL) et l'app iOS (SceneKit) : relief, herbe, asphalte, lignes
//! blanches, vibreurs, bacs à graviers, rails, grille de départ, portique, tribunes, stands, arbres.
//!
//! Même format binaire que la monoplace (`F1XC`, version 2 : échelle des positions en tête).

use std::collections::HashMap;

use f1x_protocol::TrackMap;

/// Mêmes constantes que les clients (repère : x − cx, altitude × RELIEF, y − cz).
pub const RELIEF: f32 = 2.5;
pub const HALF_WIDTH: f32 = 12.0;

type V = [f32; 3];
/// (nature, r, g, b, brillance) — nature 0 : couleur fixe.
type Mat = (u8, u8, u8, u8, u8);

fn mat(hex: u32, gloss: u8) -> Mat {
    (0, (hex >> 16) as u8, (hex >> 8) as u8, hex as u8, gloss)
}

fn add(a: V, b: V) -> V {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}
fn sub(a: V, b: V) -> V {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn mul(a: V, k: f32) -> V {
    [a[0] * k, a[1] * k, a[2] * k]
}
fn cross(a: V, b: V) -> V {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}
fn norm(a: V) -> V {
    let l = (a[0] * a[0] + a[1] * a[1] + a[2] * a[2]).sqrt();
    if l < 1e-9 {
        [0.0, 1.0, 0.0]
    } else {
        mul(a, 1.0 / l)
    }
}

struct Group {
    mat: Mat,
    pos: Vec<V>,
    nrm: Vec<V>,
    idx: Vec<u16>,
}

#[derive(Default)]
struct Builder {
    groups: Vec<Group>,
    current: HashMap<Mat, usize>,
}

impl Builder {
    /// Groupe du matériau avec de la place pour `n` sommets (indices sur 16 bits).
    fn group(&mut self, m: Mat, n: usize) -> &mut Group {
        let fits = self
            .current
            .get(&m)
            .is_some_and(|&g| self.groups[g].pos.len() + n < 65_000);
        if !fits {
            self.groups.push(Group {
                mat: m,
                pos: Vec::new(),
                nrm: Vec::new(),
                idx: Vec::new(),
            });
            self.current.insert(m, self.groups.len() - 1);
        }
        let g = self.current[&m];
        &mut self.groups[g]
    }

    /// Quadrilatère plan, normale orientée vers `up` (ou vers l'extérieur indiqué).
    fn quad(&mut self, m: Mat, p: [V; 4], towards: V) {
        let mut n = norm(cross(sub(p[1], p[0]), sub(p[2], p[0])));
        let flip = n[0] * towards[0] + n[1] * towards[1] + n[2] * towards[2] < 0.0;
        if flip {
            n = mul(n, -1.0);
        }
        let g = self.group(m, 4);
        let base = g.pos.len() as u16;
        g.pos.extend_from_slice(&p);
        g.nrm.extend_from_slice(&[n; 4]);
        if flip {
            g.idx
                .extend_from_slice(&[base, base + 2, base + 1, base, base + 3, base + 2]);
        } else {
            g.idx
                .extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
        }
    }

    /// Pavé orienté : centre, axes (avant, côté, haut) et demi-dimensions.
    fn block(&mut self, m: Mat, c: V, ax: [V; 3], h: V) {
        let corner = |a: f32, b: f32, d: f32| {
            add(
                c,
                add(
                    mul(ax[0], a * h[0]),
                    add(mul(ax[1], b * h[1]), mul(ax[2], d * h[2])),
                ),
            )
        };
        for (dir, i) in [
            (ax[0], 0usize),
            (mul(ax[0], -1.0), 0),
            (ax[1], 1),
            (mul(ax[1], -1.0), 1),
            (ax[2], 2),
            (mul(ax[2], -1.0), 2),
        ] {
            let s = if dir == ax[i] { 1.0 } else { -1.0 };
            let p = match i {
                0 => [
                    corner(s, -1.0, -1.0),
                    corner(s, 1.0, -1.0),
                    corner(s, 1.0, 1.0),
                    corner(s, -1.0, 1.0),
                ],
                1 => [
                    corner(-1.0, s, -1.0),
                    corner(1.0, s, -1.0),
                    corner(1.0, s, 1.0),
                    corner(-1.0, s, 1.0),
                ],
                _ => [
                    corner(-1.0, -1.0, s),
                    corner(1.0, -1.0, s),
                    corner(1.0, 1.0, s),
                    corner(-1.0, 1.0, s),
                ],
            };
            self.quad(m, p, dir);
        }
    }

    /// Cône (arbres) : base au centre `c`, rayon `r`, hauteur `h`.
    fn cone(&mut self, m: Mat, c: V, r: f32, h: f32) {
        let sides = 7;
        let top = add(c, [0.0, h, 0.0]);
        for k in 0..sides {
            let a0 = std::f32::consts::TAU * k as f32 / sides as f32;
            let a1 = std::f32::consts::TAU * (k + 1) as f32 / sides as f32;
            let p0 = add(c, [a0.cos() * r, 0.0, a0.sin() * r]);
            let p1 = add(c, [a1.cos() * r, 0.0, a1.sin() * r]);
            let mid = [((a0 + a1) / 2.0).cos(), r / h, ((a0 + a1) / 2.0).sin()];
            self.quad(m, [p0, p1, top, top], mid);
        }
    }

    /// Boule lissée (feuillus) : centre, rayons horizontal et vertical.
    fn blob(&mut self, m: Mat, c: V, r: f32, ry: f32) {
        let (rings, segs) = (5usize, 8usize);
        let g = self.group(m, (rings + 1) * (segs + 1));
        let base = g.pos.len() as u16;
        for i in 0..=rings {
            let phi = std::f32::consts::PI * i as f32 / rings as f32;
            for k in 0..=segs {
                let th = std::f32::consts::TAU * k as f32 / segs as f32;
                let d = [phi.sin() * th.cos(), phi.cos(), phi.sin() * th.sin()];
                g.pos.push(add(c, [d[0] * r, d[1] * ry, d[2] * r]));
                g.nrm.push(norm([d[0] / r, d[1] / ry, d[2] / r]));
            }
        }
        let w = (segs + 1) as u16;
        for i in 0..rings as u16 {
            for k in 0..segs as u16 {
                let a = base + i * w + k;
                g.idx
                    .extend_from_slice(&[a, a + 1, a + w, a + 1, a + w + 1, a + w]);
            }
        }
    }

    /// Panneau vertical entre deux points au sol, de `y0` à `y1`, visible des deux côtés.
    fn wall(&mut self, m: Mat, a: V, b: V, y0: f32, y1: f32, out: V) {
        let p = [
            add(a, [0.0, y0, 0.0]),
            add(b, [0.0, y0, 0.0]),
            add(b, [0.0, y1, 0.0]),
            add(a, [0.0, y1, 0.0]),
        ];
        self.quad(m, p, out);
        self.quad(m, p, mul(out, -1.0));
    }

    fn export(&self) -> Vec<u8> {
        let mut max = 1.0f32;
        for g in &self.groups {
            for p in &g.pos {
                for c in p {
                    max = max.max(c.abs());
                }
            }
        }
        let scale = 32_000.0 / max;
        let mut out = b"F1XC".to_vec();
        out.extend_from_slice(&2u32.to_le_bytes());
        out.extend_from_slice(&scale.to_le_bytes());
        out.extend_from_slice(&(self.groups.len() as u32).to_le_bytes());
        for g in &self.groups {
            let (k, r, gg, b, gloss) = g.mat;
            out.extend_from_slice(&[k, r, gg, b, gloss, 0, 0, 0]);
            out.extend_from_slice(&(g.pos.len() as u32).to_le_bytes());
            out.extend_from_slice(&(g.idx.len() as u32).to_le_bytes());
            for p in &g.pos {
                for c in p {
                    out.extend_from_slice(
                        &((c * scale).round().clamp(-32767.0, 32767.0) as i16).to_le_bytes(),
                    );
                }
            }
            for n in &g.nrm {
                for c in n {
                    out.push(((c * 127.0).round().clamp(-127.0, 127.0) as i8) as u8);
                }
            }
            for i in &g.idx {
                out.extend_from_slice(&i.to_le_bytes());
            }
        }
        out
    }
}

/// Générateur pseudo-aléatoire déterministe (même décor à chaque fois).
struct Rng(u64);
impl Rng {
    fn next(&mut self) -> f32 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        ((self.0 >> 33) as f32) / (u32::MAX >> 1) as f32
    }
}

pub fn build(map: &TrackMap) -> Vec<u8> {
    let raw = &map.points;
    let mut b = Builder::default();
    if raw.len() < 8 {
        return b.export();
    }
    let (cx, cz) = (map.width as f32 / 2.0, map.height as f32 / 2.0);
    let pts: Vec<V> = raw
        .iter()
        .map(|p| [p.x - cx, p.z * RELIEF, p.y - cz])
        .collect();
    let n = pts.len();
    let d0 = sub(pts[0], pts[n - 1]);
    let closed = (d0[0] * d0[0] + d0[2] * d0[2]).sqrt() < 60.0;
    let at = |i: isize| -> usize {
        if closed {
            i.rem_euclid(n as isize) as usize
        } else {
            i.clamp(0, n as isize - 1) as usize
        }
    };
    let tangent = |i: usize| {
        let d = sub(pts[at(i as isize + 1)], pts[at(i as isize - 1)]);
        norm([d[0], 0.0, d[2]])
    };
    let side = |i: usize| norm(cross(tangent(i), [0.0, 1.0, 0.0]));
    // Virage : angle signé entre les directions avant et après le point.
    let turn: Vec<f32> = (0..n)
        .map(|i| {
            let a = tangent(at(i as isize - 3));
            let c = tangent(at(i as isize + 3));
            let s = a[0] * c[2] - a[2] * c[0];
            let d = a[0] * c[0] + a[2] * c[2];
            s.atan2(d)
        })
        .collect();
    let segs = if closed { n } else { n - 1 };
    let up = [0.0, 1.0, 0.0];
    let hw = HALF_WIDTH;
    let off = |i: usize, o: f32, dy: f32| add(add(pts[i], mul(side(i), o)), [0.0, dy, 0.0]);
    // Grille des segments de piste : vérifie qu'un élément de décor ne déborde pas sur une
    // AUTRE portion du circuit (épingles, portions parallèles, croisements).
    let cell = 40.0f32;
    let mut bins: HashMap<(i32, i32), Vec<usize>> = HashMap::new();
    for k in 0..segs {
        let (a, c) = (pts[k], pts[at(k as isize + 1)]);
        let (x0, x1) = (a[0].min(c[0]), a[0].max(c[0]));
        let (z0, z1) = (a[2].min(c[2]), a[2].max(c[2]));
        for bx in (x0 / cell).floor() as i32..=(x1 / cell).floor() as i32 {
            for bz in (z0 / cell).floor() as i32..=(z1 / cell).floor() as i32 {
                bins.entry((bx, bz)).or_default().push(k);
            }
        }
    }
    // Vrai si un point est à moins de `r` d'un segment de piste, en ignorant les segments
    // à moins de `skip` indices de `own` (la portion de piste à laquelle l'élément appartient).
    let crowded = |p: V, r: f32, own: usize, skip: usize| -> bool {
        let reach = (r / cell).ceil() as i32;
        let (bx, bz) = ((p[0] / cell).floor() as i32, (p[2] / cell).floor() as i32);
        for dx in -reach..=reach {
            for dz in -reach..=reach {
                let Some(list) = bins.get(&(bx + dx, bz + dz)) else {
                    continue;
                };
                for &k in list {
                    let gap = (k as isize - own as isize).unsigned_abs();
                    let gap = if closed { gap.min(n - gap) } else { gap };
                    if gap <= skip {
                        continue;
                    }
                    let (a, c) = (pts[k], pts[at(k as isize + 1)]);
                    let (sx, sz) = (c[0] - a[0], c[2] - a[2]);
                    let len2 = (sx * sx + sz * sz).max(1e-6);
                    let t = (((p[0] - a[0]) * sx + (p[2] - a[2]) * sz) / len2).clamp(0.0, 1.0);
                    let (qx, qz) = (a[0] + sx * t - p[0], a[2] + sz * t - p[2]);
                    if qx * qx + qz * qz < r * r {
                        return true;
                    }
                }
            }
        }
        false
    };

    let asphalt = mat(0x2A2C31, 18);
    let white = mat(0xEDEDED, 20);
    let red = mat(0xD3202A, 25);
    let gravel = mat(0xC4AE86, 4);
    let armco = mat(0x9EA3AA, 70);
    let skirt = mat(0x3A3C40, 10);
    let runoff = mat(0x3A3E46, 14);
    let rubber = mat(0x26282C, 14);
    let paint = [mat(0x2F6FB5, 25), mat(0x2E9B57, 25)];
    let tyres = [mat(0x15161A, 10), mat(0xE8E8E8, 15)];
    let (tecpro_a, tecpro_b) = (mat(0x1E4FB5, 35), mat(0xD3202A, 35));
    let fence = mat(0x9AA0A8, 30);
    // Panneaux publicitaires génériques (aplats de couleurs, aucune marque).
    let boards = [
        mat(0x1B1F27, 40),
        mat(0x262B35, 40),
        mat(0x1B1F27, 40),
        mat(0xB8141C, 40),
    ];

    for i in 0..segs {
        let j = at(i as isize + 1);
        let strip = |b: &mut Builder, m: Mat, o0: f32, o1: f32, dy: f32| {
            b.quad(
                m,
                [
                    off(i, o0, dy),
                    off(j, o0, dy),
                    off(j, o1, dy),
                    off(i, o1, dy),
                ],
                up,
            );
        };
        // Asphalte (légères variations de teinte par tronçon), lignes blanches de bord,
        // trajectoire gommée plus sombre (les points sont la trajectoire réelle du pilote).
        // Décalages verticaux nets (≥ 0,12) : les positions sont quantifiées sur 16 bits
        // (~0,03 unité), des marquages plus bas « clignotaient » en dents de scie.
        strip(&mut b, asphalt, -hw, hw, 0.0);
        strip(&mut b, white, hw - 0.9, hw - 0.3, 0.14);
        strip(&mut b, white, -hw + 0.3, -hw + 0.9, 0.14);
        strip(&mut b, rubber, -2.2, 2.2, 0.08);
        // Bordures latérales jusqu'au sol (pas de jour entre la piste et le relief).
        for s in [-1.0f32, 1.0] {
            let (a0, a1) = (off(i, s * hw, 0.0), off(j, s * hw, 0.0));
            b.quad(
                skirt,
                [a0, a1, add(a1, [0.0, -4.0, 0.0]), add(a0, [0.0, -4.0, 0.0])],
                mul(side(i), s),
            );
        }
        // Vibreurs rouges et blancs en relief (bord biseauté), rayures courtes comme en vrai.
        let t = turn[i].abs();
        if t > 0.10 {
            const STRIPES: usize = 4;
            for k in 0..STRIPES {
                let (f0, f1) = (k as f32 / STRIPES as f32, (k + 1) as f32 / STRIPES as f32);
                let m = if (i * STRIPES + k) % 2 == 0 {
                    red
                } else {
                    white
                };
                let at_f = |f: f32, o: f32, dy: f32| {
                    add(mul(off(i, o, dy), 1.0 - f), mul(off(j, o, dy), f))
                };
                for sgn in [-1.0f32, 1.0] {
                    let (o0, o1) = (sgn * hw, sgn * (hw + 2.4));
                    // Pente montante depuis la piste, puis face extérieure jusqu'au sol.
                    b.quad(
                        m,
                        [
                            at_f(f0, o0, 0.12),
                            at_f(f1, o0, 0.12),
                            at_f(f1, o1, 0.3),
                            at_f(f0, o1, 0.3),
                        ],
                        up,
                    );
                    b.quad(
                        m,
                        [
                            at_f(f0, o1, 0.3),
                            at_f(f1, o1, 0.3),
                            at_f(f1, o1, -0.4),
                            at_f(f0, o1, -0.4),
                        ],
                        mul(side(i), sgn),
                    );
                }
            }
        }
        let outside = if turn[i] > 0.0 { -1.0 } else { 1.0 };
        let inward = mul(side(i), -outside);
        // Largeur de dégagement possible sans empiéter sur une autre portion de piste.
        let room = |w: f32| {
            !crowded(off(i, outside * (hw + w), 0.0), hw + 3.0, i, 12)
                && !crowded(off(j, outside * (hw + w), 0.0), hw + 3.0, j, 12)
        };
        let width = [22.0f32, 14.0, 8.0].into_iter().find(|&w| room(w + 2.0));
        if t > 0.10 && width.is_none() {
            // Pas la place (épingle serrée, portion de piste voisine) : vibreur seul.
        } else if t > 0.22 && raw[i].speed < 230 {
            // Virage lent : bac à graviers, mur de pneus rouge et blanc, grillage.
            let w = width.unwrap_or(22.0);
            let (o0, o1) = (outside * (hw + 2.4), outside * (hw + w));
            strip(&mut b, gravel, o0.min(o1), o0.max(o1), 0.1);
            let (r0, r1) = (off(i, o1, 0.0), off(j, o1, 0.0));
            b.wall(tyres[i % 2], r0, r1, 0.0, 1.2, inward);
            b.wall(
                if (i / 2) % 2 == 0 { tecpro_a } else { tecpro_b },
                off(i, o1 + outside * 0.1, 0.0),
                off(j, o1 + outside * 0.1, 0.0),
                1.2,
                1.9,
                inward,
            );
            b.wall(
                fence,
                off(i, o1 + outside * 1.5, 0.0),
                off(j, o1 + outside * 1.5, 0.0),
                1.9,
                3.6,
                inward,
            );
        } else if t > 0.10 {
            // Virage rapide : dégagement asphalté peint (bandes bleues et vertes), rail.
            let w = width.unwrap_or(16.0).min(16.0);
            let (o0, o1) = (outside * (hw + 2.4), outside * (hw + w));
            strip(&mut b, runoff, o0.min(o1), o0.max(o1), 0.1);
            let (p0, p1) = (outside * (hw + 2.4), outside * (hw + 4.4));
            strip(&mut b, paint[(i / 3) % 2], p0.min(p1), p0.max(p1), 0.2);
            b.wall(armco, off(i, o1, 0.0), off(j, o1, 0.0), 0.3, 1.3, inward);
        } else {
            // Ligne droite : rail, panneaux publicitaires génériques des deux côtés.
            for sgn in [-1.0f32, 1.0] {
                let o = sgn * (hw + 7.0);
                if crowded(off(i, sgn * (hw + 8.0), 0.0), hw + 2.0, i, 12) {
                    continue;
                }
                b.wall(
                    armco,
                    off(i, o, 0.0),
                    off(j, o, 0.0),
                    0.3,
                    1.1,
                    mul(side(i), -sgn),
                );
                if (i / 2) % 3 != 2 {
                    let board = boards[(i / 2 + if sgn > 0.0 { 1 } else { 0 }) % boards.len()];
                    let o = sgn * (hw + 7.6);
                    b.wall(
                        board,
                        off(i, o, 0.0),
                        off(j, o, 0.0),
                        1.1,
                        2.3,
                        mul(side(i), -sgn),
                    );
                }
            }
        }
    }

    // Relief : terrain qui épouse l'altitude du circuit, un peu sous la piste.
    let ext = cx.max(cz) + 190.0;
    let grid = 150usize;
    let step = 2.0 * ext / grid as f32;
    let mut h = vec![0.0f32; (grid + 1) * (grid + 1)];
    let mut dist = vec![0.0f32; (grid + 1) * (grid + 1)];
    let mean = pts.iter().map(|p| p[1]).sum::<f32>() / n as f32;
    for gz in 0..=grid {
        for gx in 0..=grid {
            let (x, z) = (-ext + gx as f32 * step, -ext + gz as f32 * step);
            let (mut wsum, mut hsum, mut best, mut best_y) = (1e-4f32, mean * 1e-4, f32::MAX, mean);
            // Contraintes en « cône » autour de chaque point de piste : le relief ne monte
            // jamais au-dessus du revêtement (talus doux, pas de piste encaissée) et ne descend
            // pas trop dessous (pas de piste suspendue dans le vide).
            // Distances aux SEGMENTS de piste (pas seulement aux points GPS : sur les longues
            // lignes droites, les points sont espacés et le relief remontait entre deux).
            let (mut cap, mut floor) = (f32::MAX, f32::MIN);
            let edge = hw + step * 1.2;
            for k in 0..segs {
                let (a, c) = (pts[k], pts[at(k as isize + 1)]);
                let (dx, dz) = (c[0] - a[0], c[2] - a[2]);
                let len2 = (dx * dx + dz * dz).max(1e-6);
                let t = (((x - a[0]) * dx + (z - a[2]) * dz) / len2).clamp(0.0, 1.0);
                let (px, pz, py) = (a[0] + dx * t, a[2] + dz * t, a[1] + (c[1] - a[1]) * t);
                let d2 = (px - x).powi(2) + (pz - z).powi(2);
                let beyond = (d2.sqrt() - edge).max(0.0);
                cap = cap.min(py - 0.6 + 0.30 * beyond);
                floor = floor.max(py - 0.6 - 0.45 * beyond);
                if d2 < best {
                    best = d2;
                    best_y = py;
                }
            }
            for p in pts.iter().step_by(2) {
                let d2 = (p[0] - x).powi(2) + (p[2] - z).powi(2);
                let w = 1.0 / (d2 + 400.0).powi(2);
                wsum += w;
                hsum += w * p[1];
            }
            let d = best.sqrt();
            // Près de la piste : à la hauteur du revêtement (un peu dessous) ; au loin : relief
            // moyen des alentours ; transition douce entre les deux.
            let far = hsum / wsum;
            let k = ((d - (hw + 6.0)) / 90.0).clamp(0.0, 1.0);
            let k = k * k * (3.0 - 2.0 * k);
            let near = best_y - 0.6;
            let y = near + (far - near) * k;
            h[gz * (grid + 1) + gx] = y.max(floor).min(cap);
            dist[gz * (grid + 1) + gx] = d;
        }
    }
    let hv = |gx: usize, gz: usize| h[gz * (grid + 1) + gx];
    // Normales lissées du relief, sommets partagés par groupe de matériau.
    let normal_at = |gx: usize, gz: usize| {
        let l = hv(gx.saturating_sub(1), gz);
        let r = hv((gx + 1).min(grid), gz);
        let d = hv(gx, gz.saturating_sub(1));
        let u = hv(gx, (gz + 1).min(grid));
        norm([l - r, 2.0 * step, d - u])
    };
    // Herbe propre : un seul vert uniforme (les teintes par carré dessinaient un damier
    // en escalier) ; le relief est lu grâce à l'éclairage et aux ombres.
    let lawn = mat(0x4A7E33, 6);
    let mut shared: HashMap<(usize, usize, usize), u16> = HashMap::new();
    for gz in 0..grid {
        for gx in 0..grid {
            let m = lawn;
            let g = {
                b.group(m, 4);
                b.current[&m]
            };
            let mut ids = [0u16; 4];
            for (k, (dx, dz)) in [(0, 0), (1, 0), (1, 1), (0, 1)].into_iter().enumerate() {
                let (vx, vz) = (gx + dx, gz + dz);
                ids[k] = *shared.entry((g, vx, vz)).or_insert_with(|| {
                    let grp = &mut b.groups[g];
                    grp.pos
                        .push([-ext + vx as f32 * step, hv(vx, vz), -ext + vz as f32 * step]);
                    grp.nrm.push(normal_at(vx, vz));
                    (grp.pos.len() - 1) as u16
                });
            }
            b.groups[g]
                .idx
                .extend_from_slice(&[ids[0], ids[2], ids[1], ids[0], ids[3], ids[2]]);
        }
    }

    // Ligne de départ en damier, grille de départ, portique.
    let ahead = tangent(0);
    let s0 = side(0);
    let cols = 8;
    for r in 0..2 {
        for c in 0..cols {
            let m = if (r + c) % 2 == 0 {
                white
            } else {
                mat(0x111111, 20)
            };
            let a = -hw + 2.0 * hw * c as f32 / cols as f32;
            let w = 2.0 * hw / cols as f32;
            let base = add(
                pts[0],
                add(mul(ahead, r as f32 * 1.0 - 1.0), [0.0, 0.18, 0.0]),
            );
            let q = |u: f32, v: f32| add(base, add(mul(s0, u), mul(ahead, v)));
            b.quad(m, [q(a, 0.0), q(a + w, 0.0), q(a + w, 1.0), q(a, 1.0)], up);
        }
    }
    let mut back = 0usize;
    let mut walked = 0.0;
    for slot in 1..=10 {
        while walked < slot as f32 * 9.0 && back < n {
            let a = at(-(back as isize));
            let c = at(-(back as isize) - 1);
            let d = sub(pts[a], pts[c]);
            walked += (d[0] * d[0] + d[2] * d[2]).sqrt();
            back += 1;
        }
        let i = at(-(back as isize));
        let lateral = if slot % 2 == 0 { -hw * 0.5 } else { hw * 0.5 };
        let c = off(i, lateral, 0.16);
        let (t, s) = (tangent(i), side(i));
        let q = |u: f32, v: f32| add(c, add(mul(s, u), mul(t, v)));
        b.quad(
            white,
            [q(-2.2, -0.25), q(2.2, -0.25), q(2.2, 0.25), q(-2.2, 0.25)],
            up,
        );
    }
    let gantry = mat(0x2E3036, 40);
    for s in [-1.0f32, 1.0] {
        b.block(
            gantry,
            add(off(0, s * (hw + 3.0), 0.0), [0.0, 7.0, 0.0]),
            [ahead, s0, up],
            [0.6, 0.6, 7.0],
        );
    }
    b.block(
        gantry,
        add(pts[0], [0.0, 13.0, 0.0]),
        [ahead, s0, up],
        [0.9, hw + 3.6, 1.2],
    );
    for k in 0..5 {
        b.block(
            red,
            add(
                add(pts[0], mul(s0, -4.0 + 2.0 * k as f32)),
                [0.0, 13.0, 0.0],
            ),
            [ahead, s0, up],
            [1.0, 0.5, 0.5],
        );
    }

    // Tribunes et stands le long de la ligne droite des stands.
    let mid = at(-((n / 40).max(3) as isize));
    let (t, s) = (tangent(mid), side(mid));
    let free_side = |sgn: f32, o0: f32, o1: f32, half: f32| {
        [-half, 0.0, half].iter().all(|&e| {
            [o0, (o0 + o1) / 2.0, o1]
                .iter()
                .all(|&o| !crowded(add(off(mid, sgn * o, 0.0), mul(t, e)), hw + 3.0, mid, 0))
        })
    };
    let stands_ok = free_side(-1.0, hw + 12.0, hw + 34.0, 56.0);
    let pits_ok = free_side(1.0, hw + 7.0, hw + 29.0, 61.0);
    let stand_seats = [mat(0xC8102E, 30), mat(0x1E4FB5, 30), mat(0xE8E8E8, 30)];
    let concrete = mat(0x8E9196, 15);
    if stands_ok {
        for tier in 0..4 {
            let o = hw + 14.0 + tier as f32 * 4.5;
            let c = add(off(mid, -o, 0.0), [0.0, 1.5 + tier as f32 * 2.4, 0.0]);
            b.block(
                concrete,
                c,
                [t, s, up],
                [55.0, 2.25, 1.5 + tier as f32 * 2.4],
            );
            b.block(
                stand_seats[tier % 3],
                add(c, [0.0, 1.5 + tier as f32 * 2.4, 0.0]),
                [t, s, up],
                [54.0, 2.0, 0.25],
            );
        }
        b.block(
            mat(0xD9DCE1, 25),
            add(off(mid, -(hw + 30.0), 0.0), [0.0, 14.0, 0.0]),
            [t, s, up],
            [56.0, 3.0, 0.6],
        );
    }
    if pits_ok {
        let pit = mat(0xE4E6EA, 30);
        b.block(
            pit,
            add(off(mid, hw + 22.0, 0.0), [0.0, 5.0, 0.0]),
            [t, s, up],
            [60.0, 6.0, 5.0],
        );
        b.block(
            mat(0x1C1E22, 60),
            add(off(mid, hw + 22.0, 0.0), [0.0, 10.3, 0.0]),
            [t, s, up],
            [61.0, 6.6, 0.3],
        );
        b.block(
            mat(0x3B76D6, 80),
            add(off(mid, hw + 15.9, 0.0), [0.0, 7.0, 0.0]),
            [t, s, up],
            [58.0, 0.1, 2.0],
        );

        // Voie des stands : asphalte parallèle, ligne blanche, garages.
        let span = (n / 25).max(4) as isize;
        for k in -span..span {
            let (i, j) = (at(mid as isize + k), at(mid as isize + k + 1));
            let q = |o0: f32, o1: f32, dy: f32| {
                [
                    off(i, o0, dy),
                    off(j, o0, dy),
                    off(j, o1, dy),
                    off(i, o1, dy),
                ]
            };
            b.quad(asphalt, q(hw + 7.5, hw + 15.5, 0.1), up);
            b.quad(white, q(hw + 7.5, hw + 7.9, 0.22), up);
        }
        for k in 0..10 {
            let along = -50.0 + k as f32 * 11.0;
            b.block(
                mat(if k % 2 == 0 { 0x2B2E35 } else { 0x343842 }, 50),
                add(
                    add(off(mid, hw + 15.95, 0.0), mul(t, along)),
                    [0.0, 2.4, 0.0],
                ),
                [t, s, up],
                [4.5, 0.05, 2.4],
            );
        }
    }

    // Tribunes à l'extérieur des virages les plus serrés.
    let mut corners: Vec<usize> = (0..n).filter(|&i| turn[i].abs() > 0.5).collect();
    corners.sort_by(|a, b| turn[*b].abs().total_cmp(&turn[*a].abs()));
    let mut placed: Vec<V> = vec![pts[mid]];
    for &i in &corners {
        if placed.len() > 5 {
            break;
        }
        if placed.iter().any(|p| {
            let d = sub(*p, pts[i]);
            d[0] * d[0] + d[2] * d[2] < 160.0 * 160.0
        }) {
            continue;
        }
        let outside = if turn[i] > 0.0 { -1.0 } else { 1.0 };
        let (ti, si) = (tangent(i), side(i));
        // Emprise de la tribune (gradins + toit) : rien sur une autre portion de piste.
        let blocked = [-26.0f32, 0.0, 26.0].iter().any(|&e| {
            [hw + 26.0, hw + 38.0, hw + 48.0]
                .iter()
                .any(|&o| crowded(add(off(i, outside * o, 0.0), mul(ti, e)), hw + 4.0, i, 0))
        });
        if blocked {
            continue;
        }
        placed.push(pts[i]);
        for tier in 0..3 {
            let o = outside * (hw + 32.0 + tier as f32 * 4.0);
            let c = add(off(i, o, 0.0), [0.0, 1.2 + tier as f32 * 2.0, 0.0]);
            b.block(
                concrete,
                c,
                [ti, si, up],
                [24.0, 2.0, 1.2 + tier as f32 * 2.0],
            );
            b.block(
                stand_seats[(tier + placed.len()) % 3],
                add(c, [0.0, 1.2 + tier as f32 * 2.0, 0.0]),
                [ti, si, up],
                [23.5, 1.8, 0.22],
            );
        }
        let roof_o = outside * (hw + 38.0);
        b.block(
            mat(0xD9DCE1, 25),
            add(off(i, roof_o, 0.0), [0.0, 10.5, 0.0]),
            [ti, si, up],
            [25.0, 8.5, 0.4],
        );
        for e in [-23.0f32, 23.0] {
            b.block(
                concrete,
                add(
                    add(off(i, outside * (hw + 44.0), 0.0), mul(ti, e)),
                    [0.0, 5.2, 0.0],
                ),
                [ti, si, up],
                [0.5, 0.5, 5.2],
            );
        }
    }

    // Panneaux de freinage (3, 2 et 1 bandes) avant les virages lents, côté extérieur.
    let board = mat(0xF2F2F2, 30);
    let board_ink = mat(0x15161A, 20);
    let post = mat(0x5A5E66, 40);
    let speeds: Vec<f32> = raw.iter().map(|p| p.speed as f32).collect();
    let mut last_apex: Option<usize> = None;
    for i in 0..n {
        let window = 8isize;
        let is_min = (-window..=window).all(|k| speeds[at(i as isize + k)] >= speeds[i]);
        let before = (10..40)
            .map(|k| speeds[at(i as isize - k)])
            .fold(0.0f32, f32::max);
        if !is_min || speeds[i] > 200.0 || before < speeds[i] + 70.0 {
            continue;
        }
        if last_apex.is_some_and(|l| i - l < 12) {
            continue;
        }
        last_apex = Some(i);
        // Côté extérieur du virage qui arrive.
        let outside = if turn[i] > 0.0 { -1.0 } else { 1.0 };
        let mut walked = 0.0f32;
        let mut k = i as isize;
        for (bars, dist) in [(1usize, 45.0f32), (2, 90.0), (3, 135.0)] {
            while walked < dist && (i as isize - k) < n as isize {
                let d = sub(pts[at(k)], pts[at(k - 1)]);
                walked += (d[0] * d[0] + d[2] * d[2]).sqrt();
                k -= 1;
            }
            let idx = at(k);
            let (tg, sd) = (tangent(idx), side(idx));
            let foot = off(idx, outside * (hw + 5.0), 0.0);
            if crowded(foot, hw + 1.5, idx, 12) {
                continue;
            }
            b.block(
                post,
                add(foot, [0.0, 1.4, 0.0]),
                [tg, sd, up],
                [0.12, 0.12, 1.4],
            );
            let panel = add(foot, [0.0, 3.4, 0.0]);
            b.block(board, panel, [tg, sd, up], [0.08, 1.1, 0.75]);
            for bar in 0..bars {
                let yy = (bar as f32 - (bars as f32 - 1.0) / 2.0) * 0.42;
                b.block(
                    board_ink,
                    add(panel, [0.0, yy, 0.0]),
                    [tg, sd, up],
                    [0.1, 0.85, 0.11],
                );
            }
        }
    }

    // Arbres, à distance de la piste.
    let mut rng = Rng(0xF1F1_2026 ^ raw.len() as u64);
    let trunk = mat(0x4A3423, 5);
    let leaves = [
        mat(0x24451F, 8),
        mat(0x2E5626, 8),
        mat(0x1D3A1A, 8),
        mat(0x3D6B2A, 8),
        mat(0x4F7A2C, 8),
    ];
    let mut planted = 0;
    let mut tries = 0;
    while planted < 420 && tries < 8000 {
        tries += 1;
        let (gx, gz) = (
            (rng.next() * grid as f32) as usize,
            (rng.next() * grid as f32) as usize,
        );
        let d = dist[gz * (grid + 1) + gx];
        if !(40.0..170.0).contains(&d) {
            continue;
        }
        let base = [-ext + gx as f32 * step, hv(gx, gz), -ext + gz as f32 * step];
        let size = 0.7 + rng.next() * 0.8;
        b.block(
            trunk,
            add(base, [0.0, 2.0 * size, 0.0]),
            [[1.0, 0.0, 0.0], [0.0, 0.0, 1.0], up],
            [0.5 * size, 0.5 * size, 2.0 * size],
        );
        let m = leaves[planted % leaves.len()];
        if rng.next() < 0.55 {
            // Feuillu : deux ou trois boules qui se chevauchent.
            let r = 3.6 * size;
            b.blob(m, add(base, [0.0, 4.0 * size + r * 0.7, 0.0]), r, r * 0.85);
            b.blob(
                m,
                add(base, [r * 0.55, 3.6 * size + r * 0.5, r * 0.3]),
                r * 0.7,
                r * 0.6,
            );
            if rng.next() < 0.5 {
                b.blob(
                    m,
                    add(base, [-r * 0.5, 3.8 * size + r * 0.55, -r * 0.35]),
                    r * 0.65,
                    r * 0.55,
                );
            }
        } else {
            b.cone(m, add(base, [0.0, 2.5 * size, 0.0]), 4.2 * size, 7.0 * size);
            b.cone(m, add(base, [0.0, 6.0 * size, 0.0]), 3.0 * size, 6.0 * size);
        }
        planted += 1;
    }

    b.export()
}

#[cfg(test)]
mod tests {
    use super::*;
    use f1x_protocol::{TrackPoint, TrackStats};

    #[test]
    fn builds_a_valid_file() {
        let points: Vec<TrackPoint> = (0..120)
            .map(|i| {
                let a = i as f32 / 120.0 * std::f32::consts::TAU;
                TrackPoint {
                    x: 500.0 + 400.0 * a.cos(),
                    y: 300.0 + 250.0 * a.sin(),
                    z: 5.0 + 3.0 * a.sin(),
                    t: i as f32,
                    speed: 150 + (i % 7) as u16 * 20,
                    gear: 5,
                    throttle: 80,
                    brake: false,
                }
            })
            .collect();
        let map = TrackMap {
            circuit_id: "test".into(),
            year: 2026,
            session_key: 1,
            event: "Test".into(),
            driver: "X".into(),
            team: "Y".into(),
            colour: "FF0000".into(),
            lap: 1,
            lap_time: 90.0,
            width: 1000.0,
            height: 600.0,
            points,
            stats: TrackStats {
                top_speed: 290,
                min_speed: 150,
                avg_speed: 200.0,
                full_throttle_pct: 50.0,
                braking_pct: 10.0,
                length_km: 4.0,
                gear_changes: 30,
            },
        };
        let bytes = build(&map);
        assert_eq!(&bytes[..4], b"F1XC");
        assert_eq!(u32::from_le_bytes(bytes[4..8].try_into().unwrap()), 2);
        assert!(bytes.len() > 100_000);
    }
}
