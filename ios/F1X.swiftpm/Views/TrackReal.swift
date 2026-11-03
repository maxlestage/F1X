import Combine
import MapKit
import SwiftUI
import simd

// Vue réelle d'un circuit : Apple Plans en 3D satellite (vrai relief, vrais bâtiments),
// avec la trajectoire GPS du meilleur tour et les monoplaces qui roulent dessus.
//
// Les positions OpenF1 sont dans un repère local sans latitude ni longitude : on les cale
// sur le tracé géographique réel du circuit (bacinger/f1-circuits, licence MIT) par une
// similitude (rotation, échelle, translation, miroir) ajustée au plus proche (ICP).

// MARK: - Tracés géographiques

enum RealCircuits {
    /// Identifiant Jolpica → identifiant du jeu de tracés.
    private static let ids: [String: String] = [
        "albert_park": "au-1953", "bahrain": "bh-2002", "shanghai": "cn-2004", "catalunya": "es-1991",
        "monaco": "mc-1929", "villeneuve": "ca-1978", "ricard": "fr-1969", "red_bull_ring": "at-1969",
        "silverstone": "gb-1948", "hockenheimring": "de-1932", "hungaroring": "hu-1986", "spa": "be-1925",
        "monza": "it-1922", "marina_bay": "sg-2008", "sochi": "ru-2014", "suzuka": "jp-1962",
        "americas": "us-2012", "rodriguez": "mx-1962", "interlagos": "br-1940", "yas_marina": "ae-2009",
        "imola": "it-1953", "nurburgring": "de-1927", "portimao": "pt-2008", "mugello": "it-1914",
        "sepang": "my-1999", "istanbul": "tr-2005", "zandvoort": "nl-1948", "magny_cours": "fr-1960",
        "estoril": "pt-1972", "jacarepagua": "br-1977", "jeddah": "sa-2021", "miami": "us-2022",
        "losail": "qa-2004", "madring": "es-2026", "baku": "az-2016", "vegas": "us-2023",
        "indianapolis": "us-1909", "galvez": "ar-1952", "kyalami": "za-1961", "watkins_glen": "us-1956",
    ]

    private struct File: Decodable {
        struct Feature: Decodable {
            struct Props: Decodable { let id: String }
            struct Geometry: Decodable { let coordinates: [[Double]] }
            let properties: Props
            let geometry: Geometry
        }
        let features: [Feature]
    }

    private static let lines: [String: [CLLocationCoordinate2D]] = {
        guard let url = Bundle.main.url(forResource: "f1-circuits", withExtension: "geojson"),
              let data = try? Data(contentsOf: url),
              let file = try? JSONDecoder().decode(File.self, from: data) else { return [:] }
        var out: [String: [CLLocationCoordinate2D]] = [:]
        for f in file.features {
            out[f.properties.id] = f.geometry.coordinates.compactMap {
                $0.count >= 2 ? CLLocationCoordinate2D(latitude: $0[1], longitude: $0[0]) : nil
            }
        }
        return out
    }()

    static func line(_ circuitId: String) -> [CLLocationCoordinate2D]? {
        ids[circuitId].flatMap { lines[$0] }.flatMap { $0.count > 10 ? $0 : nil }
    }
}

// MARK: - Calage du tracé GPS sur la carte

enum TrackGeoFit {
    private static var cache: [String: [CLLocationCoordinate2D]] = [:]
    private static let lock = NSLock()

    /// Latitude/longitude de chaque point du tracé OpenF1, ou nil si le calage échoue.
    static func coordinates(_ map: TrackMap) async -> [CLLocationCoordinate2D]? {
        let key = "\(map.circuit_id)-\(map.points.count)-\(map.points.first?.x ?? 0)"
        lock.lock()
        let hit = cache[key]
        lock.unlock()
        if let hit { return hit }
        guard let line = RealCircuits.line(map.circuit_id) else { return nil }
        let pts = map.points.map { SIMD2<Double>($0.x, $0.y) }
        let length = map.stats.length_km * 1000
        let result = await Task.detached(priority: .userInitiated) { fit(pts, length: length, line: line) }.value
        if let result {
            lock.lock()
            cache[key] = result
            lock.unlock()
        }
        return result
    }

    private static let earth = 6_371_000.0

    static func fit(_ raw: [SIMD2<Double>], length: Double, line: [CLLocationCoordinate2D]) -> [CLLocationCoordinate2D]? {
        guard raw.count > 10 else { return nil }
        // Tracé réel en mètres autour de son centre (projection locale), densifié tous les 4 m.
        let lat0 = line.map(\.latitude).reduce(0, +) / Double(line.count)
        let lon0 = line.map(\.longitude).reduce(0, +) / Double(line.count)
        let kx = .pi / 180 * earth * cos(lat0 * .pi / 180), ky = .pi / 180 * earth
        let real = line.map { SIMD2(($0.longitude - lon0) * kx, ($0.latitude - lat0) * ky) }
        var dense: [SIMD2<Double>] = []
        for i in 0..<real.count {
            let a = real[i], b = real[(i + 1) % real.count]
            let n = max(1, Int(simd_length(b - a) / 4))
            for k in 0..<n { dense.append(a + (b - a) * Double(k) / Double(n)) }
        }
        // Échelle de départ : longueur officielle du tour / longueur du tracé OpenF1.
        var span = 0.0
        for i in 1..<raw.count { span += simd_length(raw[i] - raw[i - 1]) }
        guard span > 0 else { return nil }
        let s0 = (length > 100 ? length : real.indices.dropFirst().reduce(0) { $0 + simd_length(real[$1] - real[$1 - 1]) }) / span
        let step = max(1, raw.count / 180)
        let sample = Swift.stride(from: 0, to: raw.count, by: step).map { raw[$0] }
        let centre = sample.reduce(SIMD2<Double>(0, 0), +) / Double(sample.count)
        let realCentre = dense.reduce(SIMD2<Double>(0, 0), +) / Double(dense.count)

        func nearest(_ q: SIMD2<Double>) -> (SIMD2<Double>, Double) {
            var best = dense[0], bd = Double.infinity
            for d in dense {
                let e = simd_length_squared(d - q)
                if e < bd { bd = e; best = d }
            }
            return (best, bd.squareRoot())
        }
        typealias Pose = (rot: simd_double2x2, s: Double, t: SIMD2<Double>, mirror: Double)
        func apply(_ p: SIMD2<Double>, _ pose: Pose) -> SIMD2<Double> {
            let local = SIMD2(p.x - centre.x, (p.y - centre.y) * pose.mirror)
            return pose.rot * local * pose.s + pose.t
        }
        func refine(_ start: Pose, _ iterations: Int) -> (Pose, Double) {
            var pose = start
            var err = Double.infinity
            for _ in 0..<iterations {
                let moved = sample.map { apply($0, pose) }
                let pairs = moved.map { nearest($0) }
                let cut = pairs.map { $0.1 }.sorted()[Int(Double(pairs.count) * 0.9)]
                var a: [SIMD2<Double>] = [], b: [SIMD2<Double>] = []
                for (i, p) in pairs.enumerated() where p.1 <= cut {
                    a.append(SIMD2(sample[i].x - centre.x, (sample[i].y - centre.y) * pose.mirror))
                    b.append(p.0)
                }
                err = pairs.map { $0.1 }.reduce(0, +) / Double(pairs.count)
                guard a.count > 3 else { break }
                // Similitude de Umeyama (2D) : rotation, échelle et translation optimales.
                let ma = a.reduce(SIMD2<Double>(0, 0), +) / Double(a.count)
                let mb = b.reduce(SIMD2<Double>(0, 0), +) / Double(b.count)
                var sxx = 0.0, sxy = 0.0, syx = 0.0, syy = 0.0, va = 0.0
                for (p, q) in zip(a, b) {
                    let u = p - ma, v = q - mb
                    sxx += v.x * u.x; sxy += v.x * u.y; syx += v.y * u.x; syy += v.y * u.y
                    va += simd_length_squared(u)
                }
                let angle = atan2(syx - sxy, sxx + syy)
                let c = cos(angle), s = sin(angle)
                let scale = ((sxx + syy) * c + (syx - sxy) * s) / max(va, 1e-9)
                let rot = simd_double2x2(columns: (SIMD2(c, s), SIMD2(-s, c)))
                pose = (rot, scale, mb - rot * ma * scale, pose.mirror)
            }
            return (pose, err)
        }
        var best: (Pose, Double)?
        for mirror in [-1.0, 1.0] {
            for deg in Swift.stride(from: 0.0, to: 360.0, by: 15.0) {
                let a = deg * .pi / 180
                let rot = simd_double2x2(columns: (SIMD2(cos(a), sin(a)), SIMD2(-sin(a), cos(a))))
                let r = refine((rot, s0, realCentre, mirror), 10)
                if best == nil || r.1 < best!.1 { best = r }
            }
        }
        guard let start = best else { return nil }
        let (pose, err) = refine(start.0, 25)
        // Calage douteux (tracé modifié, mauvais circuit) : pas de vue réelle plutôt qu'un tracé faux.
        guard err < 25, pose.s > s0 * 0.7, pose.s < s0 * 1.4 else { return nil }
        return raw.map {
            let m = apply($0, pose)
            return CLLocationCoordinate2D(latitude: lat0 + m.y / ky, longitude: lon0 + m.x / kx)
        }
    }
}

// MARK: - Vue

struct TrackRealView: View {
    let map: TrackMap
    var markers: [TrackMarker] = []
    var ghost = true

    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @State private var coords: [CLLocationCoordinate2D]?
    /// Tracé fermé pour la polyligne, calculé une fois (et non à chaque image du chrono).
    @State private var loop: [CLLocationCoordinate2D] = []
    @State private var failed = false
    @State private var position: MapCameraPosition = .automatic
    @State private var clock: Double = 0
    @State private var speed: Double = 1
    @State private var follow = false
    @State private var labels = true
    @State private var full = false
    private let tick = Timer.publish(every: 1.0 / 24, on: .main, in: .common).autoconnect()

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            ZStack(alignment: .bottom) {
                Group {
                    if full {
                        Color(white: 0.06)
                    } else {
                        content
                    }
                }
                .frame(height: 340)
                .clipShape(RoundedRectangle(cornerRadius: 14))
                if coords != nil { controls.padding(8) }
            }
            .environment(\.colorScheme, .dark)
            Text(L("Vue réelle Apple Plans (relief et bâtiments 3D). Ligne rouge : trajectoire GPS du meilleur tour, calée sur le tracé réel du circuit.",
                   "Real Apple Maps view (3D terrain and buildings). Red line: GPS racing line of the fastest lap, aligned on the real circuit layout."))
                .font(.footnote).foregroundStyle(.secondary)
        }
        .task(id: map.circuit_id) {
            coords = await TrackGeoFit.coordinates(map)
            loop = coords.map { c in c.first.map { c + [$0] } ?? c } ?? []
            failed = coords == nil
            if let coords { position = .camera(overview(coords)) }
        }
        .onReceive(tick) { _ in advance() }
        .fullScreenCover(isPresented: $full) {
            ZStack(alignment: .bottom) {
                content.ignoresSafeArea()
                VStack {
                    HStack {
                        Spacer()
                        Button { full = false } label: { Image(systemName: "xmark.circle.fill").font(.largeTitle) }
                            .tint(.white)
                            .accessibilityLabel(L("Fermer", "Close"))
                    }
                    Spacer()
                    controls
                }
                .padding()
            }
            .environment(\.colorScheme, .dark)
        }
    }

    @ViewBuilder
    private var content: some View {
        if let coords {
            Map(position: $position, interactionModes: follow ? [] : .all) {
                MapPolyline(coordinates: loop)
                    .stroke(Color(red: 0.95, green: 0.1, blue: 0.08).opacity(0.85), lineWidth: 3)
                ForEach(cars(coords), id: \.key) { car in
                    Annotation(labels ? car.label : "", coordinate: car.coordinate, anchor: .center) {
                        Circle()
                            .fill(Color(hexString: car.colour))
                            .frame(width: 13, height: 13)
                            .overlay(Circle().stroke(.white, lineWidth: 2))
                            .shadow(radius: 2)
                    }
                }
            }
            .mapStyle(.imagery(elevation: .realistic))
            .mapControls { MapCompass() }
        } else if failed {
            VStack(spacing: 8) {
                Image(systemName: "map").font(.title2)
                Text(L("Vue réelle indisponible pour ce circuit.", "Real view unavailable for this circuit."))
                    .font(.footnote)
            }
            .foregroundStyle(.secondary)
            .frame(maxWidth: .infinity, maxHeight: .infinity)
            .background(Color(white: 0.08))
        } else {
            ProgressView()
                .frame(maxWidth: .infinity, maxHeight: .infinity)
                .background(Color(white: 0.08))
        }
    }

    private var controls: some View {
        HStack(spacing: 8) {
            if !reduceMotion {
                Button { follow.toggle(); if !follow, let coords { position = .camera(overview(coords)) } } label: {
                    Label(follow ? L("Suivi", "Following") : L("Vue d'ensemble", "Overview"),
                          systemImage: follow ? "car.rear" : "map")
                        .lineLimit(1).fixedSize()
                        .padding(.horizontal, 12).padding(.vertical, 8)
                        .background(.black.opacity(0.65), in: Capsule())
                }
            }
            Spacer(minLength: 0)
            if ghost && markers.isEmpty && !reduceMotion {
                Button { speed = speed >= 4 ? 1 : speed * 2 } label: { Text("×\(Int(speed))").monospacedDigit() }
                    .frame(width: 38, height: 34).background(.black.opacity(0.65), in: Capsule())
                    .accessibilityLabel(L("Vitesse de lecture", "Playback speed"))
            }
            Button { labels.toggle() } label: { Image(systemName: labels ? "tag.fill" : "tag.slash") }
                .frame(width: 34, height: 34).background(.black.opacity(0.65), in: Circle())
                .accessibilityLabel(L("Afficher les noms", "Show names"))
            Button { follow = false; if let coords { position = .camera(overview(coords)) } } label: { Image(systemName: "arrow.counterclockwise") }
                .frame(width: 34, height: 34).background(.black.opacity(0.65), in: Circle())
                .accessibilityLabel(L("Recentrer", "Recentre"))
            if !full {
                Button { full = true } label: { Image(systemName: "arrow.up.left.and.arrow.down.right") }
                    .frame(width: 34, height: 34).background(.black.opacity(0.65), in: Circle())
                    .accessibilityLabel(L("Plein écran", "Full screen"))
            }
        }
        .font(.footnote.bold())
        .foregroundStyle(.white)
    }

    // MARK: Voitures et caméra

    private struct Car {
        let key: String
        let label: String
        let colour: String
        let coordinate: CLLocationCoordinate2D
        let heading: Double
    }

    private func cars(_ coords: [CLLocationCoordinate2D]) -> [Car] {
        if !markers.isEmpty {
            return markers.map { m in
                let (c, h) = at(time: m.fraction * map.lap_time, coords)
                return Car(key: m.key, label: m.label, colour: m.colour, coordinate: c, heading: h)
            }
        }
        guard ghost else { return [] }
        let (c, h) = at(time: clock, coords)
        let name = map.driver.split(separator: " ").last.map(String.init) ?? map.driver
        return [Car(key: "ghost", label: name, colour: map.colour, coordinate: c, heading: h)]
    }

    private func advance() {
        // « Réduire les animations » : la voiture fantôme reste immobile sur la ligne de départ.
        guard !reduceMotion, let coords else { return }
        if markers.isEmpty && ghost {
            clock = (clock + speed / 24).truncatingRemainder(dividingBy: max(map.lap_time, 1))
        }
        guard follow, let car = cars(coords).first else { return }
        position = .camera(MapCamera(centerCoordinate: car.coordinate, distance: 420, heading: car.heading, pitch: 70))
    }

    /// Position et cap (degrés, nord = 0) au temps donné du tour.
    private func at(time: Double, _ coords: [CLLocationCoordinate2D]) -> (CLLocationCoordinate2D, Double) {
        let pts = map.points
        let n = min(pts.count, coords.count)
        guard n > 1 else { return (coords.first ?? CLLocationCoordinate2D(), 0) }
        let t = time.truncatingRemainder(dividingBy: max(map.lap_time, 1))
        var lo = 0, hi = n - 1
        while lo < hi {
            let mid = (lo + hi) / 2
            if pts[mid].t <= t { lo = mid + 1 } else { hi = mid }
        }
        let i = max(min(lo, n - 1), 1) - 1, j = min(i + 1, n - 1)
        let k = max(0, min(1, (t - pts[i].t) / max(pts[j].t - pts[i].t, 0.001)))
        let a = coords[i], b = coords[j]
        let c = CLLocationCoordinate2D(latitude: a.latitude + (b.latitude - a.latitude) * k,
                                       longitude: a.longitude + (b.longitude - a.longitude) * k)
        let p = coords[max(i - 1, 0)], q = coords[min(j + 1, n - 1)]
        let dx = (q.longitude - p.longitude) * cos(c.latitude * .pi / 180), dy = q.latitude - p.latitude
        return (c, atan2(dx, dy) * 180 / .pi)
    }

    /// Caméra d'ensemble inclinée qui cadre tout le circuit.
    private func overview(_ coords: [CLLocationCoordinate2D]) -> MapCamera {
        let lats = coords.map(\.latitude), lons = coords.map(\.longitude)
        let (minLat, maxLat) = (lats.min() ?? 0, lats.max() ?? 0)
        let (minLon, maxLon) = (lons.min() ?? 0, lons.max() ?? 0)
        let centre = CLLocationCoordinate2D(latitude: (minLat + maxLat) / 2, longitude: (minLon + maxLon) / 2)
        let h = (maxLat - minLat) * 111_000
        let w = (maxLon - minLon) * 111_000 * cos(centre.latitude * .pi / 180)
        return MapCamera(centerCoordinate: centre, distance: max(max(w, h) * 2.4, 800), heading: 20, pitch: 55)
    }
}

/// Affichage du tracé : plan 2D (télémétrie), relief 3D dessiné, ou vue réelle Apple Plans.
enum TrackViewMode: String, CaseIterable, Identifiable {
    case map, relief, real
    var id: String { rawValue }
    var label: String {
        switch self {
        case .map: return L("Plan 2D", "2D map")
        case .relief: return L("Relief 3D", "3D relief")
        case .real: return L("Réel", "Real")
        }
    }
}
