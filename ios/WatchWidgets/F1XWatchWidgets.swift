import SwiftUI
import WidgetKit

// Complications F1X pour les cadrans de l'Apple Watch.

@main
struct F1XWatchWidgets: WidgetBundle {
    var body: some Widget {
        CountdownComplication()
        LeaderComplication()
    }
}

// MARK: - Compte à rebours

struct CountdownEntry: TimelineEntry {
    let date: Date
    let race: WRace?
}

struct CountdownProvider: TimelineProvider {
    static let sample = CountdownEntry(date: Date(), race: WRace(
        name: "Singapore Grand Prix", flag: "🇸🇬", round: "17", circuit: "Marina Bay",
        start: Date().addingTimeInterval(3 * 86_400),
        sessions: [WSession(name: WL("Course", "Race"), date: Date().addingTimeInterval(3 * 86_400))]))

    func placeholder(in context: Context) -> CountdownEntry { Self.sample }

    func getSnapshot(in context: Context, completion: @escaping (CountdownEntry) -> Void) {
        if context.isPreview { completion(Self.sample); return }
        Task { completion(CountdownEntry(date: Date(), race: await WatchFeed.nextRace())) }
    }

    func getTimeline(in context: Context, completion: @escaping (Timeline<CountdownEntry>) -> Void) {
        Task {
            let race = await WatchFeed.nextRace()
            // Une entrée par heure : « J-3 » → « 5 h » sans réseau ; nouvelles données toutes les 6 h.
            let now = Date()
            let entries = (0..<12).map { CountdownEntry(date: now.addingTimeInterval(Double($0) * 3600), race: race) }
            let refresh = now.addingTimeInterval(race == nil ? 900 : 6 * 3600)
            completion(Timeline(entries: entries, policy: .after(refresh)))
        }
    }
}

struct CountdownView: View {
    @Environment(\.widgetFamily) private var family
    let entry: CountdownEntry

    var body: some View {
        let session = entry.race?.sessions.first { $0.date.addingTimeInterval(3600) > entry.date }
        switch family {
        case .accessoryCircular:
            ZStack {
                AccessoryWidgetBackground()
                VStack(spacing: 0) {
                    Text(entry.race?.flag ?? "🏁").font(.system(size: 12))
                    Text(session.map { shortCountdown($0.date, now: entry.date) } ?? "F1X")
                        .font(.system(size: 14, weight: .black))
                        .minimumScaleFactor(0.6)
                        .widgetAccentable()
                }
            }
        case .accessoryCorner:
            Text(entry.race?.flag ?? "🏁")
                .font(.title3)
                .widgetLabel {
                    Text(session.map { "\(entry.race?.shortName ?? "") · \(shortCountdown($0.date, now: entry.date))" } ?? "F1X")
                }
        case .accessoryInline:
            if let race = entry.race, let session {
                Text("\(race.flag) \(session.name) · \(shortCountdown(session.date, now: entry.date))")
            } else {
                Text("F1X")
            }
        default:
            VStack(alignment: .leading, spacing: 1) {
                if let race = entry.race {
                    Text("\(race.flag) \(race.shortName)").font(.headline).lineLimit(1).widgetAccentable()
                    if let session {
                        Text(session.name).font(.caption).lineLimit(1)
                        if session.date > entry.date {
                            Text(session.date, style: .relative).font(.caption.monospacedDigit()).foregroundStyle(f1Red)
                        } else {
                            Text(WL("En cours", "Live now")).font(.caption.bold()).foregroundStyle(f1Red)
                        }
                    }
                } else {
                    Text("F1X").font(.headline)
                }
            }
            .frame(maxWidth: .infinity, alignment: .leading)
        }
    }
}

struct CountdownComplication: Widget {
    var body: some WidgetConfiguration {
        StaticConfiguration(kind: "F1XCountdown", provider: CountdownProvider()) { entry in
            CountdownView(entry: entry).containerBackground(.clear, for: .widget)
        }
        .configurationDisplayName(WL("Prochaine séance", "Next session"))
        .description(WL("Compte à rebours jusqu'à la prochaine séance de F1.", "Countdown to the next F1 session."))
        .supportedFamilies([.accessoryCircular, .accessoryRectangular, .accessoryInline, .accessoryCorner])
    }
}

// MARK: - Championnat

struct LeaderEntry: TimelineEntry {
    let date: Date
    let rows: [WRow]
}

struct LeaderProvider: TimelineProvider {
    static let sample = LeaderEntry(date: Date(), rows: [
        WRow(position: "1", name: "Verstappen", code: "VER", team: "red_bull", value: "312"),
        WRow(position: "2", name: "Antonelli", code: "ANT", team: "mercedes", value: "302"),
        WRow(position: "3", name: "Norris", code: "NOR", team: "mclaren", value: "280"),
    ])

    func placeholder(in context: Context) -> LeaderEntry { Self.sample }

    func getSnapshot(in context: Context, completion: @escaping (LeaderEntry) -> Void) {
        if context.isPreview { completion(Self.sample); return }
        Task { completion(LeaderEntry(date: Date(), rows: await WatchFeed.drivers(limit: 3))) }
    }

    func getTimeline(in context: Context, completion: @escaping (Timeline<LeaderEntry>) -> Void) {
        Task {
            let rows = await WatchFeed.drivers(limit: 3)
            completion(Timeline(entries: [LeaderEntry(date: Date(), rows: rows)],
                                policy: .after(Date().addingTimeInterval(rows.isEmpty ? 900 : 3 * 3600))))
        }
    }
}

struct LeaderView: View {
    @Environment(\.widgetFamily) private var family
    let entry: LeaderEntry

    var body: some View {
        switch family {
        case .accessoryCircular:
            ZStack {
                AccessoryWidgetBackground()
                VStack(spacing: 0) {
                    Text("🏆").font(.system(size: 11))
                    Text(entry.rows.first?.code ?? "F1X").font(.system(size: 13, weight: .black)).widgetAccentable()
                }
            }
        case .accessoryInline:
            Text(entry.rows.first.map { "🏆 \($0.name) · \($0.value) pts" } ?? "F1X")
        default:
            VStack(alignment: .leading, spacing: 1) {
                ForEach(entry.rows.prefix(3), id: \.self) { r in
                    HStack(spacing: 4) {
                        Text(r.position).font(.caption.monospacedDigit().bold()).frame(width: 12, alignment: .trailing)
                        Text(r.code).font(.caption.bold()).widgetAccentable()
                        Spacer(minLength: 2)
                        Text(r.value).font(.caption.monospacedDigit())
                    }
                }
            }
        }
    }
}

struct LeaderComplication: Widget {
    var body: some WidgetConfiguration {
        StaticConfiguration(kind: "F1XLeader", provider: LeaderProvider()) { entry in
            LeaderView(entry: entry).containerBackground(.clear, for: .widget)
        }
        .configurationDisplayName(WL("Championnat", "Championship"))
        .description(WL("Le top 3 du championnat pilotes.", "Drivers' championship top 3."))
        .supportedFamilies([.accessoryCircular, .accessoryRectangular, .accessoryInline])
    }
}
