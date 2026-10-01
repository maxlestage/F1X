//! Décor 3D d'un circuit, calculé une fois par le serveur à partir du tracé GPS OpenF1 et
//! partagé par le site (WebGL) et l'app iOS (SceneKit) : relief, herbe, asphalte, lignes
//! blanches, vibreurs, bacs à graviers, rails, grille de départ, portique, tribunes, stands, arbres.
//!
//! Même format binaire que la monoplace (`F1XC`, version 2 : échelle des positions en tête).

use std::collections::HashMap;

use f1x_protocol::TrackMap;

/// Mêmes constantes que les clients (repère : x − cx, altitude × RELIEF, y − cz).
pub const RELIEF: f32 = 4.0;
pub const HALF_WIDTH: f32 = 8.0;

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

fn speed_colour(ratio: f32) -> u32 {
    let r = ratio.clamp(0.0, 1.0);
    let l = |a: f32, b: f32| (a + (b - a) * r) as u32;
    (l(179.0, 255.0) << 16) | (l(38.0, 228.0) << 8) | l(30.0, 222.0)
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
    let (min_s, max_s) = (map.stats.min_speed as f32, map.stats.top_speed as f32);
    let up = [0.0, 1.0, 0.0];
    let hw = HALF_WIDTH;
    let off = |i: usize, o: f32, dy: f32| add(add(pts[i], mul(side(i), o)), [0.0, dy, 0.0]);

    let asphalt = mat(0x2A2C31, 18);
    let white = mat(0xEDEDED, 20);
    let red = mat(0xD3202A, 25);
    let gravel = mat(0xC4AE86, 4);
    let armco = mat(0x9EA3AA, 70);
    let skirt = mat(0x3A3C40, 10);

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
        // Asphalte, lignes blanches de bord, trajectoire colorée selon la vitesse.
        strip(&mut b, asphalt, -hw, hw, 0.0);
        strip(&mut b, white, hw - 0.9, hw - 0.3, 0.04);
        strip(&mut b, white, -hw + 0.3, -hw + 0.9, 0.04);
        let ratio = (raw[i].speed as f32 - min_s) / (max_s - min_s).max(1.0);
        let level = (ratio * 7.0).round() / 7.0;
        strip(&mut b, mat(speed_colour(level), 15), -0.8, 0.8, 0.05);
        // Bordures latérales jusqu'au sol (pas de jour entre la piste et le relief).
        for s in [-1.0f32, 1.0] {
            let (a0, a1) = (off(i, s * hw, 0.0), off(j, s * hw, 0.0));
            b.quad(
                skirt,
                [a0, a1, add(a1, [0.0, -4.0, 0.0]), add(a0, [0.0, -4.0, 0.0])],
                mul(side(i), s),
            );
        }
        // Vibreurs rouges et blancs dans les virages.
        let t = turn[i].abs();
        if t > 0.10 {
            let m = if i % 2 == 0 { red } else { white };
            strip(&mut b, m, hw, hw + 2.4, 0.08);
            strip(&mut b, m, -hw - 2.4, -hw, 0.08);
        }
        // Bac à graviers et rail à l'extérieur des virages lents.
        if t > 0.22 && raw[i].speed < 230 {
            let outside = if turn[i] > 0.0 { -1.0 } else { 1.0 };
            let (o0, o1) = (outside * (hw + 2.4), outside * (hw + 22.0));
            strip(&mut b, gravel, o0.min(o1), o0.max(o1), 0.02);
            let (r0, r1) = (off(i, o1, 0.0), off(j, o1, 0.0));
            b.quad(
                armco,
                [r0, r1, add(r1, [0.0, 1.6, 0.0]), add(r0, [0.0, 1.6, 0.0])],
                mul(side(i), -outside),
            );
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
            // Plus bas point de piste dans le voisinage : le relief passe toujours dessous.
            let reach = (hw + step * 1.8).powi(2);
            let mut low = f32::MAX;
            for p in pts.iter() {
                let d2 = (p[0] - x).powi(2) + (p[2] - z).powi(2);
                if d2 < reach {
                    low = low.min(p[1]);
                }
            }
            for p in pts.iter().step_by(2) {
                let d2 = (p[0] - x).powi(2) + (p[2] - z).powi(2);
                let w = 1.0 / (d2 + 400.0).powi(2);
                wsum += w;
                hsum += w * p[1];
                if d2 < best {
                    best = d2;
                    best_y = p[1];
                }
            }
            let d = best.sqrt();
            // Près de la piste : à la hauteur du revêtement (un peu dessous) ; au loin : relief
            // moyen des alentours ; transition douce entre les deux (pas de falaise).
            let far = hsum / wsum;
            let k = ((d - (hw + 6.0)) / 90.0).clamp(0.0, 1.0);
            let k = k * k * (3.0 - 2.0 * k);
            let near = best_y.min(low) - 0.6;
            let y = near + (far - near) * k;
            h[gz * (grid + 1) + gx] = if low < f32::MAX { y.min(low - 0.6) } else { y };
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
    let grass = [mat(0x3E6A2D, 6), mat(0x35602A, 6), mat(0x2B4A23, 6)];
    let mut shared: HashMap<(usize, usize, usize), u16> = HashMap::new();
    for gz in 0..grid {
        for gx in 0..grid {
            let d = dist[gz * (grid + 1) + gx];
            // Bandes de tonte près de la piste, herbe plus sombre au loin.
            let _ = d;
            let m = grass[((gx + gz) / 5) % 2];
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
                add(mul(ahead, r as f32 * 1.0 - 1.0), [0.0, 0.09, 0.0]),
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
        let c = add(off(i, lateral, 0.07), [0.0, 0.0, 0.0]);
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
    let stand_seats = [mat(0xC8102E, 30), mat(0x1E4FB5, 30), mat(0xE8E8E8, 30)];
    let concrete = mat(0x8E9196, 15);
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

    // Arbres, à distance de la piste.
    let mut rng = Rng(0xF1F1_2026 ^ raw.len() as u64);
    let trunk = mat(0x4A3423, 5);
    let leaves = [mat(0x24451F, 8), mat(0x2E5626, 8), mat(0x1D3A1A, 8)];
    let mut planted = 0;
    let mut tries = 0;
    while planted < 320 && tries < 6000 {
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
        let m = leaves[planted % 3];
        b.cone(m, add(base, [0.0, 2.5 * size, 0.0]), 4.2 * size, 7.0 * size);
        b.cone(m, add(base, [0.0, 6.0 * size, 0.0]), 3.0 * size, 6.0 * size);
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
