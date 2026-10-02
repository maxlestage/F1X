import SwiftUI

// Photos libres (Wikimedia Commons, via le serveur F1X) et tracés 2D des circuits.

/// Avatar rond : photo libre si elle existe, sinon initiales sur la couleur de l'écurie.
struct Avatar: View {
    let name: String
    let wikipedia: String?
    var color: Color = .gray
    var size: CGFloat = 36

    @State private var photo: Photo?

    var body: some View {
        ZStack {
            Circle().fill(color.opacity(0.85))
            Text(initials).font(.system(size: size * 0.38, weight: .heavy)).foregroundStyle(.black.opacity(0.75))
            if let src = photo.flatMap({ URL(string: $0.src) }) {
                AsyncImage(url: src) { phase in
                    if let image = phase.image {
                        image.resizable().scaledToFill()
                    }
                }
                .clipShape(Circle())
            }
        }
        .frame(width: size, height: size)
        .overlay(Circle().stroke(color, lineWidth: 2))
        .task(id: wikipedia) { photo = await ServerAPI.shared.photo(wikipedia: wikipedia) }
        .accessibilityHidden(true)
    }

    private var initials: String {
        let parts = name.split(separator: " ")
        guard let first = parts.first?.first else { return "?" }
        if parts.count > 1, let last = parts.last?.first { return "\(first)\(last)".uppercased() }
        return String(parts[0].prefix(2)).uppercased()
    }
}

/// Grande photo créditée (fiche pilote, circuit), masquée s'il n'y a pas d'image libre.
struct WikiPhotoView: View {
    let wikipedia: String?
    var wide = false

    @State private var photo: Photo?

    var body: some View {
        Group {
            if let photo, let src = URL(string: photo.src) {
                VStack(alignment: .leading, spacing: 4) {
                    AsyncImage(url: src) { phase in
                        if let image = phase.image {
                            image.resizable().aspectRatio(contentMode: wide ? .fit : .fill)
                        } else {
                            Color(uiColor: .tertiarySystemBackground)
                        }
                    }
                    .frame(maxWidth: .infinity)
                    .frame(height: wide ? 190 : 280)
                    .background(wide ? Color.white : Color.clear)
                    .clipShape(RoundedRectangle(cornerRadius: 14))
                    if let credit = URL(string: photo.credit) {
                        Link(L("Photo : Wikimedia Commons (auteur et licence) ↗", "Photo: Wikimedia Commons (author and licence) ↗"), destination: credit)
                            .font(.caption2)
                            .foregroundStyle(.secondary)
                    }
                }
            }
        }
        .task(id: wikipedia) { photo = await ServerAPI.shared.photo(wikipedia: wikipedia) }
    }
}

/// Rampe de vitesse (lent = rouge soutenu, rapide = clair), identique au site.
func speedColor(_ ratio: Double) -> Color {
    let r = max(0, min(1, ratio))
    return Color(red: (179 + 76 * r) / 255, green: (38 + 190 * r) / 255, blue: (30 + 192 * r) / 255)
}

/// Tracé 2D coloré par la vitesse ; toucher le tracé affiche la télémétrie du point.
struct TrackMapView: View {
    let map: TrackMap
    var markers: [TrackMarker] = []
    var showTelemetry = true

    @State private var picked: Int?

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            if showTelemetry {
                Text(readout).font(.subheadline.bold().monospacedDigit()).frame(minHeight: 20, alignment: .leading)
            }
            GeometryReader { geo in
                let pad = 40.0
                let w = map.width + 2 * pad, h = map.height + 2 * pad
                let gw = Double(geo.size.width), gh = Double(geo.size.height)
                let scale: Double = min(gw / w, gh / h)
                let ox: Double = (gw - w * scale) / 2, oy: Double = (gh - h * scale) / 2
                let pt = { (p: TrackPoint) in CGPoint(x: ox + (p.x + pad) * scale, y: oy + (p.y + pad) * scale) }
                let minS = Double(map.stats.min_speed), maxS = Double(map.stats.top_speed)
                ZStack {
                    Canvas { ctx, _ in
                        var base = Path()
                        for (i, p) in map.points.enumerated() {
                            if i == 0 { base.move(to: pt(p)) } else { base.addLine(to: pt(p)) }
                        }
                        ctx.stroke(base, with: .color(Color(white: 0.2)), style: StrokeStyle(lineWidth: 10, lineCap: .round, lineJoin: .round))
                        for i in 0..<(map.points.count - 1) {
                            var seg = Path()
                            seg.move(to: pt(map.points[i]))
                            seg.addLine(to: pt(map.points[i + 1]))
                            let ratio = (Double(map.points[i].speed) - minS) / max(maxS - minS, 1)
                            ctx.stroke(seg, with: .color(speedColor(ratio)), style: StrokeStyle(lineWidth: 5, lineCap: .round))
                        }
                        if let first = map.points.first {
                            let p = pt(first)
                            ctx.fill(Path(ellipseIn: CGRect(x: p.x - 6, y: p.y - 6, width: 12, height: 12)), with: .color(.white))
                        }
                        if let i = picked {
                            let p = pt(map.points[i])
                            ctx.fill(Path(ellipseIn: CGRect(x: p.x - 8, y: p.y - 8, width: 16, height: 16)), with: .color(.white))
                        }
                    }
                    ForEach(markers, id: \.key) { m in
                        let target = m.fraction * map.lap_time
                        let idx = map.points.firstIndex { $0.t >= target } ?? 0
                        let p = pt(map.points[idx])
                        Text(m.label)
                            .font(.system(size: 10, weight: .heavy))
                            .foregroundStyle(.black)
                            .frame(width: 22, height: 22)
                            .background(Circle().fill(Color(hexString: m.colour)))
                            .position(p)
                            .animation(.linear(duration: 1), value: idx)
                    }
                }
                .contentShape(Rectangle())
                .gesture(DragGesture(minimumDistance: 0).onChanged { g in
                    guard showTelemetry else { return }
                    let x: Double = (Double(g.location.x) - ox) / scale - pad
                    let y: Double = (Double(g.location.y) - oy) / scale - pad
                    picked = map.points.indices.min { a, b in
                        let pa = map.points[a], pb = map.points[b]
                        return pow(pa.x - x, 2) + pow(pa.y - y, 2) < pow(pb.x - x, 2) + pow(pb.y - y, 2)
                    }
                })
            }
            .aspectRatio((map.width + 80) / (map.height + 80), contentMode: .fit)
            if showTelemetry {
                HStack(spacing: 6) {
                    Text("\(map.stats.min_speed) km/h").font(.caption2)
                    LinearGradient(colors: [speedColor(0), speedColor(1)], startPoint: .leading, endPoint: .trailing)
                        .frame(height: 6).clipShape(Capsule())
                    Text("\(map.stats.top_speed) km/h").font(.caption2)
                }
                .foregroundStyle(.secondary)
            }
        }
    }

    private var readout: String {
        guard let i = picked else { return L("Touche le tracé pour lire la télémétrie.", "Touch the track to read the telemetry.") }
        let p = map.points[i]
        return L("\(p.speed) km/h · rapport \(p.gear) · gaz \(p.throttle) %\(p.brake ? " · freinage" : "") · \(String(format: "%.1f", p.t)) s",
                 "\(p.speed) km/h · gear \(p.gear) · throttle \(p.throttle)%\(p.brake ? " · braking" : "") · \(String(format: "%.1f", p.t)) s")
    }
}

/// Bloc « Tracé » : plan 2D (télémétrie) ou relief 3D.
struct TrackPanel: View {
    let circuitId: String
    var start3D = false

    @State private var map: TrackMap?
    @State private var failed = false
    @State private var unavailable = false
    @State private var attempt = 0
    @State private var three = false

    var body: some View {
        Group {
            if let map {
                VStack(alignment: .leading, spacing: 10) {
                    Picker("", selection: $three) {
                        Text(L("Plan 2D", "2D map")).tag(false)
                        Text(L("Relief 3D", "3D relief")).tag(true)
                    }
                    .pickerStyle(.segmented)
                    if three {
                        Track3DView(map: map)
                    } else {
                        TrackMapView(map: map)
                    }
                    StatGrid(items: [
                        (L("Tour", "Lap"), formatLap(map.lap_time)),
                        (L("Longueur", "Length"), String(format: "%.2f km", map.stats.length_km)),
                        (L("V. max", "Top speed"), "\(map.stats.top_speed) km/h"),
                        (L("V. mini", "Min speed"), "\(map.stats.min_speed) km/h"),
                        (L("À fond", "Full throttle"), String(format: "%.0f%%", map.stats.full_throttle_pct)),
                        (L("Freinage", "Braking"), String(format: "%.0f%%", map.stats.braking_pct)),
                    ])
                    Text(L("Meilleur tour de \(map.driver) (\(map.team)) au \(map.event) \(map.year) — positions GPS et télémétrie OpenF1.",
                           "Fastest lap by \(map.driver) (\(map.team)) at the \(map.event) \(map.year) — OpenF1 GPS and telemetry."))
                        .font(.caption).foregroundStyle(.secondary)
                }
            } else if failed {
                Text(L("Tracé disponible pour les circuits utilisés depuis 2023 (données GPS OpenF1).", "Layout available for circuits used since 2023 (OpenF1 GPS data)."))
                    .font(.footnote).foregroundStyle(.secondary)
            } else if unavailable {
                VStack(alignment: .leading, spacing: 8) {
                    Text(L("Tracé momentanément indisponible (source OpenF1 saturée ou séance en direct).", "Layout temporarily unavailable (OpenF1 busy or a live session is running)."))
                        .font(.footnote).foregroundStyle(.secondary)
                    Button(L("Réessayer", "Retry")) { attempt += 1 }
                        .buttonStyle(.bordered)
                }
            } else {
                ProgressView().frame(maxWidth: .infinity, minHeight: 120)
            }
        }
        .task(id: "\(circuitId)-\(attempt)") {
            three = start3D
            unavailable = false
            do {
                map = try await ServerAPI.shared.track(circuitId)
            } catch let error as URLError where error.code == .fileDoesNotExist {
                failed = true
            } catch {
                unavailable = true
            }
        }
    }
}
