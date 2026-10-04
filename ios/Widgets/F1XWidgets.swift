import ActivityKit
import SwiftUI
import WidgetKit

// Extension F1X : widget « Prochain Grand Prix » (écran d'accueil et écran verrouillé) et
// affichage de la Live Activity de la séance suivie (écran verrouillé + Dynamic Island).

private let server = URL(string: "https://f1x-29170430865f.herokuapp.com/")!
private let red = Color(red: 0.88, green: 0.02, blue: 0)
private var isFrench: Bool { Locale.preferredLanguages.first?.hasPrefix("fr") ?? true }
private func L(_ fr: String, _ en: String) -> String { isFrench ? fr : en }

private func colour(_ hex: String) -> Color {
    let v = UInt32(hex.trimmingCharacters(in: CharacterSet(charactersIn: "#")), radix: 16) ?? 0x8A8A99
    return Color(red: Double(v >> 16 & 0xFF) / 255, green: Double(v >> 8 & 0xFF) / 255, blue: Double(v & 0xFF) / 255)
}

@main
struct F1XWidgets: WidgetBundle {
    var body: some Widget {
        NextRaceWidget()
        StandingsWidget()
        LastRaceWidget()
        RaceLiveActivity()
        WeekendLiveActivity()
    }
}

// MARK: - Prochain Grand Prix

struct NextRace: Codable, Hashable {
    var name: String
    var circuit: String
    var country: String
    var round: String
    var start: Date
    var sprint: Bool
    var sessions: [SessionItem] = []
}

struct SessionItem: Codable, Hashable {
    var name: String
    var date: Date
}

private func teamColour(_ id: String) -> Color {
    let map: [String: String] = [
        "mercedes": "27F4D2", "ferrari": "E8002D", "red_bull": "3671C6", "mclaren": "FF8000",
        "aston_martin": "229971", "alpine": "FF87BC", "williams": "64C4FF", "rb": "6692FF",
        "haas": "B6BABD", "sauber": "52E252", "audi": "F50537", "cadillac": "C9A96E",
    ]
    return colour(map[id] ?? "8A8A99")
}

private let isoParser = ISO8601DateFormatter()

private func sessionDate(_ obj: Any?) -> Date? {
    guard let d = obj as? [String: Any], let date = d["date"] as? String else { return nil }
    let time = (d["time"] as? String) ?? "12:00:00Z"
    return isoParser.date(from: "\(date)T\(time.hasSuffix("Z") ? time : time + "Z")")
}

struct Leader: Codable, Hashable {
    var position: String
    var name: String
    var points: String
}

struct NextRaceEntry: TimelineEntry {
    let date: Date
    let race: NextRace?
    let leaders: [Leader]
}

/// Lecture minimale des réponses Jolpica relayées par le serveur F1X.
private enum Feed {
    static func json(_ path: String) async -> [String: Any]? {
        guard let url = URL(string: "api/f1/\(path)", relativeTo: server),
              let (data, _) = try? await URLSession.shared.data(from: url),
              let obj = try? JSONSerialization.jsonObject(with: data) as? [String: Any] else { return nil }
        return obj["MRData"] as? [String: Any]
    }

    static func nextRace() async -> NextRace? {
        guard let mr = await json("current.json?limit=100"),
              let races = (mr["RaceTable"] as? [String: Any])?["Races"] as? [[String: Any]] else { return nil }
        let iso = ISO8601DateFormatter()
        let now = Date()
        for r in races {
            let date = r["date"] as? String ?? ""
            let time = (r["time"] as? String) ?? "12:00:00Z"
            guard let start = iso.date(from: "\(date)T\(time.hasSuffix("Z") ? time : time + "Z")"),
                  start.addingTimeInterval(2 * 3600) > now else { continue }
            let circuit = r["Circuit"] as? [String: Any]
            let location = circuit?["Location"] as? [String: Any]
            let names: [(String, String, String)] = [
                ("FirstPractice", "Essais libres 1", "Practice 1"),
                ("SecondPractice", "Essais libres 2", "Practice 2"),
                ("ThirdPractice", "Essais libres 3", "Practice 3"),
                ("SprintQualifying", "Qualifs sprint", "Sprint qualifying"),
                ("SprintShootout", "Qualifs sprint", "Sprint qualifying"),
                ("Sprint", "Sprint", "Sprint"),
                ("Qualifying", "Qualifications", "Qualifying"),
            ]
            var sessions = names.compactMap { item -> SessionItem? in
                guard let date = sessionDate(r[item.0]) else { return nil }
                return SessionItem(name: L(item.1, item.2), date: date)
            }
            sessions.append(SessionItem(name: L("Course", "Race"), date: start))
            sessions.sort { $0.date < $1.date }
            return NextRace(name: r["raceName"] as? String ?? "Grand Prix",
                            circuit: circuit?["circuitName"] as? String ?? "",
                            country: location?["country"] as? String ?? "",
                            round: r["round"] as? String ?? "",
                            start: start,
                            sprint: r["Sprint"] != nil,
                            sessions: sessions)
        }
        return nil
    }

    static func leaders() async -> [Leader] {
        guard let mr = await json("current/driverStandings.json?limit=3"),
              let lists = (mr["StandingsTable"] as? [String: Any])?["StandingsLists"] as? [[String: Any]],
              let rows = lists.first?["DriverStandings"] as? [[String: Any]] else { return [] }
        return rows.prefix(3).map { s in
            let d = s["Driver"] as? [String: Any]
            let name = "\((d?["givenName"] as? String ?? "").prefix(1)). \(d?["familyName"] as? String ?? "")"
            return Leader(position: s["position"] as? String ?? s["positionText"] as? String ?? "",
                          name: name, points: s["points"] as? String ?? "")
        }
    }
}

struct NextRaceProvider: TimelineProvider {
    private static let sample = NextRaceEntry(
        date: Date(),
        race: NextRace(name: "Grand Prix", circuit: "Circuit", country: "", round: "1",
                       start: Date().addingTimeInterval(3 * 86_400), sprint: false),
        leaders: [Leader(position: "1", name: "M. Verstappen", points: "300")])

    func placeholder(in context: Context) -> NextRaceEntry { Self.sample }

    func getSnapshot(in context: Context, completion: @escaping (NextRaceEntry) -> Void) {
        if context.isPreview { completion(Self.sample); return }
        Task { completion(await load()) }
    }

    func getTimeline(in context: Context, completion: @escaping (Timeline<NextRaceEntry>) -> Void) {
        Task {
            let entry = await load()
            // Rafraîchi toutes les heures, et 2 h après le départ (résultats).
            // Hors ligne : nouvel essai dans 15 min.
            var next = Date().addingTimeInterval(entry.leaders.isEmpty ? 900 : 3600)
            if let start = entry.race?.start, start > Date(), start < next { next = start.addingTimeInterval(2 * 3600) }
            completion(Timeline(entries: [entry], policy: .after(next)))
        }
    }

    private func load() async -> NextRaceEntry {
        async let race = Feed.nextRace()
        async let leaders = Feed.leaders()
        return NextRaceEntry(date: Date(), race: await race, leaders: await leaders)
    }
}

struct NextRaceWidgetView: View {
    @Environment(\.widgetFamily) private var family
    let entry: NextRaceEntry

    var body: some View {
        switch family {
        case .accessoryInline:
            if let r = entry.race {
                Text("🏁 \(r.name) · ") + Text(r.start, style: .relative)
            } else {
                Text("F1X")
            }
        case .accessoryCircular:
            ZStack {
                AccessoryWidgetBackground()
                if let r = entry.race {
                    VStack(spacing: 0) {
                        Text("R\(r.round)").font(.system(size: 10, weight: .heavy))
                        Text(daysLeft(r.start)).font(.system(size: 15, weight: .black)).minimumScaleFactor(0.6)
                    }
                } else {
                    Text("F1X").font(.caption.bold())
                }
            }
        case .systemLarge:
            VStack(alignment: .leading, spacing: 10) {
                raceBlock.frame(maxHeight: 110)
                if let r = entry.race, !r.sessions.isEmpty {
                    Divider()
                    VStack(alignment: .leading, spacing: 5) {
                        ForEach(r.sessions, id: \.self) { s in
                            HStack {
                                Circle().fill(s.date.addingTimeInterval(3600) < entry.date ? Color.secondary : red).frame(width: 5, height: 5)
                                Text(s.name).font(.caption.weight(.semibold)).lineLimit(1)
                                Spacer()
                                Text(s.date.formatted(.dateTime.weekday(.abbreviated).hour().minute()))
                                    .font(.caption.monospacedDigit()).foregroundStyle(.secondary)
                            }
                        }
                    }
                }
                if !entry.leaders.isEmpty {
                    Divider()
                    HStack(spacing: 10) {
                        ForEach(entry.leaders, id: \.self) { l in
                            VStack(alignment: .leading, spacing: 1) {
                                Text("P\(l.position)").font(.caption2.bold()).foregroundStyle(red)
                                Text(l.name).font(.caption.weight(.semibold)).lineLimit(1).minimumScaleFactor(0.7)
                                Text("\(l.points) pts").font(.caption2.monospacedDigit()).foregroundStyle(.secondary)
                            }
                            .frame(maxWidth: .infinity, alignment: .leading)
                        }
                    }
                }
                Spacer(minLength: 0)
            }
        case .accessoryRectangular:
            VStack(alignment: .leading, spacing: 1) {
                Text(entry.race?.name ?? "F1X").font(.headline).lineLimit(1)
                if let r = entry.race {
                    Text(r.start, style: .relative).font(.caption).monospacedDigit()
                    Text(r.start.formatted(.dateTime.weekday(.abbreviated).day().hour().minute())).font(.caption2)
                }
            }
        case .systemMedium:
            HStack(alignment: .top, spacing: 14) {
                raceBlock
                if !entry.leaders.isEmpty {
                    Divider()
                    VStack(alignment: .leading, spacing: 5) {
                        Text(L("CHAMPIONNAT", "STANDINGS")).font(.caption2.weight(.heavy)).foregroundStyle(.secondary)
                        ForEach(entry.leaders, id: \.self) { l in
                            HStack(spacing: 6) {
                                Text(l.position).font(.caption.monospacedDigit().bold()).frame(width: 14, alignment: .trailing)
                                Text(l.name).font(.caption.weight(.semibold)).lineLimit(1)
                                Spacer(minLength: 2)
                                Text(l.points).font(.caption2.monospacedDigit()).foregroundStyle(.secondary)
                            }
                        }
                    }
                }
            }
        default:
            raceBlock
        }
    }

    private var raceBlock: some View {
        VStack(alignment: .leading, spacing: 4) {
            HStack(spacing: 4) {
                Text("F1").font(.caption.weight(.black).italic())
                Text("X").font(.caption.weight(.black).italic()).foregroundStyle(red)
                Spacer()
                if let r = entry.race { Text("R\(r.round)").font(.caption2.bold()).foregroundStyle(.secondary) }
            }
            if let r = entry.race {
                Text(r.name.replacingOccurrences(of: " Grand Prix", with: ""))
                    .font(.headline.weight(.heavy)).lineLimit(2).minimumScaleFactor(0.7)
                Text(r.start.formatted(.dateTime.weekday(.abbreviated).day().month(.abbreviated).hour().minute()))
                    .font(.caption2).foregroundStyle(.secondary).lineLimit(1)
                Spacer(minLength: 0)
                Text(r.start, style: .relative)
                    .font(.caption.weight(.bold).monospacedDigit()).foregroundStyle(red).lineLimit(1)
                if r.sprint { Text("SPRINT").font(.caption2.bold()).foregroundStyle(red) }
            } else {
                Text(entry.leaders.isEmpty ? L("Chargement…", "Loading…") : L("Saison terminée", "Season over")).font(.headline)
                Spacer(minLength: 0)
            }
        }
    }
}

struct NextRaceWidget: Widget {
    var body: some WidgetConfiguration {
        StaticConfiguration(kind: "NextRace", provider: NextRaceProvider()) { entry in
            NextRaceWidgetView(entry: entry)
                .containerBackground(for: .widget) { WidgetBackground() }
        }
        .configurationDisplayName(L("Prochain Grand Prix", "Next Grand Prix"))
        .description(L("Compte à rebours de la prochaine course et top 3 du championnat.",
                       "Countdown to the next race and championship top 3."))
        .supportedFamilies([.systemSmall, .systemMedium, .systemLarge, .accessoryCircular, .accessoryRectangular, .accessoryInline])
    }
}

/// « J-3 », « 5 h », « GO ».
private func daysLeft(_ date: Date) -> String {
    let seconds = date.timeIntervalSinceNow
    if seconds <= 0 { return "GO" }
    if seconds < 86_400 { return "\(Int(seconds / 3600)) h" }
    return L("J-\(Int(ceil(seconds / 86_400)))", "D-\(Int(ceil(seconds / 86_400)))")
}

private struct Logo: View {
    var body: some View {
        HStack(spacing: 0) {
            Text("F1").font(.caption.weight(.black).italic())
            Text("X").font(.caption.weight(.black).italic()).foregroundStyle(red)
        }
    }
}

/// Fond qui suit le thème de l'iPhone (clair le jour, sombre la nuit).
private struct WidgetBackground: View {
    @Environment(\.colorScheme) private var scheme
    var body: some View {
        LinearGradient(colors: scheme == .dark
                           ? [Color(white: 0.09), Color(red: 0.16, green: 0.03, blue: 0.03)]
                           : [Color.white, Color(red: 1, green: 0.92, blue: 0.91)],
                       startPoint: .topLeading, endPoint: .bottomTrailing)
    }
}

// MARK: - Classements

struct StandingRowData: Hashable {
    var position: String
    var name: String
    var team: String
    var points: String
}

struct StandingsEntry: TimelineEntry {
    let date: Date
    let drivers: [StandingRowData]
    let teams: [StandingRowData]
}

private extension Feed {
    static func standings(_ kind: String) async -> [StandingRowData] {
        let drivers = kind == "driverStandings"
        guard let mr = await json("current/\(kind).json?limit=10"),
              let lists = (mr["StandingsTable"] as? [String: Any])?["StandingsLists"] as? [[String: Any]],
              let rows = lists.first?[drivers ? "DriverStandings" : "ConstructorStandings"] as? [[String: Any]] else { return [] }
        return rows.map { s in
            let position = s["position"] as? String ?? s["positionText"] as? String ?? ""
            let points = s["points"] as? String ?? ""
            if drivers {
                let d = s["Driver"] as? [String: Any]
                let team = ((s["Constructors"] as? [[String: Any]])?.last?["constructorId"] as? String) ?? ""
                return StandingRowData(position: position, name: d?["familyName"] as? String ?? "", team: team, points: points)
            }
            let c = s["Constructor"] as? [String: Any]
            return StandingRowData(position: position, name: c?["name"] as? String ?? "",
                                   team: c?["constructorId"] as? String ?? "", points: points)
        }
    }
}

struct StandingsProvider: TimelineProvider {
    private static let sample = StandingsEntry(date: Date(), drivers: [
        StandingRowData(position: "1", name: "Verstappen", team: "red_bull", points: "300"),
        StandingRowData(position: "2", name: "Norris", team: "mclaren", points: "280"),
        StandingRowData(position: "3", name: "Leclerc", team: "ferrari", points: "250"),
    ], teams: [])

    func placeholder(in context: Context) -> StandingsEntry { Self.sample }

    func getSnapshot(in context: Context, completion: @escaping (StandingsEntry) -> Void) {
        if context.isPreview { completion(Self.sample); return }
        Task { completion(await load()) }
    }

    func getTimeline(in context: Context, completion: @escaping (Timeline<StandingsEntry>) -> Void) {
        Task { completion(Timeline(entries: [await load()], policy: .after(Date().addingTimeInterval(3600)))) }
    }

    private func load() async -> StandingsEntry {
        async let d = Feed.standings("driverStandings")
        async let t = Feed.standings("constructorStandings")
        return StandingsEntry(date: Date(), drivers: await d, teams: await t)
    }
}

private struct StandingLine: View {
    let row: StandingRowData
    var body: some View {
        HStack(spacing: 6) {
            Text(row.position).font(.caption.monospacedDigit().bold()).frame(width: 16, alignment: .trailing)
            RoundedRectangle(cornerRadius: 1).fill(teamColour(row.team)).frame(width: 3, height: 12)
            Text(row.name).font(.caption.weight(.semibold)).lineLimit(1)
            Spacer(minLength: 2)
            Text(row.points).font(.caption.monospacedDigit().bold())
        }
    }
}

struct StandingsWidgetView: View {
    @Environment(\.widgetFamily) private var family
    let entry: StandingsEntry

    var body: some View {
        VStack(alignment: .leading, spacing: 4) {
            HStack {
                Logo()
                Text(L("CHAMPIONNAT", "STANDINGS")).font(.caption2.weight(.heavy)).foregroundStyle(.secondary)
            }
            switch family {
            case .systemLarge:
                ForEach(entry.drivers.prefix(10), id: \.self) { StandingLine(row: $0) }
                if !entry.teams.isEmpty {
                    Divider()
                    ForEach(entry.teams.prefix(4), id: \.self) { StandingLine(row: $0) }
                }
            case .systemMedium:
                HStack(alignment: .top, spacing: 12) {
                    VStack(spacing: 3) { ForEach(entry.drivers.prefix(5), id: \.self) { StandingLine(row: $0) } }
                    VStack(spacing: 3) { ForEach(entry.teams.prefix(5), id: \.self) { StandingLine(row: $0) } }
                }
            default:
                ForEach(entry.drivers.prefix(5), id: \.self) { StandingLine(row: $0) }
            }
            Spacer(minLength: 0)
        }
    }
}

struct StandingsWidget: Widget {
    var body: some WidgetConfiguration {
        StaticConfiguration(kind: "Standings", provider: StandingsProvider()) { entry in
            StandingsWidgetView(entry: entry).containerBackground(for: .widget) { WidgetBackground() }
        }
        .configurationDisplayName(L("Championnat", "Championship"))
        .description(L("Classement des pilotes et des écuries.", "Driver and constructor standings."))
        .supportedFamilies([.systemSmall, .systemMedium, .systemLarge])
    }
}

// MARK: - Dernière course

struct PodiumRow: Hashable {
    var position: String
    var name: String
    var team: String
    var detail: String
}

struct LastRaceEntry: TimelineEntry {
    let date: Date
    let race: String
    let round: String
    let podium: [PodiumRow]
}

private extension Feed {
    static func lastRace() async -> LastRaceEntry? {
        guard let mr = await json("current/last/results.json?limit=10"),
              let race = ((mr["RaceTable"] as? [String: Any])?["Races"] as? [[String: Any]])?.first,
              let results = race["Results"] as? [[String: Any]] else { return nil }
        let podium = results.prefix(10).map { r -> PodiumRow in
            let d = r["Driver"] as? [String: Any]
            let c = r["Constructor"] as? [String: Any]
            let time = (r["Time"] as? [String: Any])?["time"] as? String ?? r["status"] as? String ?? ""
            return PodiumRow(position: r["positionText"] as? String ?? "", name: d?["familyName"] as? String ?? "",
                             team: c?["constructorId"] as? String ?? "", detail: time)
        }
        return LastRaceEntry(date: Date(), race: race["raceName"] as? String ?? "", round: race["round"] as? String ?? "", podium: podium)
    }
}

struct LastRaceProvider: TimelineProvider {
    private static let sample = LastRaceEntry(date: Date(), race: "Grand Prix", round: "1", podium: [
        PodiumRow(position: "1", name: "Verstappen", team: "red_bull", detail: "1:31:44.742"),
        PodiumRow(position: "2", name: "Norris", team: "mclaren", detail: "+2.307"),
        PodiumRow(position: "3", name: "Leclerc", team: "ferrari", detail: "+5.120"),
    ])

    func placeholder(in context: Context) -> LastRaceEntry { Self.sample }

    func getSnapshot(in context: Context, completion: @escaping (LastRaceEntry) -> Void) {
        if context.isPreview { completion(Self.sample); return }
        Task { completion(await Feed.lastRace() ?? Self.sample) }
    }

    func getTimeline(in context: Context, completion: @escaping (Timeline<LastRaceEntry>) -> Void) {
        Task {
            let entry = await Feed.lastRace() ?? LastRaceEntry(date: Date(), race: "F1X", round: "", podium: [])
            completion(Timeline(entries: [entry], policy: .after(Date().addingTimeInterval(3600))))
        }
    }
}

struct LastRaceWidgetView: View {
    @Environment(\.widgetFamily) private var family
    let entry: LastRaceEntry

    var body: some View {
        VStack(alignment: .leading, spacing: 4) {
            HStack {
                Logo()
                Text(L("DERNIÈRE COURSE", "LAST RACE")).font(.caption2.weight(.heavy)).foregroundStyle(.secondary).lineLimit(1)
            }
            Text(entry.race.replacingOccurrences(of: " Grand Prix", with: ""))
                .font(.headline.weight(.heavy)).lineLimit(1).minimumScaleFactor(0.7)
            ForEach(entry.podium.prefix(family == .systemSmall ? 3 : 5), id: \.self) { p in
                HStack(spacing: 6) {
                    Text(p.position == "1" ? "🥇" : p.position == "2" ? "🥈" : p.position == "3" ? "🥉" : p.position)
                        .font(.caption.monospacedDigit().bold()).frame(width: 20, alignment: .leading)
                    RoundedRectangle(cornerRadius: 1).fill(teamColour(p.team)).frame(width: 3, height: 12)
                    Text(p.name).font(.caption.weight(.semibold)).lineLimit(1)
                    if family != .systemSmall {
                        Spacer(minLength: 2)
                        Text(p.detail).font(.caption2.monospacedDigit()).foregroundStyle(.secondary).lineLimit(1)
                    }
                }
            }
            Spacer(minLength: 0)
        }
    }
}

struct LastRaceWidget: Widget {
    var body: some WidgetConfiguration {
        StaticConfiguration(kind: "LastRace", provider: LastRaceProvider()) { entry in
            LastRaceWidgetView(entry: entry).containerBackground(for: .widget) { WidgetBackground() }
        }
        .configurationDisplayName(L("Dernière course", "Last race"))
        .description(L("Podium et arrivée du dernier Grand Prix.", "Podium and finish of the last Grand Prix."))
        .supportedFamilies([.systemSmall, .systemMedium])
    }
}

// MARK: - Live Activity du week-end

/// « ven. 18:30 » (ou « 18:30 » si c'est aujourd'hui) : toujours juste, sans mise à jour.
private func sessionTime(_ date: Date) -> String {
    Calendar.current.isDateInToday(date)
        ? date.formatted(.dateTime.hour().minute())
        : date.formatted(.dateTime.weekday(.abbreviated).hour().minute())
}

/// « dans 2 jours, 3 heures » puis, une fois commencée, « En cours ».
private struct Until: View {
    let start: Date
    var body: some View {
        if start > Date() {
            Text(L("dans ", "in ")) + Text(start, style: .relative)
        } else {
            Text(L("En cours", "Live now")).foregroundColor(red)
        }
    }
}

private let gold = Color(red: 1, green: 0.78, blue: 0.2)

/// « 🏆 Dernier vainqueur : M. Verstappen (Malaysia) ».
private struct WinnerLine: View {
    let winner: String
    let race: String?
    var body: some View {
        HStack(spacing: 4) {
            Text("🏆")
            Text(L("Dernier vainqueur :", "Last winner:")).foregroundStyle(.secondary)
            Text(winner).fontWeight(.bold)
            if let race { Text("(\(race))").foregroundStyle(.secondary) }
            Spacer(minLength: 0)
        }
        .font(.caption2)
        .lineLimit(1)
    }
}

struct WeekendLiveActivity: Widget {
    var body: some WidgetConfiguration {
        ActivityConfiguration(for: WeekendActivityAttributes.self) { context in
            // Écran verrouillé.
            VStack(alignment: .leading, spacing: 6) {
                HStack(spacing: 6) {
                    Logo()
                    Text("\(context.attributes.flag) \(context.attributes.raceName)").font(.caption.bold()).lineLimit(1)
                    Spacer()
                    Text(L("Manche \(context.attributes.round)", "Round \(context.attributes.round)")).font(.caption2.bold()).foregroundStyle(.secondary)
                }
                if context.state.podium == true, let winner = context.state.winner {
                    Text(L("🏆 VAINQUEUR", "🏆 WINNER")).font(.caption2.weight(.heavy)).foregroundStyle(gold)
                    Text(winner).font(.title2.weight(.black)).lineLimit(1)
                    if let next = context.state.next {
                        Text(L("Prochain : \(next)", "Next: \(next)")).font(.caption2).foregroundStyle(.secondary).lineLimit(1)
                    }
                } else {
                    Text(L("PROCHAINE SÉANCE", "NEXT SESSION")).font(.caption2.weight(.heavy)).foregroundStyle(.secondary)
                    HStack(alignment: .firstTextBaseline) {
                        Text(context.state.session).font(.title3.weight(.heavy)).lineLimit(1)
                        Spacer()
                        Text(context.state.start.formatted(.dateTime.weekday(.wide).hour().minute()))
                            .font(.subheadline.weight(.bold)).foregroundStyle(red)
                    }
                    HStack {
                        Until(start: context.state.start).font(.caption.weight(.semibold))
                        Spacer()
                        if let next = context.state.next {
                            Text(L("Puis : \(next)", "Then: \(next)")).font(.caption2).foregroundStyle(.secondary).lineLimit(1)
                        }
                    }
                    if let winner = context.state.winner {
                        WinnerLine(winner: winner, race: context.state.winnerRace)
                    }
                }
            }
            .padding(14)
            .activityBackgroundTint(nil)
        } dynamicIsland: { context in
            DynamicIsland {
                DynamicIslandExpandedRegion(.leading) {
                    Text(context.attributes.flag).font(.largeTitle)
                }
                DynamicIslandExpandedRegion(.trailing) {
                    VStack(alignment: .trailing, spacing: 2) {
                        Text(L("Manche", "Round")).font(.caption2).foregroundStyle(.secondary)
                        Text(context.attributes.round).font(.title3.weight(.heavy))
                    }
                }
                DynamicIslandExpandedRegion(.center) {
                    VStack(spacing: 2) {
                        Text(context.attributes.raceName).font(.caption.bold()).lineLimit(1)
                        if context.state.podium == true, let winner = context.state.winner {
                            Text("🏆 \(winner)").font(.headline.weight(.heavy)).foregroundStyle(gold).lineLimit(1)
                        } else {
                            Text(context.state.session).font(.headline.weight(.heavy)).lineLimit(1)
                        }
                    }
                }
                DynamicIslandExpandedRegion(.bottom) {
                    if context.state.podium == true {
                        if let next = context.state.next {
                            Text(L("Prochain : \(next)", "Next: \(next)")).font(.caption2).foregroundStyle(.secondary)
                                .lineLimit(1).frame(maxWidth: .infinity, alignment: .leading)
                        }
                    } else {
                    VStack(spacing: 4) {
                        HStack {
                            Text(context.state.start.formatted(.dateTime.weekday(.wide).hour().minute()))
                                .font(.subheadline.weight(.bold)).foregroundStyle(red)
                            Spacer()
                            Until(start: context.state.start).font(.caption.weight(.semibold))
                        }
                        if let next = context.state.next {
                            Text(L("Puis : \(next)", "Then: \(next)")).font(.caption2).foregroundStyle(.secondary)
                                .lineLimit(1).frame(maxWidth: .infinity, alignment: .leading)
                        }
                        if let winner = context.state.winner {
                            WinnerLine(winner: winner, race: context.state.winnerRace)
                        }
                    }
                    }
                }
            } compactLeading: {
                // Pastille gauche : drapeau + séance (EL1, Q, Course…).
                HStack(spacing: 4) {
                    Text(context.attributes.flag)
                    Text(context.state.short).fontWeight(.heavy).foregroundStyle(.white)
                }
                .font(.caption)
            } compactTrailing: {
                if context.state.podium == true, let code = context.state.winnerCode {
                    // Après l'arrivée : le vainqueur (« VER »).
                    Text(code).font(.caption.weight(.heavy)).foregroundStyle(gold)
                } else {
                    // Pastille droite : jour et heure de la séance (« ven. 18:30 »).
                    Text(sessionTime(context.state.start))
                        .font(.caption.weight(.bold))
                        .foregroundStyle(red)
                        .lineLimit(1)
                        .minimumScaleFactor(0.7)
                }
            } minimal: {
                Text(context.state.podium == true ? "🏆" : context.attributes.flag).font(.caption)
            }
            .keylineTint(red)
        }
    }
}

// MARK: - Live Activity

private func statusText(_ s: String) -> String {
    switch s {
    case "yellow": return L("Drapeau jaune", "Yellow flag")
    case "safety_car": return L("Voiture de sécurité", "Safety car")
    case "virtual_safety_car": return "VSC"
    case "red": return L("Drapeau rouge", "Red flag")
    case "chequered": return L("Arrivée", "Chequered")
    default: return L("Piste verte", "Green")
    }
}

private func statusColour(_ s: String) -> Color {
    switch s {
    case "yellow", "safety_car", "virtual_safety_car": return .yellow
    case "red": return .red
    case "chequered": return .gray
    default: return .green
    }
}

private func progress(_ c: RaceActivityAttributes.ContentState, _ a: RaceActivityAttributes) -> String {
    guard a.racing else { return a.sessionName }
    if let t = c.totalLaps { return L("Tour \(c.lap)/\(t)", "Lap \(c.lap)/\(t)") }
    return L("Tour \(c.lap)", "Lap \(c.lap)")
}

private struct LeaderRow: View {
    let e: RaceActivityAttributes.Entry
    var body: some View {
        HStack(spacing: 6) {
            Text("\(e.position)").font(.caption.monospacedDigit().bold()).frame(width: 16, alignment: .trailing)
            RoundedRectangle(cornerRadius: 1).fill(colour(e.colour)).frame(width: 3, height: 13)
            Text(e.code).font(.caption.bold())
            Spacer(minLength: 4)
            Text(e.gap.isEmpty ? L("Leader", "Leader") : e.gap).font(.caption2.monospacedDigit()).foregroundStyle(.secondary)
        }
    }
}

struct RaceLiveActivity: Widget {
    var body: some WidgetConfiguration {
        ActivityConfiguration(for: RaceActivityAttributes.self) { context in
            // Écran verrouillé / bannière.
            VStack(alignment: .leading, spacing: 6) {
                HStack {
                    Text("F1").font(.caption.weight(.black).italic()) + Text("X").font(.caption.weight(.black).italic()).foregroundColor(red)
                    Text("\(context.attributes.location) · \(context.attributes.sessionName)").font(.caption.bold()).lineLimit(1)
                    Spacer()
                    Text(progress(context.state, context.attributes)).font(.caption.monospacedDigit().bold())
                }
                HStack(spacing: 6) {
                    Circle().fill(statusColour(context.state.status)).frame(width: 7, height: 7)
                    Text(statusText(context.state.status)).font(.caption2).foregroundStyle(.secondary)
                    Spacer()
                    Text(context.state.updated, style: .time).font(.caption2).foregroundStyle(.secondary)
                }
                ForEach(context.state.leaders, id: \.self) { LeaderRow(e: $0) }
            }
            .padding(14)
            .activityBackgroundTint(nil)
        } dynamicIsland: { context in
            let first = context.state.leaders.first
            return DynamicIsland {
                DynamicIslandExpandedRegion(.leading) {
                    Text(progress(context.state, context.attributes)).font(.caption.monospacedDigit().bold())
                }
                DynamicIslandExpandedRegion(.trailing) {
                    HStack(spacing: 4) {
                        Circle().fill(statusColour(context.state.status)).frame(width: 7, height: 7)
                        Text(statusText(context.state.status)).font(.caption2).lineLimit(1)
                    }
                }
                DynamicIslandExpandedRegion(.center) {
                    Text(context.attributes.location).font(.caption.bold()).lineLimit(1)
                }
                DynamicIslandExpandedRegion(.bottom) {
                    VStack(spacing: 3) {
                        ForEach(context.state.leaders, id: \.self) { LeaderRow(e: $0) }
                    }
                }
            } compactLeading: {
                HStack(spacing: 3) {
                    RoundedRectangle(cornerRadius: 1).fill(colour(first?.colour ?? "E10600")).frame(width: 3, height: 12)
                    Text(first?.code ?? "F1X").font(.caption2.bold())
                }
            } compactTrailing: {
                Text(context.attributes.racing ? "T\(context.state.lap)" : "P1")
                    .font(.caption2.monospacedDigit().bold())
                    .foregroundStyle(statusColour(context.state.status))
            } minimal: {
                Text(first?.code.prefix(1).description ?? "F").font(.caption2.bold()).foregroundStyle(red)
            }
            .keylineTint(red)
        }
    }
}
