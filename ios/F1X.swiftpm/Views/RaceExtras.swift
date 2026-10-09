import Charts
import SwiftUI

// Sections complètes d'une page de Grand Prix (comme sur le site) : arrêts par pilote,
// meilleurs tours, analyse tour par tour (positions, tours en tête), bilan de course.

/// « 2.456 » → « 2,456 s », « 1:02.3 » inchangé.
private func stopDuration(_ d: String?) -> String {
    guard let d else { return "–" }
    return d.contains(":") ? d : "\(d.replacingOccurrences(of: ".", with: isFrench ? "," : ".")) s"
}

/// Tous les passages aux stands, groupés par pilote dans l'ordre d'arrivée.
struct PitStopsSection: View {
    let pits: [PitStop]
    let results: [RaceResult]

    var body: some View {
        let byDriver = Dictionary(grouping: pits, by: \.driverId)
        let fastest = pits.compactMap { p in p.duration.flatMap(Double.init).map { (p, $0) } }.min { $0.1 < $1.1 }
        let order = results.map(\.driver.driverId) + byDriver.keys.filter { id in !results.contains { $0.driver.driverId == id } }.sorted()
        FoldSection(L("Arrêts aux stands (\(pits.count))", "Pit stops (\(pits.count))")) {
            if let f = fastest {
                Label(L("Le plus rapide : \(name(f.0.driverId)) — \(stopDuration(f.0.duration)) (tour \(f.0.lap))",
                        "Fastest: \(name(f.0.driverId)) — \(stopDuration(f.0.duration)) (lap \(f.0.lap))"),
                      systemImage: "stopwatch")
                    .font(.subheadline.bold())
                    .foregroundStyle(Color.f1Purple)
            }
            ForEach(order.filter { byDriver[$0] != nil }, id: \.self) { id in
                let stops = (byDriver[id] ?? []).sorted { (Int($0.stop) ?? 0) < (Int($1.stop) ?? 0) }
                let result = results.first { $0.driver.driverId == id }
                let total = stops.compactMap { $0.duration.flatMap(Double.init) }.reduce(0, +)
                HStack(alignment: .top, spacing: 10) {
                    Text(result?.positionText ?? "–")
                        .font(.body.monospacedDigit().weight(.bold))
                        .frame(width: 28, alignment: .leading)
                    Rectangle().fill(Team.color(result?.constructor.constructorId)).frame(width: 3)
                    VStack(alignment: .leading, spacing: 3) {
                        Text(name(id)).font(.body.weight(.semibold))
                        ForEach(stops) { s in
                            Text(L("Arrêt \(s.stop) · tour \(s.lap) · \(stopDuration(s.duration))",
                                   "Stop \(s.stop) · lap \(s.lap) · \(stopDuration(s.duration))"))
                                .font(.caption.monospacedDigit())
                                .foregroundStyle(s.id == fastest?.0.id ? Color.f1Purple : .secondary)
                        }
                    }
                    Spacer(minLength: 4)
                    VStack(alignment: .trailing, spacing: 2) {
                        Text("\(stops.count)").font(.title3.weight(.heavy))
                        Text(stops.count > 1 ? L("arrêts", "stops") : L("arrêt", "stop")).font(.caption2).foregroundStyle(.secondary)
                        if total > 0 {
                            Text(String(format: "%.1f s", total)).font(.caption2.monospacedDigit()).foregroundStyle(.secondary)
                        }
                    }
                }
            }
            Text(L("Durée = temps passé dans la voie des stands (entrée → sortie), source Jolpica.",
                   "Duration = time spent in the pit lane (entry → exit), Jolpica data."))
                .font(.caption2).foregroundStyle(.secondary)
        }
    }

    private func name(_ id: String) -> String {
        results.first { $0.driver.driverId == id }.map { "\($0.driver.givenName) \($0.driver.familyName)" } ?? id.replacingOccurrences(of: "_", with: " ").capitalized
    }
}

/// Meilleurs tours de la course (classement, tour, vitesse moyenne).
struct FastestLapsSection: View {
    let results: [RaceResult]

    var body: some View {
        let laps = results.filter { $0.fastestLap?.time != nil }
            .sorted { (Int($0.fastestLap?.rank ?? "") ?? 99) < (Int($1.fastestLap?.rank ?? "") ?? 99) }
        if !laps.isEmpty {
            FoldSection(L("Meilleurs tours", "Fastest laps")) {
                ForEach(Array(laps.prefix(10).enumerated()), id: \.offset) { _, r in
                    let f = r.fastestLap!
                    let detail = [f.lap.map { L("tour \($0)", "lap \($0)") },
                                  f.averageSpeed.map { "\($0.speed) km/h" }].compactMap { $0 }.joined(separator: " · ")
                    NavigationLink(value: r.driver) {
                        StandingRow(position: f.rank ?? "–", teamId: r.constructor.constructorId,
                                    title: driverTitle(r.driver), subtitle: [r.constructor.name, detail].joined(separator: " · ")) {
                            PointsLabel(value: f.time?.time ?? "", suffix: "")
                        }
                    }
                }
            }
        }
    }
}

/// Bilan : arrivés, abandons et autres statuts.
struct RaceSummarySection: View {
    let results: [RaceResult]

    var body: some View {
        let groups = Dictionary(grouping: results) { r -> String in
            let s = r.status ?? ""
            if s == "Finished" || s.hasPrefix("+") || s == "Lapped" { return L("Classés à l'arrivée", "Classified finishers") }
            return r.outcome.isEmpty ? s : r.outcome
        }
        .map { ($0.key, $0.value) }
        .sorted { $0.1.count > $1.1.count }
        if !groups.isEmpty {
            FoldSection(L("Bilan de la course", "Race summary")) {
                ForEach(groups, id: \.0) { g in
                    VStack(alignment: .leading, spacing: 2) {
                        HStack {
                            Text(g.0).font(.body.weight(.semibold))
                            Spacer()
                            Text("\(g.1.count)").font(.body.monospacedDigit().weight(.bold))
                        }
                        if g.1.count <= 6 {
                            Text(g.1.map(\.driver.familyName).joined(separator: ", "))
                                .font(.caption).foregroundStyle(.secondary)
                        }
                    }
                }
            }
        }
    }
}

/// Analyse tour par tour (Jolpica `laps`) : positions des 10 premiers et tours en tête.
struct LapByLapSection: View {
    let season: String
    let round: Int
    let results: [RaceResult]
    let pits: [PitStop]

    @State private var laps: [LapData] = []
    @State private var state = 0 // 0 inactif, 1 chargement, 2 prêt, 3 erreur

    private struct Pt: Identifiable {
        let id = UUID()
        let driver: String
        let lap: Int
        let pos: Int
    }

    var body: some View {
        FoldSection(L("Tour par tour", "Lap by lap")) {
            switch state {
            case 0:
                Text(L("Position de chaque pilote à chaque tour, tours en tête et passages aux stands.",
                       "Each driver's position on every lap, laps led and pit stops."))
                    .font(.footnote).foregroundStyle(.secondary)
                Button(L("Charger l'analyse", "Load the analysis")) { Task { await load() } }
            case 1:
                StartLightsLoader()
            case 3:
                Text(L("Données tour par tour indisponibles.", "Lap data unavailable.")).foregroundStyle(.secondary)
                Button(L("Réessayer", "Retry")) { Task { await load() } }
            default:
                content
            }
        }
    }

    @ViewBuilder
    private var content: some View {
        let top = results.prefix(10).map(\.driver)
        let codes = Dictionary(top.map { ($0.driverId, $0.code ?? String($0.familyName.prefix(3)).uppercased()) }, uniquingKeysWith: { a, _ in a })
        // Légende sans doublon (deux pilotes au même code feraient planter Charts).
        let legend: [(String, Color)] = {
            var seen = Set<String>()
            return zip(top, results.prefix(10)).compactMap { d, r in
                let c = codes[d.driverId] ?? ""
                return seen.insert(c).inserted ? (c, Team.color(r.constructor.constructorId)) : nil
            }
        }()
        let pts: [Pt] = laps.flatMap { lap in
            lap.timings.compactMap { t in
                guard let c = codes[t.driverId], let p = Int(t.position) else { return nil }
                return Pt(driver: c, lap: Int(lap.number) ?? 0, pos: p)
            }
        }
        Text(L("Positions des 10 premiers à l'arrivée", "Positions of the top 10 finishers"))
            .font(.caption.weight(.semibold)).foregroundStyle(.secondary)
        Chart(pts) {
            LineMark(x: .value(L("Tour", "Lap"), $0.lap), y: .value("Position", $0.pos))
                .interpolationMethod(.stepEnd)
                .foregroundStyle(by: .value(L("Pilote", "Driver"), $0.driver))
        }
        .chartYScale(domain: .automatic(includesZero: false, reversed: true))
        .chartForegroundStyleScale(domain: legend.map(\.0), range: legend.map(\.1))
        .frame(height: 260)
        // Tours en tête.
        let leaders = laps.compactMap { $0.timings.first { $0.position == "1" }?.driverId }
        let led = Dictionary(grouping: leaders, by: { $0 }).mapValues(\.count).sorted { $0.value > $1.value }
        Text(L("Tours en tête (\(laps.count) tours)", "Laps led (\(laps.count) laps)"))
            .font(.caption.weight(.semibold)).foregroundStyle(.secondary)
        ForEach(led, id: \.key) { e in
            let r = results.first { $0.driver.driverId == e.key }
            HStack {
                Rectangle().fill(Team.color(r?.constructor.constructorId)).frame(width: 3, height: 20)
                Text(r.map { "\($0.driver.givenName) \($0.driver.familyName)" } ?? e.key)
                Spacer()
                Text("\(e.value)").font(.body.monospacedDigit().weight(.bold))
                GeometryReader { g in
                    Capsule().fill(Team.color(r?.constructor.constructorId))
                        .frame(width: g.size.width * CGFloat(e.value) / CGFloat(max(laps.count, 1)))
                }
                .frame(width: 80, height: 6)
            }
        }
        Text(L("Temps au tour enregistrés : \(laps.reduce(0) { $0 + $1.timings.count })", "Lap times recorded: \(laps.reduce(0) { $0 + $1.timings.count })"))
            .font(.caption2).foregroundStyle(.secondary)
    }

    private func load() async {
        state = 1
        do {
            laps = try await F1API.shared.laps(season: season, round: round)
            state = laps.isEmpty ? 3 : 2
        } catch {
            state = 3
        }
    }
}

/// Tracé du circuit (contour), comme sur la page d'accueil du site : il se dessine,
/// puis un point rouge en fait le tour.
struct TrackOutline: View {
    let circuitId: String
    var height: CGFloat = 190

    @State private var map: TrackMap?
    @State private var drawn: CGFloat = 0
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    var body: some View {
        VStack {
            if let map, map.points.count > 2 {
                GeometryReader { geo in
                    let pad = 30.0
                    let w = map.width + 2 * pad, h = map.height + 2 * pad
                    let gw = Double(geo.size.width), gh = Double(geo.size.height)
                    let scale = min(gw / w, gh / h)
                    let ox = (gw - w * scale) / 2, oy = (gh - h * scale) / 2
                    let pts = map.points.map { CGPoint(x: ox + ($0.x + pad) * scale, y: oy + ($0.y + pad) * scale) }
                    let path = Path { p in
                        p.addLines(pts)
                        p.closeSubpath()
                    }
                    ZStack(alignment: .topLeading) {
                        path.stroke(Color.ink.opacity(0.13), style: StrokeStyle(lineWidth: 12, lineCap: .round, lineJoin: .round))
                        TracedPath(path: path, progress: drawn)
                            .stroke(Color.ink, style: StrokeStyle(lineWidth: 4.5, lineCap: .round, lineJoin: .round))
                        // Ligne de départ.
                        Circle().fill(Color.cardBackground).frame(width: 12, height: 12)
                            .overlay(Circle().fill(Color.f1Red).frame(width: 9, height: 9))
                            .position(pts[0])
                        // Une voiture (point rouge lumineux) boucle le tour toutes les 9 s.
                        if drawn >= 1 && !reduceMotion {
                            TimelineView(.animation(minimumInterval: 1 / 30)) { context in
                                let t = context.date.timeIntervalSinceReferenceDate.truncatingRemainder(dividingBy: 9) / 9
                                let i = min(pts.count - 1, Int(t * Double(pts.count)))
                                Circle().fill(Color.f1Red).frame(width: 9, height: 9)
                                    .shadow(color: .f1Red, radius: 6)
                                    .position(pts[i])
                            }
                        }
                    }
                }
                .frame(height: height)
                .accessibilityLabel(L("Tracé du circuit", "Circuit layout"))
            }
        }
        .task(id: circuitId) {
            map = try? await ServerAPI.shared.track(circuitId)
            if reduceMotion { drawn = 1; return }
            drawn = 0
            withAnimation(.easeInOut(duration: 1.8)) { drawn = 1 }
        }
    }
}

/// Chemin tracé progressivement (0 → 1), animable.
private struct TracedPath: Shape {
    let path: Path
    var progress: CGFloat
    var animatableData: CGFloat {
        get { progress }
        set { progress = newValue }
    }
    func path(in rect: CGRect) -> Path { path.trimmedPath(from: 0, to: progress) }
}

/// Détail de chaque arrêt (OpenF1, courses depuis 2023) : voie des stands, immobilisation,
/// pneus retirés et montés, position avant et après.
struct PitDetailSection: View {
    /// Requête du serveur : `session_key=…` ou `year=…&date=…`.
    let query: String

    private struct Tyre: Decodable {
        let compound: String
        let age_at_start: Int?
        let age_end: Int?
    }

    private struct Stop: Decodable, Identifiable {
        let code: String
        let name: String
        let team: String
        let colour: String
        let lap: Int
        let date: String
        let lane_duration: Double?
        let stop_duration: Double?
        let tyre_before: Tyre?
        let tyre_after: Tyre?
        let position_before: Int?
        let position_after: Int?
        var id: String { "\(code)-\(lap)-\(date)" }
    }

    private struct Response: Decodable {
        let stops: [Stop]
    }

    @State private var stops: [Stop] = []
    @State private var state = 0 // 0 chargement, 1 prêt, 2 rien

    var body: some View {
        // Une seule Section, et le chargement attaché à une vraie ligne (un Group dans une
        // List perd ses modificateurs).
        FoldSection(state == 1 ? L("Détail de chaque arrêt (\(stops.count))", "Every pit stop in detail (\(stops.count))")
                           : L("Détail de chaque arrêt", "Every pit stop in detail")) {
            switch state {
            case 0:
                StartLightsLoader()
                    .task(id: query) { await load() }
            case 1:
                let fastest = stops.compactMap(\.stop_duration).min()
                ForEach(stops) { s in row(s, fastest: fastest) }
                Text(L("Voie des stands : de l'entrée à la sortie. Immobilisé : voiture à l'arrêt pendant le changement de pneus. Positions juste avant l'entrée et juste après la ressortie. Données OpenF1.",
                       "Pit lane: entry to exit. Stationary: car stopped for the tyre change. Positions just before entry and just after exit. OpenF1 data."))
                    .font(.caption2).foregroundStyle(.secondary)
            default:
                Text(L("Pas de détail OpenF1 pour ce Grand Prix.", "No OpenF1 detail for this Grand Prix."))
                    .font(.footnote).foregroundStyle(.secondary)
            }
        }
    }

    private func load() async {
        if let r = try? await ServerAPI.shared.get("api/of1/pitdetail?\(query)", as: Response.self, ttl: 600), !r.stops.isEmpty {
            stops = r.stops
            state = 1
        } else {
            state = 2
        }
    }

    // Mise en page verticale : chaque ligne occupe toute la largeur et passe à la ligne
    // normalement (lisible aussi avec les grandes tailles de texte).
    private func row(_ s: Stop, fastest: Double?) -> some View {
        HStack(alignment: .top, spacing: 10) {
            Rectangle().fill(Color(hexString: s.colour)).frame(width: 4)
            VStack(alignment: .leading, spacing: 4) {
                HStack(alignment: .firstTextBaseline, spacing: 8) {
                    Text(L("Tour \(s.lap)", "Lap \(s.lap)"))
                        .font(.caption.weight(.heavy))
                        .padding(.horizontal, 6).padding(.vertical, 2)
                        .background(Color.chip, in: Capsule())
                        .fixedSize()
                    Text(s.name.capitalized).font(.body.weight(.semibold))
                }
                Text(s.team).font(.caption).foregroundStyle(.secondary)
                tyres(s)
                let lane = s.lane_duration.map { L("Voie des stands \(String(format: "%.1f", $0)) s", "Pit lane \(String(format: "%.1f", $0)) s") }
                let stop = s.stop_duration.map { L("immobilisé \(String(format: "%.1f", $0)) s", "stationary \(String(format: "%.1f", $0)) s") }
                let best = s.stop_duration != nil && s.stop_duration == fastest
                Text([lane, stop, best ? L("le plus rapide", "fastest") : nil].compactMap { $0 }.joined(separator: " · "))
                    .font(.footnote.monospacedDigit())
                    .foregroundStyle(best ? Color.f1Purple : .secondary)
                if let b = s.position_before, let a = s.position_after {
                    Text(a == b ? L("Position conservée : P\(b)", "Kept position: P\(b)")
                                : L("P\(b) → P\(a) (\(a > b ? "−" : "+")\(abs(a - b)) place\(abs(a - b) > 1 ? "s" : ""))",
                                    "P\(b) → P\(a) (\(a > b ? "−" : "+")\(abs(a - b)))"))
                        .font(.footnote.monospacedDigit().weight(.bold))
                        .foregroundStyle(a > b ? Color.red : (a < b ? Color.green : Color.secondary))
                }
            }
            .frame(maxWidth: .infinity, alignment: .leading)
        }
        .padding(.vertical, 2)
    }

    private func tyreName(_ c: String) -> String {
        switch c {
        case "SOFT": return L("Tendres", "Soft")
        case "MEDIUM": return "Medium"
        case "HARD": return L("Durs", "Hard")
        case "INTERMEDIATE": return L("Intermédiaires", "Inters")
        case "WET": return L("Pluie", "Wet")
        default: return "?"
        }
    }

    /// « ● Medium (20 tours) → ● Tendres usagés (4 tours) », en un seul texte qui passe à la ligne.
    private func tyres(_ s: Stop) -> some View {
        let dot = { (c: String?) -> Text in Text("● ").foregroundColor(tyreColor(c)) }
        let before: Text = s.tyre_before.map { (t: Tyre) -> Text in
            dot(t.compound) + Text(L("\(tyreName(t.compound)) (\(t.age_end ?? 0) tours)", "\(tyreName(t.compound)) (\(t.age_end ?? 0) laps)"))
        } ?? Text("?")
        let after: Text = s.tyre_after.map { (t: Tyre) -> Text in
            let age = t.age_at_start ?? 0
            let state = age == 0 ? L("neufs", "new") : L("usagés, \(age) tours", "used, \(age) laps")
            return dot(t.compound) + Text("\(tyreName(t.compound)) \(state)")
        } ?? Text("?")
        return (before + Text("  →  ").foregroundColor(.secondary) + after)
            .font(.subheadline)
            .fixedSize(horizontal: false, vertical: true)
    }
}
