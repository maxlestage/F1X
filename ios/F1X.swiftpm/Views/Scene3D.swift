import SceneKit
import SwiftUI
import UIKit

// 3D native (SceneKit) : monoplace stylisée générée par le code (aucun modèle officiel) et
// circuits en relief à partir des positions GPS OpenF1. Mêmes formes que le site web.

typealias V3 = SIMD3<Float>

// MARK: - Livrées

struct Livery: Equatable {
    var primary: UIColor
    var second: UIColor
    var accent: UIColor

    static func team(_ constructorId: String?) -> Livery {
        func c(_ hex: UInt32) -> UIColor { UIColor(Color(hex: hex)) }
        switch constructorId {
        case "mercedes": return Livery(primary: c(0xA3AAB1), second: c(0x141618), accent: c(0x27F4D2))
        case "ferrari": return Livery(primary: c(0xD8001F), second: c(0x141414), accent: c(0xF2F2F2))
        case "red_bull": return Livery(primary: c(0x1C2A63), second: c(0x0E1636), accent: c(0xE2141C))
        case "mclaren": return Livery(primary: c(0xFF8000), second: c(0x1E1E20), accent: c(0x56C3F0))
        case "aston_martin": return Livery(primary: c(0x00594F), second: c(0x0A2E29), accent: c(0xCEDC00))
        case "alpine": return Livery(primary: c(0x1667C9), second: c(0x0B1A3A), accent: c(0xFF87BC))
        case "williams": return Livery(primary: c(0x0C2D6B), second: c(0x06173A), accent: c(0x64C4FF))
        case "rb": return Livery(primary: c(0xF1F2F4), second: c(0x1634CB), accent: c(0xE1061B))
        case "haas": return Livery(primary: c(0xECEDEF), second: c(0x1A1A1C), accent: c(0xE10600))
        case "sauber": return Livery(primary: c(0x1B1B1D), second: c(0x0E0E0F), accent: c(0x52E252))
        case "audi": return Livery(primary: c(0xA9ADB1), second: c(0x151517), accent: c(0xF50537))
        case "cadillac": return Livery(primary: c(0x151517), second: c(0xE7E7EA), accent: c(0xC9A96E))
        default: return .single(UIColor(Team.color(constructorId)))
        }
    }

    /// Livrée déduite d'une seule couleur (voitures du direct).
    static func single(_ color: UIColor) -> Livery {
        var r: CGFloat = 0, g: CGFloat = 0, b: CGFloat = 0, a: CGFloat = 0
        color.getRed(&r, green: &g, blue: &b, alpha: &a)
        return Livery(
            primary: color,
            second: UIColor(red: r * 0.28, green: g * 0.28, blue: b * 0.28, alpha: 1),
            accent: UIColor(red: r * 0.35 + 0.65, green: g * 0.35 + 0.65, blue: b * 0.35 + 0.65, alpha: 1)
        )
    }
}

// MARK: - Construction des maillages

/// Matériau d'un groupe de triangles.
enum Mat: Hashable {
    case paint, second, accent
    /// Couleur fixe (r, g, b) et brillance 0–100.
    case fixed(UInt32, Int)
}

private let CARBON = Mat.fixed(0x0D0D0F, 25)
private let DARK = Mat.fixed(0x040405, 60)
private let TYRE = Mat.fixed(0x0B0B0C, 6)
private let RIM = Mat.fixed(0x4D4F54, 85)
private let STRIPE = Mat.fixed(0xFACC14, 20)
private let VISOR = Mat.fixed(0x05080D, 100)
private let NUT = Mat.fixed(0xD91414, 70)
private let MIRROR = Mat.fixed(0x99A6B3, 100)
private let RAINLIGHT = Mat.fixed(0xFF2020, 10)

final class MeshBuilder {
    var groups: [Mat: (pos: [V3], nrm: [V3])] = [:]
    var order: [Mat] = []

    private func tri(_ a: V3, _ b: V3, _ c: V3, _ na: V3, _ nb: V3, _ nc: V3, _ m: Mat) {
        if groups[m] == nil {
            groups[m] = ([], [])
            order.append(m)
        }
        // Sens trigonométrique vu de l'extérieur (faces arrière masquées par SceneKit).
        let face = simd_cross(b - a, c - a)
        if simd_dot(face, na + nb + nc) < 0 {
            groups[m]!.pos += [a, c, b]
            groups[m]!.nrm += [na, nc, nb]
        } else {
            groups[m]!.pos += [a, b, c]
            groups[m]!.nrm += [na, nb, nc]
        }
    }

    func quad(_ p: [V3], _ n: [V3], _ m: Mat) {
        tri(p[0], p[1], p[2], n[0], n[1], n[2], m)
        tri(p[0], p[2], p[3], n[0], n[2], n[3], m)
    }

    /// Face plane dont la normale s'éloigne de `inside`.
    func face(_ pts: [V3], inside: V3, _ m: Mat) {
        var n = simd_normalize(simd_cross(pts[1] - pts[0], pts[2] - pts[0]))
        let centre = pts.reduce(V3.zero, +) / Float(pts.count)
        if simd_dot(n, centre - inside) < 0 { n = -n }
        for i in 1..<(pts.count - 1) {
            tri(pts[0], pts[i], pts[i + 1], n, n, n, m)
        }
    }

    func hexa(_ p: [V3], _ m: Mat) {
        let inside = p.reduce(V3.zero, +) / 8
        face([p[0], p[1], p[2], p[3]], inside: inside, m)
        face([p[4], p[5], p[6], p[7]], inside: inside, m)
        for i in 0..<4 {
            let j = (i + 1) % 4
            face([p[i], p[j], p[j + 4], p[i + 4]], inside: inside, m)
        }
    }

    func box(_ lo: V3, _ hi: V3, _ m: Mat) {
        hexa([
            V3(lo.x, lo.y, lo.z), V3(lo.x, lo.y, hi.z), V3(lo.x, hi.y, hi.z), V3(lo.x, hi.y, lo.z),
            V3(hi.x, lo.y, lo.z), V3(hi.x, lo.y, hi.z), V3(hi.x, hi.y, hi.z), V3(hi.x, hi.y, lo.z),
        ], m)
    }

    /// Lame d'aileron entre bord d'attaque et bord de fuite, sur une envergure z0…z1.
    func blade(_ front: (Float, Float), _ back: (Float, Float), _ thick: Float, _ z0: Float, _ z1: Float, _ m: Mat) {
        let (xf, yf) = front, (xb, yb) = back
        hexa([
            V3(xf, yf, z0), V3(xf, yf, z1), V3(xf, yf + thick, z1), V3(xf, yf + thick, z0),
            V3(xb, yb, z0), V3(xb, yb, z1), V3(xb, yb + thick, z1), V3(xb, yb + thick, z0),
        ], m)
    }

    struct Ring {
        var x: Float, zc: Float = 0, hw: Float, y0: Float, y1: Float
    }

    /// Carrosserie lissée : sections en super-ellipse reliées, normales lissées.
    func smooth(_ rings: [Ring], e: Float, segs: Int, skin: (Float) -> Mat) {
        let n = rings.count
        let unit: [(Float, Float)] = (0..<segs).map { j in
            let a = 2 * Float.pi * Float(j) / Float(segs)
            let c = cos(a), s = sin(a)
            return (copysign(pow(abs(c), 2 / e), c), copysign(pow(abs(s), 2 / e), s))
        }
        func point(_ i: Int, _ j: Int) -> V3 {
            let r = rings[i], (u, v) = unit[j % segs]
            return V3(r.x, (r.y0 + r.y1) / 2 + v * (r.y1 - r.y0) / 2, r.zc + u * r.hw)
        }
        func centre(_ i: Int) -> V3 { V3(rings[i].x, (rings[i].y0 + rings[i].y1) / 2, rings[i].zc) }
        func normal(_ i: Int, _ j: Int) -> V3 {
            let along = point(min(i + 1, n - 1), j) - point(max(i - 1, 0), j)
            let around = point(i, j + 1) - point(i, j + segs - 1)
            var nn = simd_normalize(simd_cross(along, around))
            if simd_dot(nn, point(i, j) - centre(i)) < 0 { nn = -nn }
            return nn
        }
        for i in 0..<(n - 1) {
            for j in 0..<segs {
                let m = skin((unit[j].1 + unit[(j + 1) % segs].1) / 2)
                quad([point(i, j), point(i + 1, j), point(i + 1, j + 1), point(i, j + 1)],
                     [normal(i, j), normal(i + 1, j), normal(i + 1, j + 1), normal(i, j + 1)], m)
            }
        }
        for (i, dir) in [(0, Float(-1)), (n - 1, Float(1))] {
            let nn = simd_normalize((centre(min(i + 1, n - 1)) - centre(max(i - 1, 0))) * dir)
            let m = skin(0)
            for j in 0..<segs {
                tri(centre(i), point(i, j), point(i, j + 1), nn, nn, nn, m)
            }
        }
    }

    /// Tube de section ronde le long d'une polyligne.
    func tube(_ pts: [V3], _ r: Float, _ m: Mat) {
        let segs = 8
        let rings: [[(V3, V3)]] = pts.indices.map { i in
            let t = simd_normalize(pts[min(i + 1, pts.count - 1)] - pts[max(i - 1, 0)])
            let up: V3 = abs(t.y) > 0.9 ? V3(1, 0, 0) : V3(0, 1, 0)
            let s = simd_normalize(simd_cross(t, up))
            let u = simd_cross(s, t)
            return (0..<segs).map { j in
                let a = 2 * Float.pi * Float(j) / Float(segs)
                let d = s * cos(a) + u * sin(a)
                return (pts[i] + d * r, d)
            }
        }
        for k in 0..<(rings.count - 1) {
            for j in 0..<segs {
                let jn = (j + 1) % segs
                quad([rings[k][j].0, rings[k + 1][j].0, rings[k + 1][jn].0, rings[k][jn].0],
                     [rings[k][j].1, rings[k + 1][j].1, rings[k + 1][jn].1, rings[k][jn].1], m)
            }
        }
    }

    func ellipsoid(_ c: V3, _ r: V3, _ m: Mat) {
        let rings = 10, segs = 18
        func at(_ i: Int, _ j: Int) -> V3 {
            let th = Float.pi * Float(i) / Float(rings), ph = 2 * Float.pi * Float(j) / Float(segs)
            return V3(sin(th) * cos(ph), cos(th), sin(th) * sin(ph))
        }
        for i in 0..<rings {
            for j in 0..<segs {
                let q = [at(i, j), at(i + 1, j), at(i + 1, j + 1), at(i, j + 1)]
                quad(q.map { c + $0 * r }, q.map { simd_normalize($0 / r) }, m)
            }
        }
    }

    /// Anneau plat perpendiculaire à Z.
    func ringZ(_ c: V3, _ r0: Float, _ r1: Float, _ z: Float, _ nz: Float, _ m: Mat) {
        let segs = 40, n = V3(0, 0, nz)
        for i in 0..<segs {
            let a0 = 2 * Float.pi * Float(i) / Float(segs), a1 = 2 * Float.pi * Float(i + 1) / Float(segs)
            func p(_ a: Float, _ r: Float) -> V3 { V3(c.x + cos(a) * r, c.y + sin(a) * r, c.z + z) }
            quad([p(a0, r0), p(a1, r0), p(a1, r1), p(a0, r1)], [n, n, n, n], m)
        }
    }

    /// Roue d'axe Z : pneu arrondi, liseré, jante, enjoliveur, écrou.
    func wheel(_ c: V3, _ r: Float, _ width: Float) {
        let segs = 40, hw = width / 2, ri = r * 0.70, rc: Float = 0.075
        var prof: [(Float, Float, Float, Float)] = [(ri, -hw, 0, -1)]
        for s in 0...6 {
            let f = Float.pi / 2 * Float(s) / 6
            prof.append((r - rc + rc * sin(f), -hw + rc - rc * cos(f), sin(f), -cos(f)))
        }
        for s in 0...6 {
            let f = Float.pi / 2 + Float.pi / 2 * Float(s) / 6
            prof.append((r - rc + rc * sin(f), hw - rc - rc * cos(f), sin(f), -cos(f)))
        }
        prof.append((ri, hw, 0, 1))
        for i in 0..<segs {
            let a0 = 2 * Float.pi * Float(i) / Float(segs), a1 = 2 * Float.pi * Float(i + 1) / Float(segs)
            for k in 0..<(prof.count - 1) {
                func p(_ a: Float, _ q: (Float, Float, Float, Float)) -> (V3, V3) {
                    (V3(c.x + cos(a) * q.0, c.y + sin(a) * q.0, c.z + q.1), simd_normalize(V3(cos(a) * q.2, sin(a) * q.2, q.3)))
                }
                let p0 = p(a0, prof[k]), p1 = p(a1, prof[k]), p2 = p(a1, prof[k + 1]), p3 = p(a0, prof[k + 1])
                quad([p0.0, p1.0, p2.0, p3.0], [p0.1, p1.1, p2.1, p3.1], TYRE)
            }
        }
        for side: Float in [-1, 1] {
            ringZ(c, r * 0.80, r * 0.845, side * (hw + 0.002), side, STRIPE)
            let zr = side * (hw - 0.035)
            ringZ(c, ri, r * 0.62, zr, side, RIM)
            ringZ(c, r * 0.62, r * 0.52, zr, side, .accent)
            ringZ(c, r * 0.52, r * 0.12, zr, side, CARBON)
            ringZ(c, r * 0.12, 0, side * (hw - 0.02), side, NUT)
        }
    }

    /// Géométrie SceneKit : un élément par matériau.
    func geometry(livery: Livery) -> SCNGeometry {
        var pos: [V3] = [], nrm: [V3] = []
        var elements: [SCNGeometryElement] = [], materials: [SCNMaterial] = []
        for m in order {
            guard let g = groups[m] else { continue }
            let start = Int32(pos.count)
            pos += g.pos
            nrm += g.nrm
            let idx = (0..<Int32(g.pos.count)).map { start + $0 }
            elements.append(SCNGeometryElement(indices: idx, primitiveType: .triangles))
            materials.append(Self.material(m, livery))
        }
        let geo = SCNGeometry(
            sources: [
                SCNGeometrySource(vertices: pos.map { SCNVector3($0.x, $0.y, $0.z) }),
                SCNGeometrySource(normals: nrm.map { SCNVector3($0.x, $0.y, $0.z) }),
            ],
            elements: elements
        )
        geo.materials = materials
        return geo
    }

    static func material(_ m: Mat, _ l: Livery) -> SCNMaterial {
        let mat = SCNMaterial()
        mat.lightingModel = .physicallyBased
        switch m {
        case .paint, .second, .accent:
            mat.diffuse.contents = m == .paint ? l.primary : (m == .second ? l.second : l.accent)
            mat.roughness.contents = 0.22
            mat.metalness.contents = 0.35
            mat.clearCoat.contents = 1.0
            mat.clearCoatRoughness.contents = 0.05
        case .fixed(let hex, let gloss):
            mat.diffuse.contents = UIColor(Color(hex: hex))
            mat.roughness.contents = 1 - Double(gloss) / 110
            mat.metalness.contents = gloss >= 80 ? 0.85 : 0.05
        }
        return mat
    }
}

// MARK: - Monoplace

/// Modèle détaillé partagé avec le site (`car.bin`, généré par tools/carmodel/build.py).
enum CarFile {
    struct Group {
        let mat: Mat
        let positions: [SCNVector3]
        let normals: [SCNVector3]
        let indices: [UInt16]
    }

    static let groups: [Group]? = {
        guard let url = Bundle.main.url(forResource: "car", withExtension: "bin"),
              let data = try? Data(contentsOf: url) else { return nil }
        return parse([UInt8](data))
    }()

    static func parse(_ b: [UInt8]) -> [Group]? {
        guard b.count > 12, b[0] == 0x46, b[1] == 0x31, b[2] == 0x58, b[3] == 0x43 else { return nil }
        func u32(_ o: Int) -> Int { Int(b[o]) | Int(b[o + 1]) << 8 | Int(b[o + 2]) << 16 | Int(b[o + 3]) << 24 }
        func i16(_ o: Int) -> Float { Float(Int16(bitPattern: UInt16(b[o]) | UInt16(b[o + 1]) << 8)) }
        var off = 12
        var out: [Group] = []
        for _ in 0..<u32(8) {
            guard off + 16 <= b.count else { return nil }
            let kind = b[off], gloss = Int(b[off + 4])
            let hex = UInt32(b[off + 1]) << 16 | UInt32(b[off + 2]) << 8 | UInt32(b[off + 3])
            let mat: Mat = kind == 1 ? .paint : kind == 2 ? .second : kind == 3 ? .accent : .fixed(hex, gloss)
            let nv = u32(off + 8), ni = u32(off + 12)
            off += 16
            guard off + nv * 9 + ni * 2 <= b.count else { return nil }
            var pos: [SCNVector3] = [], nrm: [SCNVector3] = []
            pos.reserveCapacity(nv)
            nrm.reserveCapacity(nv)
            for i in 0..<nv {
                let o = off + i * 6
                pos.append(SCNVector3(i16(o) / 4096, i16(o + 2) / 4096, i16(o + 4) / 4096))
            }
            off += nv * 6
            for i in 0..<nv {
                let o = off + i * 3
                nrm.append(SCNVector3(Float(Int8(bitPattern: b[o])) / 127, Float(Int8(bitPattern: b[o + 1])) / 127, Float(Int8(bitPattern: b[o + 2])) / 127))
            }
            off += nv * 3
            var idx: [UInt16] = []
            idx.reserveCapacity(ni)
            for i in 0..<ni {
                let o = off + i * 2
                idx.append(UInt16(b[o]) | UInt16(b[o + 1]) << 8)
            }
            off += ni * 2
            out.append(Group(mat: mat, positions: pos, normals: nrm, indices: idx))
        }
        return out
    }

    static func geometry(livery: Livery) -> SCNGeometry? {
        guard let groups else { return nil }
        var pos: [SCNVector3] = [], nrm: [SCNVector3] = []
        var elements: [SCNGeometryElement] = [], materials: [SCNMaterial] = []
        for g in groups {
            let base = UInt32(pos.count)
            pos += g.positions
            nrm += g.normals
            elements.append(SCNGeometryElement(indices: g.indices.map { base + UInt32($0) }, primitiveType: .triangles))
            materials.append(MeshBuilder.material(g.mat, livery))
        }
        let geo = SCNGeometry(sources: [SCNGeometrySource(vertices: pos), SCNGeometrySource(normals: nrm)], elements: elements)
        geo.materials = materials
        return geo
    }
}

enum CarModel {
    private static let base: MeshBuilder = build()

    static func node(livery: Livery) -> SCNNode {
        SCNNode(geometry: CarFile.geometry(livery: livery) ?? base.geometry(livery: livery))
    }

    private static func build() -> MeshBuilder {
        let m = MeshBuilder()
        typealias R = MeshBuilder.Ring
        let twoTone: (Float) -> Mat = { $0 < -0.2 ? .second : .paint }

        // Fond plat, bords, diffuseur.
        m.hexa([V3(1.62, 0.035, -0.28), V3(1.62, 0.035, 0.28), V3(1.62, 0.065, 0.28), V3(1.62, 0.065, -0.28),
                V3(1.05, 0.035, -0.80), V3(1.05, 0.035, 0.80), V3(1.05, 0.065, 0.80), V3(1.05, 0.065, -0.80)], CARBON)
        m.box(V3(-1.95, 0.035, -0.80), V3(1.05, 0.065, 0.80), CARBON)
        for s: Float in [-1, 1] {
            m.blade((0.9, 0.065), (-1.6, 0.065), 0.09, s * 0.80, s * 0.785, CARBON)
            m.blade((0.2, 0.10), (-1.2, 0.10), 0.015, s * 0.80, s * 0.70, .second)
        }
        m.hexa([V3(-1.95, 0.035, -0.55), V3(-1.95, 0.035, 0.55), V3(-1.95, 0.11, 0.55), V3(-1.95, 0.11, -0.55),
                V3(-2.45, 0.08, -0.56), V3(-2.45, 0.08, 0.56), V3(-2.45, 0.30, 0.56), V3(-2.45, 0.30, -0.56)], CARBON)

        // Châssis : nez, monocoque, capot moteur.
        m.smooth([
            R(x: 2.88, hw: 0.05, y0: 0.17, y1: 0.23), R(x: 2.75, hw: 0.08, y0: 0.15, y1: 0.28),
            R(x: 2.45, hw: 0.12, y0: 0.14, y1: 0.36), R(x: 2.05, hw: 0.17, y0: 0.14, y1: 0.44),
            R(x: 1.6, hw: 0.23, y0: 0.13, y1: 0.52), R(x: 1.15, hw: 0.30, y0: 0.12, y1: 0.59),
            R(x: 0.75, hw: 0.36, y0: 0.10, y1: 0.64), R(x: 0.3, hw: 0.40, y0: 0.09, y1: 0.66),
            R(x: -0.1, hw: 0.41, y0: 0.09, y1: 0.70), R(x: -0.5, hw: 0.36, y0: 0.10, y1: 0.78),
            R(x: -0.95, hw: 0.27, y0: 0.12, y1: 0.68), R(x: -1.45, hw: 0.19, y0: 0.14, y1: 0.55),
            R(x: -1.9, hw: 0.12, y0: 0.16, y1: 0.44), R(x: -2.2, hw: 0.07, y0: 0.18, y1: 0.36),
        ], e: 3.2, segs: 28, skin: twoTone)
        // Habitacle et pilote.
        m.ellipsoid(V3(0.38, 0.655, 0), V3(0.44, 0.03, 0.22), DARK)
        m.ellipsoid(V3(0.22, 0.73, 0), V3(0.145, 0.14, 0.125), .accent)
        m.ellipsoid(V3(0.30, 0.735, 0), V3(0.08, 0.045, 0.11), VISOR)
        m.smooth([R(x: 0.02, hw: 0.2, y0: 0.58, y1: 0.70), R(x: -0.08, hw: 0.22, y0: 0.58, y1: 0.74)], e: 3, segs: 16) { _ in .second }

        // Prise d'air, aileron de requin, caméra.
        m.smooth([
            R(x: -0.04, hw: 0.10, y0: 0.70, y1: 0.96), R(x: -0.18, hw: 0.15, y0: 0.66, y1: 1.0),
            R(x: -0.45, hw: 0.17, y0: 0.62, y1: 0.95), R(x: -0.85, hw: 0.12, y0: 0.58, y1: 0.80),
            R(x: -1.2, hw: 0.05, y0: 0.55, y1: 0.66),
        ], e: 2.6, segs: 20) { _ in .paint }
        m.ellipsoid(V3(-0.035, 0.85, 0), V3(0.02, 0.085, 0.075), DARK)
        m.hexa([V3(-0.8, 0.80, -0.006), V3(-0.8, 0.80, 0.006), V3(-0.8, 0.92, 0.006), V3(-0.8, 0.92, -0.006),
                V3(-1.9, 0.44, -0.006), V3(-1.9, 0.44, 0.006), V3(-1.9, 0.62, 0.006), V3(-1.9, 0.62, -0.006)], .accent)
        m.box(V3(-0.12, 1.0, -0.06), V3(0, 1.04, 0.06), .accent)

        // Pontons et rétroviseurs.
        for s: Float in [-1, 1] {
            m.smooth([
                R(x: 0.98, zc: s * 0.55, hw: 0.11, y0: 0.20, y1: 0.44), R(x: 0.82, zc: s * 0.58, hw: 0.19, y0: 0.14, y1: 0.54),
                R(x: 0.35, zc: s * 0.59, hw: 0.22, y0: 0.11, y1: 0.56), R(x: -0.25, zc: s * 0.53, hw: 0.20, y0: 0.11, y1: 0.49),
                R(x: -0.85, zc: s * 0.43, hw: 0.14, y0: 0.12, y1: 0.37), R(x: -1.45, zc: s * 0.31, hw: 0.08, y0: 0.14, y1: 0.26),
            ], e: 3.6, segs: 24, skin: twoTone)
            m.ellipsoid(V3(0.985, 0.33, s * 0.55), V3(0.01, 0.09, 0.08), DARK)
            m.tube([V3(0.6, 0.62, s * 0.36), V3(0.58, 0.70, s * 0.47)], 0.012, CARBON)
            m.smooth([R(x: 0.63, zc: s * 0.52, hw: 0.07, y0: 0.68, y1: 0.76), R(x: 0.53, zc: s * 0.52, hw: 0.075, y0: 0.67, y1: 0.77)], e: 3, segs: 14) { _ in .paint }
            m.box(V3(0.528, 0.685, s * 0.52 - 0.06), V3(0.532, 0.755, s * 0.52 + 0.06), MIRROR)
        }

        // Halo.
        let hoop: [V3] = (0...16).map { i in
            let a = (-150 + 300 * Float(i) / 16) * Float.pi / 180
            return V3(0.30 + 0.44 * cos(a), 0.87 - 0.03 * abs(cos(a)), 0.32 * sin(a))
        }
        m.tube(hoop, 0.026, CARBON)
        m.tube([V3(0.74, 0.84, 0), V3(0.86, 0.72, 0), V3(0.98, 0.57, 0)], 0.03, CARBON)
        m.tube([hoop[0], V3(-0.12, 0.74, -0.24), V3(-0.16, 0.66, -0.28)], 0.026, CARBON)
        m.tube([hoop[16], V3(-0.12, 0.74, 0.24), V3(-0.16, 0.66, 0.28)], 0.026, CARBON)

        // Aileron avant.
        m.blade((2.98, 0.075), (2.55, 0.085), 0.025, -0.97, 0.97, CARBON)
        for s: Float in [-1, 1] {
            let z0 = s * 0.26, z1 = s * 0.95
            m.blade((2.70, 0.115), (2.47, 0.15), 0.02, z0, z1, CARBON)
            m.blade((2.56, 0.165), (2.38, 0.215), 0.018, z0, z1, .second)
            m.blade((2.45, 0.235), (2.33, 0.285), 0.016, z0, z1, .accent)
            m.hexa([V3(2.99, 0.05, s * 0.95), V3(2.99, 0.05, s * 0.985), V3(2.99, 0.20, s * 0.985), V3(2.99, 0.20, s * 0.95),
                    V3(2.32, 0.05, s * 0.95), V3(2.32, 0.05, s * 0.985), V3(2.32, 0.32, s * 0.985), V3(2.32, 0.32, s * 0.95)], .paint)
        }
        m.blade((2.62, 0.13), (2.55, 0.10), 0.03, -0.012, 0.012, CARBON)

        // Aileron arrière.
        for s: Float in [-1, 1] {
            m.hexa([V3(-2.0, 0.42, s * 0.50), V3(-2.0, 0.42, s * 0.53), V3(-2.0, 0.98, s * 0.53), V3(-2.0, 0.98, s * 0.50),
                    V3(-2.62, 0.30, s * 0.50), V3(-2.62, 0.30, s * 0.53), V3(-2.62, 1.0, s * 0.53), V3(-2.62, 1.0, s * 0.50)], .paint)
        }
        m.blade((-2.12, 0.80), (-2.52, 0.86), 0.035, -0.5, 0.5, CARBON)
        m.blade((-2.10, 0.91), (-2.34, 0.97), 0.025, -0.5, 0.5, .accent)
        m.blade((-2.20, 0.42), (-2.45, 0.46), 0.025, -0.45, 0.45, CARBON)
        m.box(V3(-2.30, 0.30, -0.025), V3(-2.05, 0.82, 0.025), CARBON)
        m.box(V3(-2.66, 0.30, -0.05), V3(-2.62, 0.36, 0.05), RAINLIGHT)

        // Roues et suspensions.
        for s: Float in [-1, 1] {
            m.wheel(V3(1.75, 0.36, s * 0.84), 0.36, 0.30)
            m.wheel(V3(-1.85, 0.36, s * 0.80), 0.36, 0.40)
            let arms: [(V3, V3)] = [
                (V3(1.45, 0.47, s * 0.22), V3(1.76, 0.44, s * 0.69)), (V3(1.98, 0.43, s * 0.20), V3(1.76, 0.44, s * 0.69)),
                (V3(1.40, 0.20, s * 0.24), V3(1.74, 0.24, s * 0.69)), (V3(2.00, 0.18, s * 0.20), V3(1.74, 0.24, s * 0.69)),
                (V3(-1.50, 0.47, s * 0.26), V3(-1.84, 0.46, s * 0.62)), (V3(-1.95, 0.42, s * 0.22), V3(-1.84, 0.46, s * 0.62)),
                (V3(-1.55, 0.20, s * 0.30), V3(-1.86, 0.24, s * 0.62)),
            ]
            for (a, b) in arms { m.tube([a, b], 0.016, CARBON) }
        }
        return m
    }
}

// MARK: - Scène commune

enum Studio {
    /// Environnement de reflets : ciel clair en haut, sol sombre en bas.
    static let environment: UIImage = {
        let size = CGSize(width: 512, height: 256)
        return UIGraphicsImageRenderer(size: size).image { ctx in
            let colors = [UIColor(white: 0.85, alpha: 1).cgColor, UIColor(white: 0.35, alpha: 1).cgColor,
                          UIColor(white: 0.06, alpha: 1).cgColor, UIColor(white: 0.02, alpha: 1).cgColor] as CFArray
            let g = CGGradient(colorsSpace: CGColorSpaceCreateDeviceRGB(), colors: colors, locations: [0, 0.42, 0.55, 1])!
            ctx.cgContext.drawLinearGradient(g, start: .zero, end: CGPoint(x: 0, y: size.height), options: [])
        }
    }()

    static func scene() -> SCNScene {
        let scene = SCNScene()
        scene.lightingEnvironment.contents = environment
        scene.lightingEnvironment.intensity = 1.2
        let key = SCNNode()
        key.light = SCNLight()
        key.light!.type = .directional
        key.light!.intensity = 1300
        key.light!.castsShadow = true
        key.light!.shadowMode = .deferred
        key.light!.shadowRadius = 6
        key.light!.shadowSampleCount = 8
        key.light!.shadowColor = UIColor(white: 0, alpha: 0.6)
        key.eulerAngles = SCNVector3(-1.0, 0.6, 0)
        scene.rootNode.addChildNode(key)
        let fill = SCNNode()
        fill.light = SCNLight()
        fill.light!.type = .ambient
        fill.light!.intensity = 250
        scene.rootNode.addChildNode(fill)
        return scene
    }

    static func camera(fov: CGFloat = 35, far: Double = 200) -> SCNNode {
        let cam = SCNNode()
        cam.camera = SCNCamera()
        cam.camera!.fieldOfView = fov
        cam.camera!.zFar = far
        cam.camera!.zNear = 0.05
        cam.camera!.wantsHDR = true
        cam.camera!.bloomIntensity = 0.15
        return cam
    }
}

// MARK: - Vue monoplace

struct CarSceneView: UIViewRepresentable {
    let livery: Livery

    func makeUIView(context: Context) -> SCNView {
        let view = SCNView()
        view.backgroundColor = .clear
        view.antialiasingMode = .multisampling4X
        view.allowsCameraControl = true
        view.defaultCameraController.interactionMode = .orbitTurntable
        view.defaultCameraController.maximumVerticalAngle = 80
        view.defaultCameraController.minimumVerticalAngle = -5
        let scene = Studio.scene()
        let car = CarModel.node(livery: livery)
        car.name = "car"
        car.runAction(.repeatForever(.rotateBy(x: 0, y: CGFloat.pi * 2, z: 0, duration: 24)))
        scene.rootNode.addChildNode(car)
        let floor = SCNNode(geometry: SCNFloor())
        (floor.geometry as! SCNFloor).reflectivity = 0
        floor.geometry!.firstMaterial!.lightingModel = .shadowOnly
        scene.rootNode.addChildNode(floor)
        let cam = Studio.camera()
        cam.position = SCNVector3(5.2, 2.2, 5.6)
        cam.look(at: SCNVector3(0, 0.35, 0))
        scene.rootNode.addChildNode(cam)
        view.pointOfView = cam
        view.scene = scene
        return view
    }

    func updateUIView(_ view: SCNView, context: Context) {
        guard let car = view.scene?.rootNode.childNode(withName: "car", recursively: false) else { return }
        car.geometry = CarModel.node(livery: livery).geometry
    }
}

// MARK: - Circuit en relief

/// Ligne médiane du circuit dans le repère 3D, avec les temps du tour de référence.
final class TrackPath {
    let pts: [V3]
    let t: [Float]
    let lapTime: Float
    let radius: Float
    let centreY: Float

    static let relief: Float = 4
    static let halfWidth: Float = 8
    static let carScale: Float = 7

    init(_ map: TrackMap) {
        let cx = Float(map.width / 2), cz = Float(map.height / 2)
        pts = map.points.map { V3(Float($0.x) - cx, Float($0.z ?? 0) * Self.relief, Float($0.y) - cz) }
        t = map.points.map { Float($0.t) }
        lapTime = Float(map.lap_time)
        radius = (cx * cx + cz * cz).squareRoot()
        centreY = (pts.map(\.y).max() ?? 0) / 2
    }

    /// Position et cap (radians autour de Y) au temps donné du tour.
    func at(time: Float) -> (V3, Float) {
        let n = pts.count
        let time = time.truncatingRemainder(dividingBy: max(lapTime, 1))
        var lo = 0, hi = n - 1
        while lo < hi {
            let mid = (lo + hi) / 2
            if t[mid] <= time { lo = mid + 1 } else { hi = mid }
        }
        let i = max(min(lo, n - 1), 1) - 1
        let j = min(i + 1, n - 1)
        let k = max(0, min(1, (time - t[i]) / max(t[j] - t[i], 0.001)))
        let pos = pts[i] + (pts[j] - pts[i]) * k
        let d = pts[min(i + 2, n - 1)] - pts[max(i - 1, 0)]
        return (pos, atan2(-d.z, d.x))
    }
}

enum TrackModel {
    static func node(_ map: TrackMap, path: TrackPath) -> SCNNode {
        let pts = path.pts, n = pts.count
        let closed = simd_length(pts[0] - pts[n - 1]) < 60
        func side(_ i: Int) -> V3 {
            let prev = i == 0 ? (closed ? n - 1 : 0) : i - 1
            let next = i + 1 == n ? (closed ? 0 : n - 1) : i + 1
            let d = pts[next] - pts[prev]
            return simd_normalize(simd_cross(V3(d.x, 0, d.z), V3(0, 1, 0)))
        }
        let minS = Float(map.stats.min_speed), maxS = Float(map.stats.top_speed)
        let ground: Float = -6
        var pos: [SCNVector3] = [], nrm: [SCNVector3] = [], col: [SIMD4<Float>] = []
        func add(_ p: [V3], _ n: V3, _ c: SIMD4<Float>) {
            for i in [0, 1, 2, 0, 2, 3] {
                pos.append(SCNVector3(p[i].x, p[i].y, p[i].z))
                nrm.append(SCNVector3(n.x, n.y, n.z))
                col.append(c)
            }
        }
        func ramp(_ r: Float) -> SIMD4<Float> {
            let r = max(0, min(1, r))
            return SIMD4((179 + 76 * r) / 255, (38 + 190 * r) / 255, (30 + 192 * r) / 255, 1)
        }
        let segs = closed ? n : n - 1
        for i in 0..<segs {
            let j = (i + 1) % n
            let si = side(i), sj = side(j), hw = TrackPath.halfWidth
            let li = pts[i] + si * hw, ri = pts[i] - si * hw, lj = pts[j] + sj * hw, rj = pts[j] - sj * hw
            let ratio = (Float(map.points[i].speed) - minS) / max(maxS - minS, 1)
            add([li, ri, rj, lj], V3(0, 1, 0), ramp(ratio))
            for (a, b, s) in [(li, lj, Float(1)), (ri, rj, Float(-1))] {
                let oa = a + si * (s * 1.1), ob = b + sj * (s * 1.1)
                add(s > 0 ? [a, oa, ob, b] : [a, b, ob, oa], V3(0, 1, 0), SIMD4(0.62, 0.62, 0.68, 1))
                let ga = V3(oa.x, ground, oa.z), gb = V3(ob.x, ground, ob.z)
                let out = si * s
                add(s > 0 ? [oa, ga, gb, ob] : [oa, ob, gb, ga], out, SIMD4(0.20, 0.20, 0.24, 1))
            }
        }
        let colorData = col.withUnsafeBufferPointer { Data(buffer: $0) }
        let colors = SCNGeometrySource(data: colorData, semantic: .color, vectorCount: col.count, usesFloatComponents: true,
                                       componentsPerVector: 4, bytesPerComponent: 4, dataOffset: 0,
                                       dataStride: MemoryLayout<SIMD4<Float>>.stride)
        let geo = SCNGeometry(sources: [SCNGeometrySource(vertices: pos), SCNGeometrySource(normals: nrm), colors],
                              elements: [SCNGeometryElement(indices: (0..<Int32(pos.count)).map { $0 }, primitiveType: .triangles)])
        let mat = SCNMaterial()
        mat.lightingModel = .physicallyBased
        mat.diffuse.contents = UIColor.white
        mat.roughness.contents = 0.85
        mat.isDoubleSided = true
        geo.materials = [mat]
        let node = SCNNode(geometry: geo)

        // Sol quadrillé.
        let ext = CGFloat(path.radius + 160)
        let plane = SCNPlane(width: ext * 2, height: ext * 2)
        let pm = SCNMaterial()
        pm.diffuse.contents = UIColor(red: 0.085, green: 0.085, blue: 0.105, alpha: 1)
        pm.lightingModel = .lambert
        plane.materials = [pm]
        let floor = SCNNode(geometry: plane)
        floor.eulerAngles.x = -.pi / 2
        floor.position.y = ground
        node.addChildNode(floor)
        var g = -ext + 50
        while g < ext {
            for vertical in [true, false] {
                let line = SCNNode(geometry: SCNBox(width: vertical ? 1.6 : ext * 2, height: 0.05, length: vertical ? ext * 2 : 1.6, chamferRadius: 0))
                line.geometry!.firstMaterial!.diffuse.contents = UIColor(red: 0.15, green: 0.15, blue: 0.18, alpha: 1)
                line.geometry!.firstMaterial!.lightingModel = .constant
                line.position = SCNVector3(vertical ? Float(g) : 0, ground + 0.1, vertical ? 0 : Float(g))
                node.addChildNode(line)
            }
            g += 100
        }
        // Ligne de départ.
        let start = SCNNode(geometry: SCNBox(width: 3.2, height: 0.3, length: CGFloat(TrackPath.halfWidth * 2), chamferRadius: 0))
        start.geometry!.firstMaterial!.diffuse.contents = UIColor.white
        start.geometry!.firstMaterial!.lightingModel = .constant
        let (p0, h0) = path.at(time: 0)
        start.position = SCNVector3(p0.x, p0.y + 0.2, p0.z)
        start.eulerAngles.y = h0
        node.addChildNode(start)
        return node
    }
}

/// Voiture placée sur le circuit (Race Center).
struct TrackMarker: Equatable {
    let key: String
    let label: String
    let colour: String
    let fraction: Double
}

struct TrackSceneView: UIViewRepresentable {
    let map: TrackMap
    /// Une voiture rejoue le tour de référence à vitesse réelle.
    var ghost = true
    var chase = false
    var markers: [TrackMarker] = []

    func makeCoordinator() -> Coordinator { Coordinator(map: map, ghost: ghost) }

    func makeUIView(context: Context) -> SCNView {
        let view = SCNView()
        view.backgroundColor = .clear
        view.antialiasingMode = .multisampling4X
        view.allowsCameraControl = true
        view.defaultCameraController.interactionMode = .orbitTurntable
        view.defaultCameraController.maximumVerticalAngle = 89
        view.defaultCameraController.minimumVerticalAngle = 5
        view.scene = context.coordinator.scene
        view.pointOfView = context.coordinator.overview
        view.delegate = context.coordinator
        view.isPlaying = true
        context.coordinator.view = view
        return view
    }

    func updateUIView(_ view: SCNView, context: Context) {
        let c = context.coordinator
        c.chase = chase && ghost
        view.allowsCameraControl = !c.chase
        view.pointOfView = c.chase ? c.chaseCam : c.overview
        c.update(markers: markers)
    }

    final class Coordinator: NSObject, SCNSceneRendererDelegate {
        let scene = Studio.scene()
        let path: TrackPath
        let overview = Studio.camera(fov: 40, far: 20_000)
        let chaseCam = Studio.camera(fov: 55, far: 20_000)
        let ghostCar: SCNNode?
        var chase = false
        weak var view: SCNView?
        private var cars: [String: (node: SCNNode, target: Float, cur: Float)] = [:]
        private let lock = NSLock()
        private var last: TimeInterval = 0
        private var clock: Float = 0

        init(map: TrackMap, ghost: Bool) {
            path = TrackPath(map)
            ghostCar = ghost ? CarModel.node(livery: .single(UIColor(Color(hexString: map.colour)))) : nil
            super.init()
            scene.rootNode.addChildNode(TrackModel.node(map, path: path))
            let dist = path.radius / sin(20 * .pi / 180) * 0.95
            overview.position = SCNVector3(dist * 0.45, path.centreY + dist * 0.75, dist * 0.5)
            overview.look(at: SCNVector3(0, path.centreY, 0))
            scene.rootNode.addChildNode(overview)
            scene.rootNode.addChildNode(chaseCam)
            if let ghostCar {
                ghostCar.scale = SCNVector3(TrackPath.carScale, TrackPath.carScale, TrackPath.carScale)
                scene.rootNode.addChildNode(ghostCar)
            }
        }

        func update(markers: [TrackMarker]) {
            lock.lock()
            defer { lock.unlock() }
            var seen = Set<String>()
            for m in markers {
                seen.insert(m.key)
                let f = Float(m.fraction)
                if var entry = cars[m.key] {
                    entry.target = f
                    cars[m.key] = entry
                    (entry.node.childNode(withName: "label", recursively: false)?.geometry as? SCNPlane)?.firstMaterial?.diffuse.contents = Self.badge(m.label, m.colour)
                } else {
                    let node = CarModel.node(livery: .single(UIColor(Color(hexString: m.colour))))
                    node.scale = SCNVector3(TrackPath.carScale, TrackPath.carScale, TrackPath.carScale)
                    let plane = SCNPlane(width: 1.6, height: 1.6)
                    plane.firstMaterial?.diffuse.contents = Self.badge(m.label, m.colour)
                    plane.firstMaterial?.lightingModel = .constant
                    plane.firstMaterial?.readsFromDepthBuffer = false
                    let label = SCNNode(geometry: plane)
                    label.name = "label"
                    label.position = SCNVector3(0, 2.4, 0)
                    label.constraints = [SCNBillboardConstraint()]
                    label.renderingOrder = 10
                    node.addChildNode(label)
                    scene.rootNode.addChildNode(node)
                    cars[m.key] = (node, f, f)
                }
            }
            for key in cars.keys where !seen.contains(key) {
                cars[key]?.node.removeFromParentNode()
                cars[key] = nil
            }
        }

        static func badge(_ text: String, _ colour: String) -> UIImage {
            let size = CGSize(width: 64, height: 64)
            return UIGraphicsImageRenderer(size: size).image { _ in
                UIColor(Color(hexString: colour)).setFill()
                UIBezierPath(ovalIn: CGRect(x: 4, y: 4, width: 56, height: 56)).fill()
                let attrs: [NSAttributedString.Key: Any] = [.font: UIFont.systemFont(ofSize: 28, weight: .heavy), .foregroundColor: UIColor.black]
                let s = NSString(string: text)
                let b = s.size(withAttributes: attrs)
                s.draw(at: CGPoint(x: (size.width - b.width) / 2, y: (size.height - b.height) / 2), withAttributes: attrs)
            }
        }

        func renderer(_ renderer: SCNSceneRenderer, updateAtTime time: TimeInterval) {
            let dt = Float(last == 0 ? 0 : min(time - last, 0.1))
            last = time
            clock += dt
            if let ghostCar {
                let (p, h) = path.at(time: clock)
                ghostCar.position = SCNVector3(p.x, p.y + 0.6, p.z)
                ghostCar.eulerAngles.y = h
                if chase {
                    let fwd = V3(cos(h), 0, -sin(h))
                    let want = p - fwd * 75 + V3(0, 26, 0)
                    let target = p + fwd * 30 + V3(0, 4, 0)
                    let cur = chaseCam.simdPosition
                    let k = min(dt * 4, 1)
                    chaseCam.simdPosition = cur == .zero ? want : cur + (want - cur) * k
                    chaseCam.simdLook(at: target)
                }
            }
            lock.lock()
            defer { lock.unlock() }
            for (key, entry) in cars {
                var e = entry
                let diff = (e.target - e.cur + 0.5).truncatingRemainder(dividingBy: 1) - 0.5
                let wrapped = diff < -0.5 ? diff + 1 : diff
                e.cur = (e.cur + wrapped * min(dt * 2.5, 1)).truncatingRemainder(dividingBy: 1)
                if e.cur < 0 { e.cur += 1 }
                let (p, h) = path.at(time: e.cur * path.lapTime)
                e.node.position = SCNVector3(p.x, p.y + 0.6, p.z)
                e.node.eulerAngles.y = h
                cars[key] = e
            }
        }
    }
}

// MARK: - Cartes SwiftUI

/// Carte « monoplace en 3D » (page écurie / pilote), avec plein écran.
struct CarCard: View {
    let constructorId: String
    let teamName: String
    @State private var full = false

    var body: some View {
        SectionCard(title: L("La monoplace en 3D", "The car in 3D")) {
            ZStack(alignment: .bottomTrailing) {
                CarSceneView(livery: .team(constructorId))
                    .frame(height: 230)
                    .background(RadialGradient(colors: [Color(white: 0.16), Color(white: 0.05)], center: .top, startRadius: 10, endRadius: 320),
                                in: RoundedRectangle(cornerRadius: 14))
                Button { full = true } label: { Label(L("Plein écran", "Full screen"), systemImage: "arrow.up.left.and.arrow.down.right") }
                    .buttonStyle(.borderedProminent).tint(.black.opacity(0.6)).padding(8)
            }
            Text(L("Monoplace stylisée aux couleurs \(teamName), générée par le code (pas une reproduction officielle). Fais-la tourner, pince pour zoomer, deux doigts pour la déplacer.",
                   "Stylised car in \(teamName) colours, generated by code (not an official replica). Spin it, pinch to zoom, two fingers to move it."))
                .font(.footnote).foregroundStyle(.secondary)
        }
        .fullScreenCover(isPresented: $full) {
            ZStack(alignment: .topTrailing) {
                Color.black.ignoresSafeArea()
                CarSceneView(livery: .team(constructorId)).ignoresSafeArea()
                Button { full = false } label: { Image(systemName: "xmark.circle.fill").font(.largeTitle) }
                    .tint(.white).padding()
            }
        }
    }
}

/// Circuit en relief, avec voiture fantôme et caméra embarquée.
struct Track3DView: View {
    let map: TrackMap
    var markers: [TrackMarker] = []
    var ghost = true
    @State private var chase = false
    @State private var full = false

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            ZStack(alignment: .bottomTrailing) {
                TrackSceneView(map: map, ghost: ghost, chase: chase, markers: markers)
                    .frame(height: 320)
                    .background(Color(white: 0.06), in: RoundedRectangle(cornerRadius: 14))
                    .clipShape(RoundedRectangle(cornerRadius: 14))
                controls.padding(8)
            }
            Text(map.hasRelief
                 ? L("Tracé GPS réel avec son relief (dénivelé ×4), coloré selon la vitesse.", "Real GPS layout with elevation (×4), coloured by speed.")
                 : L("Tracé GPS réel coloré selon la vitesse.", "Real GPS layout coloured by speed."))
                .font(.footnote).foregroundStyle(.secondary)
        }
        .fullScreenCover(isPresented: $full) {
            ZStack(alignment: .topTrailing) {
                Color.black.ignoresSafeArea()
                TrackSceneView(map: map, ghost: ghost, chase: chase, markers: markers).ignoresSafeArea()
                VStack(alignment: .trailing) {
                    Button { full = false } label: { Image(systemName: "xmark.circle.fill").font(.largeTitle) }.tint(.white)
                    Spacer()
                    controls
                }
                .padding()
            }
        }
    }

    private var controls: some View {
        HStack(spacing: 8) {
            if ghost {
                Button(chase ? L("Vue d'ensemble", "Overview") : L("Caméra embarquée", "Onboard")) { chase.toggle() }
                    .buttonStyle(.borderedProminent).tint(chase ? .f1Red : .black.opacity(0.6))
            }
            if !full {
                Button { full = true } label: { Image(systemName: "arrow.up.left.and.arrow.down.right") }
                    .buttonStyle(.borderedProminent).tint(.black.opacity(0.6))
            }
        }
        .font(.footnote.bold())
    }
}
