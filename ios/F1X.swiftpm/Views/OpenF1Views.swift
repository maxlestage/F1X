import AVFoundation
import Charts
import SwiftUI

// Données OpenF1 complètes (les 18 points d'accès, via le relais mis en cache du serveur F1X).

/// Valeur JSON générique (les réponses OpenF1 sont des tableaux d'objets plats).
enum JSONValue: Decodable, Sendable, Hashable {
    case null
    case bool(Bool)
    case num(Double)
    case str(String)
    case arr([JSONValue])
    case obj([String: JSONValue])

    init(from decoder: Decoder) throws {
        let c = try decoder.singleValueContainer()
        if c.decodeNil() {
            self = .null
        } else if let b = try? c.decode(Bool.self) {
            self = .bool(b)
        } else if let d = try? c.decode(Double.self) {
            self = .num(d)
        } else if let s = try? c.decode(String.self) {
            self = .str(s)
        } else if let a = try? c.decode([JSONValue].self) {
            self = .arr(a)
        } else {
            self = .obj(try c.decode([String: JSONValue].self))
        }
    }

    subscript(_ key: String) -> JSONValue {
        if case .obj(let o) = self { return o[key] ?? .null }
        return .null
    }

    var string: String {
        switch self {
        case .str(let s): return s
        case .num(let d): return d == d.rounded() && abs(d) < 9e15 ? String(Int(d)) : String(d)
        default: return ""
        }
    }

    var double: Double? {
        if case .num(let d) = self { return d }
        return nil
    }

    var int: Int? { double.flatMap { $0.isFinite && abs($0) < 9e15 ? Int($0) : nil } }

    var bool: Bool {
        if case .bool(let b) = self { return b }
        return false
    }

    var array: [JSONValue] {
        if case .arr(let a) = self { return a }
        return []
    }

    var doubles: [Double] { array.compactMap(\.double) }
}

/// Appel au relais OpenF1 du serveur (`/api/of1/{endpoint}?{query}`).
func of1(_ endpoint: String, _ query: String, ttl: TimeInterval = 600) async -> [JSONValue] {
    (try? await ServerAPI.shared.get("api/of1/\(endpoint)?\(query)", as: [JSONValue].self, ttl: ttl)) ?? []
}

/// Date ISO OpenF1 → Date.
func of1Date(_ s: String) -> Date? {
    let f = ISO8601DateFormatter()
    f.formatOptions = [.withInternetDateTime, .withFractionalSeconds]
    if let d = f.date(from: s) { return d }
    f.formatOptions = [.withInternetDateTime]
    return f.date(from: s)
}

private let palette: [Color] = [
    Color(red: 0x39 / 255, green: 0x87 / 255, blue: 0xE5 / 255),
    Color(red: 0xD9 / 255, green: 0x59 / 255, blue: 0x26 / 255),
    Color(red: 0x19 / 255, green: 0x9E / 255, blue: 0x70 / 255),
    Color(red: 0xC9 / 255, green: 0x85 / 255, blue: 0x00 / 255),
    Color(red: 0xD5 / 255, green: 0x51 / 255, blue: 0x81 / 255),
]

/// Pilote d'une séance : acronyme, nom complet, couleur d'écurie, écurie.
struct OF1Driver: Hashable, Sendable {
    let number: Int
    let code: String
    let name: String
    let colour: String
    let team: String
}

private func driverMap(_ list: [JSONValue]) -> [Int: OF1Driver] {
    var out: [Int: OF1Driver] = [:]
    for d in list {
        guard let n = d["driver_number"].int else { continue }
        out[n] = OF1Driver(number: n, code: d["name_acronym"].string, name: d["full_name"].string,
                           colour: d["team_colour"].string, team: d["team_name"].string)
    }
    return out
}

// MARK: - Années et réunions

struct DataYearView: View {
    @State var year: Int
    @State private var meetings: [JSONValue] = []
    @State private var loading = true

    private var years: [Int] {
        let now = Calendar.current.component(.year, from: Date())
        return Array((2023...max(2023, now)).reversed())
    }

    var body: some View {
        List {
            Section {
                Text(L("Chaque séance depuis 2023 en détail : résultats, grille, championnat, tours, positions, écarts, pneus, arrêts, dépassements, télémétrie, météo, direction de course et radios.",
                       "Every session since 2023 in detail: results, grid, championship, laps, positions, gaps, tyres, stops, overtakes, telemetry, weather, race control and radio."))
                    .font(.footnote).foregroundStyle(.secondary)
                Picker(L("Saison", "Season"), selection: $year) {
                    ForEach(years, id: \.self) { Text(String($0)).tag($0) }
                }
            }
            Section(L("Grands Prix \(year)", "\(year) Grands Prix")) {
                if loading {
                    ProgressView()
                } else if meetings.isEmpty {
                    Text(L("Aucune donnée.", "No data.")).foregroundStyle(.secondary)
                }
                ForEach(meetings.reversed(), id: \.self) { m in
                    NavigationLink {
                        DataMeetingView(key: m["meeting_key"].int ?? 0, title: m["meeting_name"].string)
                    } label: {
                        VStack(alignment: .leading, spacing: 2) {
                            Text(m["meeting_name"].string).font(.body.weight(.semibold))
                            Text("\(m["location"].string) · \(m["country_name"].string) · \(shortDate(m["date_start"].string))")
                                .font(.caption).foregroundStyle(.secondary)
                        }
                    }
                }
            }
        }
        .navigationTitle(L("Données OpenF1", "OpenF1 data"))
        .task(id: year) {
            loading = true
            meetings = await of1("meetings", "year=\(year)", ttl: 3600)
            loading = false
        }
    }
}

private func shortDate(_ iso: String, time: Bool = false) -> String {
    guard let d = of1Date(iso) else { return "" }
    return time ? d.formatted(date: .abbreviated, time: .shortened) : d.formatted(date: .abbreviated, time: .omitted)
}

struct DataMeetingView: View {
    let key: Int
    let title: String
    @State private var sessions: [JSONValue] = []
    @State private var grid: [JSONValue] = []
    @State private var drivers: [Int: OF1Driver] = [:]
    @State private var meeting: JSONValue = .null

    var body: some View {
        List {
            if meeting != .null {
                Section {
                    LabeledContent(L("Circuit", "Circuit"), value: meeting["circuit_short_name"].string)
                    LabeledContent(L("Lieu", "Location"), value: "\(meeting["location"].string), \(meeting["country_name"].string)")
                    LabeledContent(L("Dates", "Dates"), value: shortDate(meeting["date_start"].string))
                }
            }
            Section(L("Séances", "Sessions")) {
                if sessions.isEmpty { ProgressView() }
                ForEach(sessions, id: \.self) { s in
                    NavigationLink {
                        DataSessionView(key: s["session_key"].int ?? 0, title: "\(s["location"].string) — \(s["session_name"].string)", type: s["session_type"].string)
                    } label: {
                        VStack(alignment: .leading, spacing: 2) {
                            Text(s["session_name"].string).font(.body.weight(.semibold))
                            Text(shortDate(s["date_start"].string, time: true)).font(.caption).foregroundStyle(.secondary)
                        }
                    }
                }
            }
            if !grid.isEmpty {
                Section(L("Grille de départ", "Starting grid")) {
                    ForEach(grid, id: \.self) { g in
                        let d = drivers[g["driver_number"].int ?? 0]
                        HStack {
                            Text(g["position"].string).font(.body.monospacedDigit().weight(.bold)).frame(width: 28, alignment: .leading)
                            Rectangle().fill(Color(hexString: d?.colour ?? "888888")).frame(width: 3, height: 22)
                            Text(d?.name ?? "#\(g["driver_number"].string)")
                            Spacer()
                            if let t = g["lap_duration"].double { Text(formatLap(t)).font(.caption.monospacedDigit()).foregroundStyle(.secondary) }
                        }
                    }
                }
            }
        }
        .navigationTitle(title)
        .navigationBarTitleDisplayMode(.inline)
        .task {
            async let m = of1("meetings", "meeting_key=\(key)", ttl: 3600)
            async let s = of1("sessions", "meeting_key=\(key)", ttl: 3600)
            async let g = of1("starting_grid", "meeting_key=\(key)", ttl: 3600)
            async let d = of1("drivers", "meeting_key=\(key)", ttl: 3600)
            meeting = await m.first ?? .null
            sessions = await s
            grid = await g.sorted { ($0["position"].int ?? 99) < ($1["position"].int ?? 99) }
            drivers = driverMap(await d)
        }
    }
}

// MARK: - Séance

enum DataTab: String, CaseIterable, Identifiable {
    case results, driver, laps, positions, tyres, telemetry, race, radio, weather
    var id: String { rawValue }
    var label: String {
        switch self {
        case .results: return L("Résultats", "Results")
        case .driver: return L("Fiche pilote", "Driver file")
        case .laps: return L("Tours", "Laps")
        case .positions: return "Positions"
        case .tyres: return L("Pneus", "Tyres")
        case .telemetry: return L("Télémétrie", "Telemetry")
        case .race: return L("Course", "Race")
        case .radio: return L("Radios", "Radio")
        case .weather: return L("Météo", "Weather")
        }
    }
    var symbol: String {
        switch self {
        case .results: return "list.number"
        case .driver: return "person.text.rectangle"
        case .laps: return "stopwatch"
        case .positions: return "arrow.up.arrow.down"
        case .tyres: return "circle.circle"
        case .telemetry: return "gauge.with.dots.needle.67percent"
        case .race: return "flag.checkered"
        case .radio: return "headphones"
        case .weather: return "cloud.sun"
        }
    }
}

struct DataSessionView: View {
    let key: Int
    let title: String
    let type: String
    @State private var tab: DataTab

    init(key: Int, title: String, type: String, start: DataTab = .results) {
        self.key = key
        self.title = title
        self.type = type
        _tab = State(initialValue: start)
    }
    @State private var drivers: [Int: OF1Driver] = [:]

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 16) {
                Picker(L("Section", "Section"), selection: $tab) {
                    ForEach(DataTab.allCases) { Text($0.label).tag($0) }
                }
                .pickerStyle(.menu)
                .frame(maxWidth: .infinity, alignment: .leading)
                Group {
                    switch tab {
                    case .results: ResultsSection(key: key, drivers: drivers)
                    case .driver: DriverRaceSection(key: key, drivers: drivers)
                    case .laps: LapsSection(key: key, drivers: drivers)
                    case .positions: PositionsSection(key: key, drivers: drivers)
                    case .tyres: TyresSection(key: key, drivers: drivers)
                    case .telemetry: TelemetrySection(key: key, drivers: drivers)
                    case .race: RaceSection(key: key, drivers: drivers)
                    case .radio: RadioSection(key: key, drivers: drivers)
                    case .weather: SessionWeatherSection(key: key)
                    }
                }
            }
            .padding()
        }
        .navigationTitle(title)
        .navigationBarTitleDisplayMode(.inline)
        .task { drivers = driverMap(await of1("drivers", "session_key=\(key)", ttl: 3600)) }
    }
}

/// Carte de section, repliable d'un toucher sur le titre (état retenu).
private struct Card<Content: View>: View {
    let title: String
    @ViewBuilder var content: Content
    @AppStorage("foldedCards") private var folded = ""

    private var key: String {
        title.filter { !$0.isNumber && $0 != "(" && $0 != ")" }.trimmingCharacters(in: .whitespaces)
    }
    private var isFolded: Bool { folded.components(separatedBy: "|").contains(key) }

    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            Button {
                var set = Set(folded.components(separatedBy: "|").filter { !$0.isEmpty })
                if set.contains(key) { set.remove(key) } else { set.insert(key) }
                withAnimation(.easeInOut(duration: 0.2)) { folded = set.sorted().joined(separator: "|") }
            } label: {
                HStack {
                    Text(title).font(.headline)
                    Spacer()
                    Image(systemName: "chevron.down")
                        .font(.caption.bold())
                        .foregroundStyle(.secondary)
                        .rotationEffect(.degrees(isFolded ? -90 : 0))
                }
                .contentShape(Rectangle())
            }
            .buttonStyle(.plain)
            if !isFolded { content }
        }
        .padding()
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(.thinMaterial, in: RoundedRectangle(cornerRadius: 16))
    }
}

private func of1Code(_ drivers: [Int: OF1Driver], _ n: Int) -> String { drivers[n]?.code ?? "#\(n)" }
private func of1Name(_ drivers: [Int: OF1Driver], _ n: Int) -> String { drivers[n]?.name ?? "#\(n)" }

private struct DriverRow: View {
    let pos: String
    let driver: OF1Driver?
    let fallback: Int
    let detail: String
    var body: some View {
        HStack(spacing: 8) {
            Text(pos).font(.body.monospacedDigit().weight(.bold)).frame(width: 30, alignment: .leading)
            Rectangle().fill(Color(hexString: driver?.colour ?? "888888")).frame(width: 3, height: 26)
            VStack(alignment: .leading, spacing: 0) {
                Text(driver?.name ?? "#\(fallback)").lineLimit(1)
                if let t = driver?.team { Text(t).font(.caption).foregroundStyle(.secondary).lineLimit(1) }
            }
            Spacer(minLength: 4)
            Text(detail).font(.caption.monospacedDigit()).foregroundStyle(.secondary).lineLimit(1)
        }
    }
}

struct ResultsSection: View {
    let key: Int
    let drivers: [Int: OF1Driver]
    @State private var results: [JSONValue] = []
    @State private var champD: [JSONValue] = []
    @State private var champT: [JSONValue] = []
    @State private var loaded = false

    var body: some View {
        VStack(spacing: 16) {
            Card(title: L("Classement de la séance", "Session classification")) {
                if !loaded { ProgressView() }
                ForEach(results, id: \.self) { r in
                    let pos = r["position"].int.map(String.init) ?? (r["dnf"].bool ? "DNF" : r["dns"].bool ? "DNS" : r["dsq"].bool ? "DSQ" : "–")
                    DriverRow(pos: pos, driver: drivers[r["driver_number"].int ?? 0], fallback: r["driver_number"].int ?? 0, detail: detail(r))
                }
            }
            if !champD.isEmpty {
                Card(title: L("Championnat pilotes", "Drivers' championship")) {
                    ForEach(champD, id: \.self) { c in
                        let gain = (c["points_current"].double ?? 0) - (c["points_start"].double ?? 0)
                        DriverRow(pos: c["position_current"].string, driver: drivers[c["driver_number"].int ?? 0], fallback: c["driver_number"].int ?? 0,
                                  detail: "\(c["points_current"].string) pts" + (gain > 0 ? " (+\(Int(gain)))" : ""))
                    }
                }
            }
            if !champT.isEmpty {
                Card(title: L("Championnat constructeurs", "Constructors' championship")) {
                    ForEach(champT, id: \.self) { c in
                        HStack {
                            Text(c["position_current"].string).font(.body.monospacedDigit().weight(.bold)).frame(width: 30, alignment: .leading)
                            Text(c["team_name"].string)
                            Spacer()
                            Text("\(c["points_current"].string) pts").font(.caption.monospacedDigit()).foregroundStyle(.secondary)
                        }
                    }
                }
            }
        }
        .task {
            async let r = of1("session_result", "session_key=\(key)")
            async let d = of1("championship_drivers", "session_key=\(key)")
            async let t = of1("championship_teams", "session_key=\(key)")
            results = await r.sorted { ($0["position"].int ?? 99) < ($1["position"].int ?? 99) }
            champD = await d.sorted { ($0["position_current"].int ?? 99) < ($1["position_current"].int ?? 99) }
            champT = await t.sorted { ($0["position_current"].int ?? 99) < ($1["position_current"].int ?? 99) }
            loaded = true
        }
    }

    private func detail(_ r: JSONValue) -> String {
        var parts: [String] = []
        if let g = r["gap_to_leader"].double, g > 0 { parts.append("+\(String(format: "%.3f", g))") }
        if case .str(let s) = r["gap_to_leader"] { parts.append(s) }
        if let p = r["points"].double, p > 0 { parts.append("\(p == p.rounded() ? String(Int(p)) : String(p)) pts") }
        if parts.isEmpty, let l = r["number_of_laps"].int { parts.append("\(l) \(L("tours", "laps"))") }
        return parts.joined(separator: " · ")
    }
}

/// Point d'une courbe.
private struct Pt: Identifiable {
    // Identifiant stable (un UUID neuf à chaque affichage faisait tout redessiner à Charts).
    var id: String { "\(series)-\(x)" }
    let series: String
    let x: Double
    let y: Double
}

struct LapsSection: View {
    let key: Int
    let drivers: [Int: OF1Driver]
    @State private var laps: [JSONValue] = []
    @State private var loaded = false

    var body: some View {
        let valid = laps.filter { $0["lap_duration"].double != nil && !$0["is_pit_out_lap"].bool }
        var bestBy: [Int: JSONValue] = [:]
        for l in valid {
            let d = l["driver_number"].int ?? 0
            if (l["lap_duration"].double ?? 1e9) < (bestBy[d]?["lap_duration"].double ?? 1e9) { bestBy[d] = l }
        }
        let best: [JSONValue] = bestBy.values.sorted { ($0["lap_duration"].double ?? 1e9) < ($1["lap_duration"].double ?? 1e9) }
        let top = best.prefix(5).compactMap { $0["driver_number"].int }
        let fastest = best.first?["lap_duration"].double ?? 0
        let pts: [Pt] = valid.compactMap { l in
            guard let d = l["driver_number"].int, top.contains(d), let t = l["lap_duration"].double, let n = l["lap_number"].int,
                  t < fastest * 1.12 else { return nil }
            return Pt(series: of1Code(drivers, d), x: Double(n), y: t)
        }
        return VStack(spacing: 16) {
            Card(title: L("Temps au tour (5 meilleurs)", "Lap times (top 5)")) {
                if !loaded { ProgressView() }
                Chart(pts) {
                    LineMark(x: .value(L("Tour", "Lap"), $0.x), y: .value(L("Temps", "Time"), $0.y))
                        .foregroundStyle(by: .value(L("Pilote", "Driver"), $0.series))
                }
                .chartForegroundStyleScale(range: palette)
                .chartYScale(domain: .automatic(includesZero: false))
                .chartYAxis { AxisMarks { v in
                        AxisGridLine()
                        AxisValueLabel {
                            if let d = v.as(Double.self) { Text(formatLap(d)) }
                        }
                    } }
                .frame(height: 220)
            }
            Card(title: L("Meilleurs tours", "Best laps")) {
                ForEach(Array(best.enumerated()), id: \.offset) { i, l in
                    let s1 = l["duration_sector_1"].double.map { String(format: "%.1f", $0) } ?? "–"
                    let s2 = l["duration_sector_2"].double.map { String(format: "%.1f", $0) } ?? "–"
                    let s3 = l["duration_sector_3"].double.map { String(format: "%.1f", $0) } ?? "–"
                    let trap = l["st_speed"].int.map { " · \($0) km/h" } ?? ""
                    VStack(alignment: .leading, spacing: 2) {
                        DriverRow(pos: "\(i + 1)", driver: drivers[l["driver_number"].int ?? 0], fallback: l["driver_number"].int ?? 0,
                                  detail: formatLap(l["lap_duration"].double ?? 0))
                        Text("S1 \(s1) · S2 \(s2) · S3 \(s3)\(trap) · \(L("tour", "lap")) \(l["lap_number"].string)")
                            .font(.caption2.monospacedDigit()).foregroundStyle(.secondary).padding(.leading, 41)
                    }
                }
            }
        }
        .task {
            laps = await of1("laps", "session_key=\(key)")
            loaded = true
        }
    }
}

struct PositionsSection: View {
    let key: Int
    let drivers: [Int: OF1Driver]
    @State private var positions: [JSONValue] = []
    @State private var intervals: [JSONValue] = []
    @State private var loaded = false

    var body: some View {
        let t0 = positions.compactMap { of1Date($0["date"].string) }.min() ?? Date()
        var last: [Int: Int] = [:]
        for p in positions { if let d = p["driver_number"].int, let v = p["position"].int { last[d] = v } }
        let top = last.sorted { $0.value < $1.value }.prefix(5).map(\.key)
        let pts: [Pt] = positions.compactMap { p in
            guard let d = p["driver_number"].int, top.contains(d), let v = p["position"].double, let t = of1Date(p["date"].string) else { return nil }
            return Pt(series: of1Code(drivers, d), x: t.timeIntervalSince(t0) / 60, y: v)
        }
        let gaps: [Pt] = intervals.enumerated().compactMap { i, p in
            guard i % 4 == 0, let d = p["driver_number"].int, top.contains(d), let g = p["gap_to_leader"].double, g < 120,
                  let t = of1Date(p["date"].string) else { return nil }
            return Pt(series: of1Code(drivers, d), x: t.timeIntervalSince(t0) / 60, y: g)
        }
        return VStack(spacing: 16) {
            Card(title: L("Positions (5 premiers à l'arrivée)", "Positions (top 5 at the finish)")) {
                if !loaded { ProgressView() }
                Chart(pts) {
                    LineMark(x: .value("min", $0.x), y: .value("Position", $0.y))
                        .interpolationMethod(.stepEnd)
                        .foregroundStyle(by: .value(L("Pilote", "Driver"), $0.series))
                }
                .chartForegroundStyleScale(range: palette)
                .chartYScale(domain: .automatic(includesZero: false, reversed: true))
                .chartXAxisLabel("min")
                .frame(height: 220)
            }
            if !gaps.isEmpty {
                Card(title: L("Écart au leader (s)", "Gap to leader (s)")) {
                    Chart(gaps) {
                        LineMark(x: .value("min", $0.x), y: .value(L("Écart", "Gap"), $0.y))
                            .foregroundStyle(by: .value(L("Pilote", "Driver"), $0.series))
                    }
                    .chartForegroundStyleScale(range: palette)
                    .chartXAxisLabel("min")
                    .frame(height: 200)
                }
            }
        }
        .task {
            async let p = of1("position", "session_key=\(key)")
            async let i = of1("intervals", "session_key=\(key)")
            positions = await p
            intervals = await i
            loaded = true
        }
    }
}

private struct Stint: Identifiable {
    let id = UUID()
    let driver: String
    let start: Double
    let end: Double
    let compound: String
}

struct TyresSection: View {
    let key: Int
    let drivers: [Int: OF1Driver]
    @State private var stints: [JSONValue] = []
    @State private var pits: [JSONValue] = []
    @State private var results: [JSONValue] = []
    @State private var loaded = false

    var body: some View {
        let order = results.sorted { ($0["position"].int ?? 99) < ($1["position"].int ?? 99) }.compactMap { $0["driver_number"].int }
        let all = order.isEmpty ? Array(Set(stints.compactMap { $0["driver_number"].int })).sorted() : order
        let bars: [Stint] = stints.compactMap { s in
            guard let d = s["driver_number"].int, let a = s["lap_start"].double else { return nil }
            let b = s["lap_end"].double ?? a
            return Stint(driver: of1Code(drivers, d), start: a - 1, end: b, compound: s["compound"].string)
        }
        return VStack(spacing: 16) {
            Card(title: L("Stratégie des pneus", "Tyre strategy")) {
                if !loaded { ProgressView() }
                Chart(bars) {
                    BarMark(xStart: .value(L("Tour", "Lap"), $0.start), xEnd: .value(L("Tour", "Lap"), $0.end), y: .value(L("Pilote", "Driver"), $0.driver))
                        .foregroundStyle(tyreColor($0.compound))
                }
                .chartYScale(domain: { var seen = Set<String>(); return all.map { of1Code(drivers, $0) }.filter { seen.insert($0).inserted } }())
                .frame(height: CGFloat(max(all.count, 1)) * 18 + 30)
                Text(L("Rouge tendre · jaune medium · blanc dur · vert intermédiaire · bleu pluie.", "Red soft · yellow medium · white hard · green intermediate · blue wet."))
                    .font(.caption).foregroundStyle(.secondary)
            }
            Card(title: L("Arrêts aux stands (\(pits.count))", "Pit stops (\(pits.count))")) {
                ForEach(pits, id: \.self) { p in
                    let stop = p["stop_duration"].double ?? p["pit_duration"].double
                    DriverRow(pos: "T\(p["lap_number"].string)", driver: drivers[p["driver_number"].int ?? 0], fallback: p["driver_number"].int ?? 0,
                              detail: stop.map { String(format: "%.1f s", $0) } ?? "")
                }
            }
        }
        .task {
            async let s = of1("stints", "session_key=\(key)")
            async let p = of1("pit", "session_key=\(key)")
            async let r = of1("session_result", "session_key=\(key)")
            stints = await s
            pits = await p
            results = await r
            loaded = true
        }
    }
}

struct TelemetrySection: View {
    let key: Int
    let drivers: [Int: OF1Driver]
    @State private var a = 0
    @State private var b = 0
    @State private var traces: [JSONValue] = []
    @State private var loading = false

    var body: some View {
        let list = drivers.values.sorted { $0.number < $1.number }
        Card(title: L("Télémétrie : meilleurs tours comparés", "Telemetry: best laps compared")) {
            HStack {
                Picker("A", selection: $a) { ForEach(list, id: \.number) { Text($0.name).tag($0.number) } }
                Picker("B", selection: $b) { ForEach(list, id: \.number) { Text($0.name).tag($0.number) } }
            }
            .pickerStyle(.menu)
            if loading { ProgressView() }
            Text(traces.map { "\(of1Code(drivers, $0["driver_number"].int ?? 0)) \(formatLap($0["lap_duration"].double ?? 0)) (T\($0["lap_number"].string))" }.joined(separator: " · "))
                .font(.caption.monospacedDigit()).foregroundStyle(.secondary)
            trace(L("Vitesse (km/h)", "Speed (km/h)"), "speed", 200)
            trace(L("Accélérateur (%)", "Throttle (%)"), "throttle", 120)
            trace(L("Rapport engagé", "Gear"), "gear", 110)
            Text(L("Données car_data OpenF1 (~4 mesures/s), alignées sur la distance parcourue.", "OpenF1 car_data (~4 samples/s), aligned on distance."))
                .font(.caption2).foregroundStyle(.secondary)
        }
        .task(id: drivers.count) {
            if a == 0, b == 0, list.count >= 2 {
                let res = await of1("session_result", "session_key=\(key)").sorted { ($0["position"].int ?? 99) < ($1["position"].int ?? 99) }
                a = res.first?["driver_number"].int ?? list[0].number
                b = res.dropFirst().first?["driver_number"].int ?? list[1].number
            }
        }
        .task(id: "\(a)-\(b)") {
            guard a != 0, b != 0 else { return }
            loading = true
            traces = (try? await ServerAPI.shared.get("api/of1/telemetry?session_key=\(key)&drivers=\(a),\(b)", as: [JSONValue].self, ttl: 3600)) ?? []
            loading = false
        }
    }

    @ViewBuilder
    private func trace(_ title: String, _ field: String, _ height: CGFloat) -> some View {
        let pts: [Pt] = traces.flatMap { t -> [Pt] in
            let dist = t["distance"].doubles, vals = t[field].doubles
            let label = of1Code(drivers, t["driver_number"].int ?? 0)
            return zip(dist, vals).map { Pt(series: label, x: $0 / 1000, y: $1) }
        }
        Text(title).font(.caption.weight(.semibold)).foregroundStyle(.secondary)
        Chart(pts) {
            LineMark(x: .value("km", $0.x), y: .value(title, $0.y))
                .interpolationMethod(field == "gear" ? .stepEnd : .linear)
                .foregroundStyle(by: .value(L("Pilote", "Driver"), $0.series))
        }
        .chartForegroundStyleScale(range: palette)
        .chartXAxisLabel("km")
        .frame(height: height)
    }
}

struct RaceSection: View {
    let key: Int
    let drivers: [Int: OF1Driver]
    @State private var overtakes: [JSONValue] = []
    @State private var control: [JSONValue] = []
    @State private var loaded = false

    var body: some View {
        let counts = Dictionary(grouping: overtakes.compactMap { $0["overtaking_driver_number"].int }, by: { $0 }).mapValues(\.count)
        let ranking = counts.sorted { $0.value > $1.value }
        return VStack(spacing: 16) {
            Card(title: L("Dépassements (\(overtakes.count))", "Overtakes (\(overtakes.count))")) {
                if !loaded { ProgressView() }
                ForEach(Array(ranking.prefix(10).enumerated()), id: \.offset) { i, e in
                    DriverRow(pos: "\(i + 1)", driver: drivers[e.key], fallback: e.key, detail: "\(e.value)")
                }
                if !overtakes.isEmpty {
                    DisclosureGroup(L("Tous les dépassements", "Every overtake")) {
                        ForEach(overtakes, id: \.self) { o in
                            Text("\(shortTime(o["date"].string)) · \(of1Code(drivers, o["overtaking_driver_number"].int ?? 0)) → \(of1Code(drivers, o["overtaken_driver_number"].int ?? 0)) · P\(o["position"].string)")
                                .font(.caption.monospacedDigit())
                                .frame(maxWidth: .infinity, alignment: .leading)
                        }
                    }
                }
            }
            Card(title: L("Direction de course", "Race control")) {
                ForEach(control.reversed(), id: \.self) { c in
                    VStack(alignment: .leading, spacing: 2) {
                        Text(c["message"].string).font(.subheadline)
                        Text([c["lap_number"].int.map { "\(L("Tour", "Lap")) \($0)" }, c["flag"].string.isEmpty ? nil : c["flag"].string, shortTime(c["date"].string)]
                            .compactMap { $0 }.joined(separator: " · "))
                            .font(.caption).foregroundStyle(.secondary)
                    }
                    .frame(maxWidth: .infinity, alignment: .leading)
                }
            }
        }
        .task {
            async let o = of1("overtakes", "session_key=\(key)")
            async let c = of1("race_control", "session_key=\(key)")
            overtakes = await o
            control = await c
            loaded = true
        }
    }
}

private func shortTime(_ iso: String) -> String {
    of1Date(iso)?.formatted(date: .omitted, time: .shortened) ?? ""
}

/// Lecteur des radios d'équipe (un seul à la fois).
@MainActor
final class RadioPlayer: ObservableObject {
    @Published var playing: String?
    private var player: AVPlayer?

    /// Durée d'un enregistrement (lue dans l'en-tête du fichier audio).
    static func duration(of url: String) async -> Double? {
        guard let u = URL(string: url), let d = try? await AVURLAsset(url: u).load(.duration) else { return nil }
        let seconds = CMTimeGetSeconds(d)
        return seconds.isFinite && seconds > 0 ? seconds : nil
    }

    func toggle(_ url: String) {
        if playing == url {
            player?.pause()
            playing = nil
            return
        }
        guard let u = URL(string: url) else { return }
        try? AVAudioSession.sharedInstance().setCategory(.playback)
        player = AVPlayer(url: u)
        player?.play()
        playing = url
    }
}

/// « 1:23:05 » ou « 4:07 ».
private func raceClock(_ seconds: Double) -> String {
    let t = Int(seconds.rounded())
    let h = t / 3600, m = t / 60 % 60, sec = t % 60
    return h > 0 ? String(format: "%d:%02d:%02d", h, m, sec) : String(format: "%d:%02d", m, sec)
}

/// Radio d'un pilote : tour, moment dans le tour, début et fin par rapport au départ, durée.
struct RadioRow: View {
    let radio: JSONValue
    @ObservedObject var player: RadioPlayer
    @State private var duration: Double?

    private var url: String { radio["url"].string }

    private var title: String {
        if radio["after_finish"].bool { return L("Après l'arrivée", "After the finish") }
        guard let lap = radio["lap"].int else { return L("Avant le départ", "Before the start") }
        let inLap = radio["in_lap"].double.map { L(" · \(raceClock($0)) dans le tour", " · \(raceClock($0)) into the lap") } ?? ""
        return L("Tour \(lap)", "Lap \(lap)") + inLap
    }

    private var detail: String {
        var parts: [String] = []
        if let start = radio["elapsed"].double, start >= 0 {
            parts.append(L("Début \(raceClock(start))", "Start \(raceClock(start))"))
            if let duration { parts.append(L("fin \(raceClock(start + duration))", "end \(raceClock(start + duration))")) }
        }
        if let duration { parts.append(L("durée \(raceClock(duration))", "length \(raceClock(duration))")) }
        parts.append(shortTime(radio["date"].string))
        return parts.joined(separator: " · ")
    }

    var body: some View {
        Button { player.toggle(url) } label: {
            HStack(spacing: 12) {
                Image(systemName: player.playing == url ? "stop.circle.fill" : "play.circle.fill")
                    .font(.title2)
                    .foregroundStyle(Color.f1Red)
                VStack(alignment: .leading, spacing: 2) {
                    Text(title).font(.subheadline.weight(.semibold))
                    Text(detail).font(.caption.monospacedDigit()).foregroundStyle(.secondary)
                }
                Spacer(minLength: 0)
            }
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .task(id: url) { duration = await RadioPlayer.duration(of: url) }
    }
}

struct RadioSection: View {
    let key: Int
    let drivers: [Int: OF1Driver]
    @State private var radio: [JSONValue] = []
    @State private var loaded = false
    @StateObject private var player = RadioPlayer()

    var body: some View {
        Card(title: L("Radios d'équipe (\(radio.count))", "Team radio (\(radio.count))")) {
            if !loaded { ProgressView() }
            ForEach(radio.reversed().prefix(60), id: \.self) { r in
                let url = r["recording_url"].string
                Button {
                    player.toggle(url)
                } label: {
                    HStack {
                        Image(systemName: player.playing == url ? "stop.circle.fill" : "play.circle.fill").font(.title2)
                        Rectangle().fill(Color(hexString: drivers[r["driver_number"].int ?? 0]?.colour ?? "888888")).frame(width: 3, height: 26)
                        VStack(alignment: .leading, spacing: 0) {
                            Text(of1Name(drivers, r["driver_number"].int ?? 0)).foregroundStyle(.primary)
                            Text(shortTime(r["date"].string)).font(.caption).foregroundStyle(.secondary)
                        }
                        Spacer()
                    }
                }
                .buttonStyle(.plain)
            }
        }
        .task {
            radio = await of1("team_radio", "session_key=\(key)")
            loaded = true
        }
    }
}

struct SessionWeatherSection: View {
    let key: Int
    @State private var weather: [JSONValue] = []
    @State private var loaded = false

    var body: some View {
        let t0 = weather.compactMap { of1Date($0["date"].string) }.min() ?? Date()
        func series(_ field: String, _ label: String, _ k: Double = 1) -> [Pt] {
            weather.compactMap { w in
                guard let v = w[field].double, let t = of1Date(w["date"].string) else { return nil }
                return Pt(series: label, x: t.timeIntervalSince(t0) / 60, y: v * k)
            }
        }
        let temps = series("track_temperature", L("Piste", "Track")) + series("air_temperature", "Air")
        let wind = series("wind_speed", L("Vent", "Wind"), 3.6)
        let last = weather.last ?? .null
        let rain = weather.contains { ($0["rainfall"].double ?? 0) > 0 }
        return Card(title: L("Météo de la séance", "Session weather")) {
            if !loaded { ProgressView() }
            LazyVGrid(columns: [GridItem(.flexible()), GridItem(.flexible()), GridItem(.flexible())], alignment: .leading, spacing: 10) {
                stat("Air", last["air_temperature"].double.map { String(format: "%.1f°", $0) })
                stat(L("Piste", "Track"), last["track_temperature"].double.map { String(format: "%.1f°", $0) })
                stat(L("Humidité", "Humidity"), last["humidity"].double.map { String(format: "%.0f%%", $0) })
                stat(L("Pression", "Pressure"), last["pressure"].double.map { String(format: "%.0f hPa", $0) })
                stat(L("Vent", "Wind"), last["wind_speed"].double.map { String(format: "%.0f km/h", $0 * 3.6) })
                stat(L("Pluie", "Rain"), rain ? L("oui", "yes") : L("non", "no"))
            }
            Text(L("Températures (°C)", "Temperatures (°C)")).font(.caption.weight(.semibold)).foregroundStyle(.secondary)
            Chart(temps) {
                LineMark(x: .value("min", $0.x), y: .value("°C", $0.y)).foregroundStyle(by: .value("", $0.series))
            }
            .chartForegroundStyleScale(range: [palette[1], palette[0]])
            .chartYScale(domain: .automatic(includesZero: false))
            .frame(height: 180)
            Text(L("Vent (km/h)", "Wind (km/h)")).font(.caption.weight(.semibold)).foregroundStyle(.secondary)
            Chart(wind) {
                LineMark(x: .value("min", $0.x), y: .value("km/h", $0.y)).foregroundStyle(palette[2])
            }
            .frame(height: 120)
        }
        .task {
            weather = await of1("weather", "session_key=\(key)")
            loaded = true
        }
    }

    private func stat(_ label: String, _ value: String?) -> some View {
        VStack(alignment: .leading, spacing: 2) {
            Text(label).font(.caption2).foregroundStyle(.secondary)
            Text(value ?? "–").font(.subheadline.weight(.semibold).monospacedDigit())
        }
    }
}

/// Lien « Analyse détaillée » d'une page de Grand Prix (saisons 2023+), retrouvé d'après la date.
struct MeetingLinkButton: View {
    let year: Int
    let date: String
    @State private var meeting: JSONValue?

    @State private var searched = false

    var body: some View {
        // Toujours une vue concrète : un Group vide dans une List perd son .task.
        VStack(alignment: .leading) {
            if let m = meeting, let key = m["meeting_key"].int {
                NavigationLink {
                    DataMeetingView(key: key, title: m["meeting_name"].string)
                } label: {
                    Label(L("Analyse détaillée (données OpenF1)", "Detailed analysis (OpenF1 data)"), systemImage: "chart.xyaxis.line")
                }
            } else if searched {
                Text(L("Pas de données OpenF1 pour ce Grand Prix.", "No OpenF1 data for this Grand Prix."))
                    .font(.footnote).foregroundStyle(.secondary)
            } else {
                ProgressView().frame(maxWidth: .infinity)
            }
        }
        .task {
            defer { searched = true }
            guard year >= 2023, let race = of1Date("\(date)T12:00:00Z") else { return }
            let list = await of1("meetings", "year=\(year)", ttl: 3600)
            meeting = list.first { m in
                guard let start = of1Date(m["date_start"].string) else { return false }
                let d = race.timeIntervalSince(start)
                return d >= -86_400 && d <= 5 * 86_400
            }
        }
    }
}


/// Tout ce que les données OpenF1 racontent de la course, affiché directement dans la page
/// du Grand Prix (Calendrier) : fiche de chaque pilote, résultats, tours, positions, pneus,
/// télémétrie, direction de course, radios, météo — un bouton par rubrique, sans changer d'écran.
struct RaceDataLinks: View {
    let year: Int
    let date: String
    @State private var session: JSONValue?
    @State private var drivers: [Int: OF1Driver] = [:]
    @State private var searched = false
    @State private var tab: DataTab = .driver

    private let columns = [GridItem(.adaptive(minimum: 104), spacing: 8)]

    var body: some View {
        VStack(alignment: .leading, spacing: 14) {
            if let s = session, let key = s["session_key"].int {
                LazyVGrid(columns: columns, spacing: 8) {
                    ForEach(DataTab.allCases) { t in
                        Button { tab = t } label: {
                            Label(t.label, systemImage: t.symbol)
                                .font(.caption.weight(.semibold))
                                .lineLimit(1)
                                .minimumScaleFactor(0.8)
                                .frame(maxWidth: .infinity, minHeight: 34)
                                .padding(.horizontal, 6)
                                .foregroundStyle(tab == t ? .white : .primary)
                                .background(tab == t ? Color.f1Red : Color.secondary.opacity(0.15), in: Capsule())
                        }
                        .buttonStyle(.plain)
                    }
                }
                Group {
                    switch tab {
                    case .results: ResultsSection(key: key, drivers: drivers)
                    case .driver: DriverRaceSection(key: key, drivers: drivers)
                    case .laps: LapsSection(key: key, drivers: drivers)
                    case .positions: PositionsSection(key: key, drivers: drivers)
                    case .tyres: TyresSection(key: key, drivers: drivers)
                    case .telemetry: TelemetrySection(key: key, drivers: drivers)
                    case .race: RaceSection(key: key, drivers: drivers)
                    case .radio: RadioSection(key: key, drivers: drivers)
                    case .weather: SessionWeatherSection(key: key)
                    }
                }
                .id("\(key)-\(tab.rawValue)")
                NavigationLink {
                    DataSessionView(key: key, title: "\(s["location"].string) — \(s["session_name"].string)", type: s["session_type"].string, start: tab)
                } label: {
                    Label(L("Ouvrir en plein écran", "Open full screen"), systemImage: "arrow.up.left.and.arrow.down.right")
                        .font(.footnote.weight(.semibold))
                }
            } else if searched {
                Text(L("Pas de données OpenF1 pour cette course.", "No OpenF1 data for this race."))
                    .font(.footnote).foregroundStyle(.secondary)
            } else {
                ProgressView().frame(maxWidth: .infinity)
            }
        }
        .task {
            defer { searched = true }
            guard year >= 2023, let race = of1Date("\(date)T12:00:00Z") else { return }
            let list = await of1("sessions", "year=\(year)&session_name=Race", ttl: 3600)
            session = list.first { s in
                guard let start = of1Date(s["date_start"].string) else { return false }
                return abs(race.timeIntervalSince(start)) <= 2 * 86_400
            }
            if let key = session?["session_key"].int {
                drivers = driverMap(await of1("drivers", "session_key=\(key)", ttl: 3600))
            }
        }
    }
}

/// Fiche course d'un pilote : résumé, temps au tour, tour par tour (secteurs, vitesses,
/// position, écarts, pneus, arrêts), télémétrie du tour choisi (vitesse, régime, gaz, frein,
/// rapport, DRS), messages de la direction de course et radios.
struct DriverRaceSection: View {
    let key: Int
    let drivers: [Int: OF1Driver]
    @State private var driver = 0
    @State private var data: JSONValue = .null
    @State private var lap = 0
    @State private var tel: JSONValue = .null
    @State private var loading = false
    @StateObject private var player = RadioPlayer()

    private var laps: [JSONValue] { data["laps"].array }
    private var bestLap: Int {
        laps.filter { !$0["pit_out"].bool }
            .min { ($0["time"].double ?? 1e9) < ($1["time"].double ?? 1e9) }?["lap"].int ?? 1
    }

    var body: some View {
        let list = drivers.values.sorted { $0.number < $1.number }
        VStack(spacing: 16) {
            Card(title: L("Fiche course du pilote", "Driver race file")) {
                Picker(L("Pilote", "Driver"), selection: $driver) {
                    ForEach(list, id: \.number) { Text($0.name).tag($0.number) }
                }
                .pickerStyle(.menu)
                if loading { ProgressView().frame(maxWidth: .infinity) }
                if data != .null {
                    summary
                }
            }
            if !laps.isEmpty {
                Card(title: L("Temps au tour", "Lap times")) { lapChart }
                Card(title: L("Tour par tour (\(laps.count))", "Lap by lap (\(laps.count))")) {
                    ForEach(laps, id: \.self) { l in lapRow(l) }
                }
                Card(title: L("Télémétrie d'un tour", "Lap telemetry")) {
                    Picker(L("Tour", "Lap"), selection: $lap) {
                        ForEach(laps.compactMap { $0["lap"].int }, id: \.self) { n in
                            Text(n == bestLap ? L("Tour \(n) (meilleur)", "Lap \(n) (fastest)") : L("Tour \(n)", "Lap \(n)")).tag(n)
                        }
                    }
                    .pickerStyle(.menu)
                    trace(L("Vitesse (km/h)", "Speed (km/h)"), "speed", 170, .stepEnd, false)
                    trace(L("Régime moteur (tr/min)", "Engine speed (rpm)"), "rpm", 120, .linear, false)
                    trace(L("Accélérateur (%)", "Throttle (%)"), "throttle", 100, .linear, false)
                    trace(L("Freinage", "Braking"), "brake", 60, .stepEnd, true)
                    trace(L("Rapport engagé", "Gear"), "gear", 100, .stepEnd, true)
                    // Plus de DRS depuis 2026 (aileron actif) : rien à tracer.
                    if !tel["drs"].doubles.isEmpty {
                        trace(L("DRS (100 ouvert · 50 autorisé)", "DRS (100 open · 50 armed)"), "drs", 60, .stepEnd, true)
                    }
                }
                // Replay du tour choisi : la voiture sur le circuit, tableau de bord synchronisé.
                if tel != .null {
                    Card(title: L("Replay du tour \(lap)", "Lap \(lap) replay")) {
                        LapReplayView(tel: tel, code: drivers[driver]?.code ?? "#\(driver)", colour: data["colour"].string)
                    }
                }
            }
            let messages = data["messages"].array
            if !messages.isEmpty {
                Card(title: L("Direction de course (\(messages.count))", "Race control (\(messages.count))")) {
                    ForEach(messages, id: \.self) { m in
                        VStack(alignment: .leading, spacing: 2) {
                            Text(m["message"].string).font(.subheadline)
                            Text([m["lap"].int.map { L("Tour \($0)", "Lap \($0)") }, Optional(shortTime(m["date"].string))].compactMap { $0 }.joined(separator: " · "))
                                .font(.caption).foregroundStyle(.secondary)
                        }
                        .frame(maxWidth: .infinity, alignment: .leading)
                    }
                }
            }
            let radios = data["radio"].array
            if !radios.isEmpty {
                Card(title: L("Radios (\(radios.count))", "Radio (\(radios.count))")) {
                    ForEach(radios, id: \.self) { r in
                        RadioRow(radio: r, player: player)
                    }
                }
            }
        }
        .task(id: drivers.count) {
            if driver == 0 {
                let res = await of1("session_result", "session_key=\(key)").sorted { ($0["position"].int ?? 99) < ($1["position"].int ?? 99) }
                driver = res.first?["driver_number"].int ?? list.first?.number ?? 0
            }
        }
        .task(id: driver) {
            guard driver != 0 else { return }
            loading = true
            data = (try? await ServerAPI.shared.get("api/of1/driverrace?session_key=\(key)&driver=\(driver)", as: JSONValue.self, ttl: 600)) ?? .null
            loading = false
            lap = bestLap
        }
        .task(id: "\(driver)-\(lap)") {
            guard driver != 0, lap != 0 else { return }
            let list = (try? await ServerAPI.shared.get("api/of1/telemetry?session_key=\(key)&drivers=\(driver)&lap=\(lap)", as: [JSONValue].self, ttl: 3600)) ?? []
            tel = list.first ?? .null
        }
    }

    private var summary: some View {
        let pos = { (k: String) in data[k].int.map { "P\($0)" } ?? "–" }
        let status = data["status"].string
        let champ: String = {
            guard let a = data["champ_before"].int, let b = data["champ_after"].int else { return "–" }
            return "P\(a) → P\(b)"
        }()
        let champPts: String = {
            guard let a = data["champ_points_before"].double, let b = data["champ_points_after"].double else { return "–" }
            return "\(Int(a)) → \(Int(b))"
        }()
        return VStack(alignment: .leading, spacing: 10) {
            HStack(spacing: 8) {
                Rectangle().fill(Color(hexString: data["colour"].string)).frame(width: 4, height: 34)
                VStack(alignment: .leading, spacing: 0) {
                    Text(data["name"].string.capitalized).font(.headline)
                    Text(data["team"].string).font(.caption).foregroundStyle(.secondary)
                }
            }
            LazyVGrid(columns: [GridItem(.flexible()), GridItem(.flexible())], alignment: .leading, spacing: 10) {
                stat(L("Départ → arrivée", "Start → finish"), "\(pos("grid")) → \(status.isEmpty ? pos("finish") : status)")
                stat("Points", data["points"].double.map { "\(Int($0))" } ?? "0")
                stat(L("Championnat", "Championship"), champ)
                stat(L("Points au championnat", "Championship points"), champPts)
                stat(L("Dépassements faits / subis", "Overtakes made / lost"), "\(data["overtakes_made"].string) / \(data["overtakes_lost"].string)")
                stat(L("Arrêts aux stands", "Pit stops"), data["pit_count"].string)
            }
        }
    }

    private func stat(_ label: String, _ value: String) -> some View {
        VStack(alignment: .leading, spacing: 2) {
            Text(label).font(.caption2).foregroundStyle(.secondary)
            Text(value).font(.subheadline.weight(.bold).monospacedDigit())
        }
    }

    private var lapChart: some View {
        let fastest = laps.compactMap { $0["time"].double }.min() ?? 0
        let pts: [Pt] = laps.compactMap { l in
            guard let n = l["lap"].double, let t = l["time"].double, t < fastest * 1.1 else { return nil }
            return Pt(series: data["code"].string, x: n, y: t)
        }
        return Chart(pts) {
            LineMark(x: .value(L("Tour", "Lap"), $0.x), y: .value(L("Temps", "Time"), $0.y))
                .foregroundStyle(Color(hexString: data["colour"].string))
        }
        .chartYScale(domain: .automatic(includesZero: false))
        .chartYAxis {
            AxisMarks { v in
                AxisGridLine()
                AxisValueLabel {
                    if let d = v.as(Double.self) { Text(formatLap(d)) }
                }
            }
        }
        .frame(height: 200)
    }

    private func gap(_ v: JSONValue) -> String {
        switch v {
        case .num(let g): return g == 0 ? "—" : String(format: "+%.1f s", g)
        case .str(let s): return s
        default: return "–"
        }
    }

    private func lapRow(_ l: JSONValue) -> some View {
        let sec = { (k: String) in l[k].double.map { String(format: "%.1f", $0) } ?? "–" }
        let spd = { (k: String) in l[k].int.map(String.init) ?? "–" }
        let badge: String? = l["pit_in"].bool
            ? (l["stop"].double.map { L("Arrêt \(String(format: "%.1f", $0)) s", "Stop \(String(format: "%.1f", $0)) s") } ?? L("Arrêt", "Pit"))
            : (l["pit_out"].bool ? L("Sortie", "Out") : nil)
        let age = l["tyre_age"].int.map { L(" · \($0) t", " · \($0) l") } ?? ""
        let sectors = "S1 \(sec("s1")) · S2 \(sec("s2")) · S3 \(sec("s3"))"
        let speeds = "\(spd("i1"))/\(spd("i2"))/\(spd("st")) km/h"
        let tyre = Text("● ").foregroundColor(tyreColor(l["compound"].string))
            + Text("\(l["compound"].string.capitalized)\(age)")
        let gaps = "\(L("leader", "leader")) \(gap(l["gap"])) · \(L("devant", "ahead")) \(gap(l["interval"]))"
        // Chaque information tient sur sa ligne, sans jamais couper un temps en morceaux.
        return VStack(alignment: .leading, spacing: 3) {
            HStack(alignment: .firstTextBaseline, spacing: 6) {
                Text("T\(l["lap"].string)")
                    .font(.caption.weight(.heavy))
                    .padding(.horizontal, 6).padding(.vertical, 2)
                    .background(Color.chip, in: Capsule())
                Text(l["time"].double.map(formatLap) ?? "–").font(.body.monospacedDigit().weight(.semibold))
                Text("P\(l["position"].string)").font(.footnote.weight(.bold)).foregroundStyle(.secondary)
                if let badge {
                    Label(badge, systemImage: "wrench.and.screwdriver.fill")
                        .font(.caption2.weight(.heavy)).foregroundStyle(.black)
                        .padding(.horizontal, 6).padding(.vertical, 2)
                        .background(Color.yellow, in: Capsule())
                }
                Spacer(minLength: 0)
            }
            .lineLimit(1)
            .minimumScaleFactor(0.75)
            Text("\(sectors) · \(speeds)")
                .font(.caption.monospacedDigit()).foregroundStyle(.secondary)
                .lineLimit(1).minimumScaleFactor(0.55)
            (tyre + Text(" · \(gaps)"))
                .font(.caption.monospacedDigit()).foregroundColor(.secondary)
                .lineLimit(1).minimumScaleFactor(0.55)
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .padding(.vertical, 2)
    }

    @ViewBuilder
    private func trace(_ title: String, _ field: String, _ height: CGFloat, _ interp: InterpolationMethod, _ stepped: Bool) -> some View {
        let dist = tel["distance"].doubles, vals = tel[field].doubles
        let pts: [Pt] = zip(dist, vals).map { Pt(series: field, x: $0 / 1000, y: $1) }
        Text(title).font(.caption.weight(.semibold)).foregroundStyle(.secondary)
        Chart(pts) {
            LineMark(x: .value("km", $0.x), y: .value(title, $0.y))
                .interpolationMethod(stepped ? .stepEnd : interp)
                .foregroundStyle(Color(hexString: data["colour"].string))
        }
        .chartXAxisLabel("km")
        // Assez haut pour que les graduations ne se chevauchent pas.
        .frame(height: max(height, 120))
    }
}
