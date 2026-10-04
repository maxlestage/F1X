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
        RaceLiveActivity()
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
        guard let url = URL(string: "api/\(path)", relativeTo: server),
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
            return NextRace(name: r["raceName"] as? String ?? "Grand Prix",
                            circuit: circuit?["circuitName"] as? String ?? "",
                            country: location?["country"] as? String ?? "",
                            round: r["round"] as? String ?? "",
                            start: start,
                            sprint: r["Sprint"] != nil)
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
            var next = Date().addingTimeInterval(3600)
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
                Text(L("Saison terminée", "Season over")).font(.headline)
                Spacer(minLength: 0)
            }
        }
    }
}

struct NextRaceWidget: Widget {
    var body: some WidgetConfiguration {
        StaticConfiguration(kind: "NextRace", provider: NextRaceProvider()) { entry in
            NextRaceWidgetView(entry: entry)
                .containerBackground(for: .widget) {
                    LinearGradient(colors: [Color(white: 0.09), Color(red: 0.16, green: 0.03, blue: 0.03)],
                                   startPoint: .topLeading, endPoint: .bottomTrailing)
                }
        }
        .configurationDisplayName(L("Prochain Grand Prix", "Next Grand Prix"))
        .description(L("Compte à rebours de la prochaine course et top 3 du championnat.",
                       "Countdown to the next race and championship top 3."))
        .supportedFamilies([.systemSmall, .systemMedium, .accessoryRectangular, .accessoryInline])
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
    case "chequered": return .white
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
            .activityBackgroundTint(Color(white: 0.08))
            .activitySystemActionForegroundColor(.white)
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
