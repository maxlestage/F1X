import SwiftUI
import WidgetKit

// Complications F1X pour les cadrans de l'Apple Watch (et la pile intelligente).

@main
struct F1XWatchWidgets: WidgetBundle {
    var body: some Widget {
        CountdownComplication()
        WinnerComplication()
        LeaderComplication()
    }
}

// MARK: - Compte à rebours

struct CountdownEntry: TimelineEntry {
    let date: Date
    let race: WRace?

    var session: WSession? { race?.sessions.first { $0.date.addingTimeInterval(3600) > date } }
    /// Remplissage de l'anneau : se complète pendant la dernière semaine avant la séance.
    var progress: Double {
        guard let s = session else { return 0 }
        return 1 - min(max(s.date.timeIntervalSince(date) / (7 * 86_400), 0), 1)
    }
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
        let session = entry.session
        switch family {
        case .accessoryCircular:
            // Anneau qui se remplit à l'approche de la séance.
            Gauge(value: entry.progress) {
                Text(entry.race?.flag ?? "🏁")
            } currentValueLabel: {
                VStack(spacing: -1) {
                    Text(session.map { shortSession($0.name) } ?? "F1X").font(.system(size: 9, weight: .heavy))
                    Text(session.map { shortCountdown($0.date, now: entry.date) } ?? "–")
                        .font(.system(size: 13, weight: .black)).minimumScaleFactor(0.6)
                }
            }
            .gaugeStyle(.accessoryCircularCapacity)
            .tint(f1Red)
            .widgetAccentable()
        case .accessoryCorner:
            Text(entry.race?.flag ?? "🏁")
                .font(.title3)
                .widgetLabel {
                    Gauge(value: entry.progress) {
                        Text("F1")
                    } currentValueLabel: {
                        Text(session.map { shortCountdown($0.date, now: entry.date) } ?? "")
                    } minimumValueLabel: {
                        Text(session.map { shortSession($0.name) } ?? "")
                    } maximumValueLabel: {
                        Text(session.map { shortCountdown($0.date, now: entry.date) } ?? "")
                    }
                    .tint(f1Red)
                }
        case .accessoryInline:
            if let race = entry.race, let session {
                Text("\(race.flag) \(shortSession(session.name)) · \(shortCountdown(session.date, now: entry.date))")
            } else {
                Text("F1X")
            }
        default:
            HStack(spacing: 6) {
                RoundedRectangle(cornerRadius: 2).fill(f1Red).frame(width: 4).widgetAccentable()
                VStack(alignment: .leading, spacing: 0) {
                    if let race = entry.race {
                        Text("\(race.flag) \(race.shortName)").font(.headline).lineLimit(1).widgetAccentable()
                        if let session {
                            Text(session.name).font(.caption).foregroundStyle(.secondary).lineLimit(1)
                            if session.date > entry.date {
                                Text(session.date.formatted(.dateTime.weekday(.abbreviated).hour().minute().locale(wLocale)))
                                    + Text(" · ") + Text(shortCountdown(session.date, now: entry.date)).bold()
                            } else {
                                Text(WL("En cours", "Live now")).bold()
                            }
                        }
                    } else {
                        Text("F1X").font(.headline)
                    }
                }
                .font(.caption)
                Spacer(minLength: 0)
            }
        }
    }
}

struct CountdownComplication: Widget {
    var body: some WidgetConfiguration {
        StaticConfiguration(kind: "F1XCountdown", provider: CountdownProvider()) { entry in
            CountdownView(entry: entry)
                .environment(\.locale, wLocale)
                .containerBackground(.clear, for: .widget)
        }
        .configurationDisplayName(WL("Prochaine séance", "Next session"))
        .description(WL("Compte à rebours jusqu'à la prochaine séance de F1.", "Countdown to the next F1 session."))
        .supportedFamilies([.accessoryCircular, .accessoryRectangular, .accessoryInline, .accessoryCorner])
    }
}

// MARK: - Vainqueur

struct WinnerEntry: TimelineEntry {
    let date: Date
    let race: WLastRace?
}

struct WinnerProvider: TimelineProvider {
    static let sample = WinnerEntry(date: Date(), race: WLastRace(name: "Bahrain Grand Prix", flag: "🇧🇭", rows: [
        WRow(position: "1", name: "Verstappen", code: "VER", team: "red_bull", value: "1:47:14.808"),
        WRow(position: "2", name: "Antonelli", code: "ANT", team: "mercedes", value: "+2.307"),
        WRow(position: "3", name: "Norris", code: "NOR", team: "mclaren", value: "+5.120"),
    ]))

    func placeholder(in context: Context) -> WinnerEntry { Self.sample }

    func getSnapshot(in context: Context, completion: @escaping (WinnerEntry) -> Void) {
        if context.isPreview { completion(Self.sample); return }
        Task { completion(WinnerEntry(date: Date(), race: await WatchFeed.lastRace())) }
    }

    func getTimeline(in context: Context, completion: @escaping (Timeline<WinnerEntry>) -> Void) {
        Task {
            let race = await WatchFeed.lastRace()
            completion(Timeline(entries: [WinnerEntry(date: Date(), race: race)],
                                policy: .after(Date().addingTimeInterval(race == nil ? 900 : 3 * 3600))))
        }
    }
}

struct WinnerView: View {
    @Environment(\.widgetFamily) private var family
    let entry: WinnerEntry

    var body: some View {
        let winner = entry.race?.rows.first
        switch family {
        case .accessoryCircular:
            ZStack {
                AccessoryWidgetBackground()
                VStack(spacing: -1) {
                    Text("🏆").font(.system(size: 13))
                    Text(winner?.code ?? "F1X").font(.system(size: 13, weight: .black)).widgetAccentable()
                }
            }
        case .accessoryInline:
            Text(winner.map { "🏆 \($0.name) · \(entry.race?.flag ?? "")" } ?? "F1X")
        default:
            HStack(spacing: 6) {
                RoundedRectangle(cornerRadius: 2).fill(f1Gold).frame(width: 4).widgetAccentable()
                VStack(alignment: .leading, spacing: 0) {
                    Text(WL("🏆 VAINQUEUR", "🏆 WINNER")).font(.caption2.weight(.heavy)).foregroundStyle(f1Gold).widgetAccentable()
                    Text(winner?.name ?? "–").font(.headline).lineLimit(1)
                    Text("\(entry.race?.flag ?? "") \(entry.race?.name.replacingOccurrences(of: " Grand Prix", with: "") ?? "")")
                        .font(.caption).foregroundStyle(.secondary).lineLimit(1)
                }
                Spacer(minLength: 0)
            }
        }
    }
}

struct WinnerComplication: Widget {
    var body: some WidgetConfiguration {
        StaticConfiguration(kind: "F1XWinner", provider: WinnerProvider()) { entry in
            WinnerView(entry: entry)
                .environment(\.locale, wLocale)
                .containerBackground(.clear, for: .widget)
        }
        .configurationDisplayName(WL("Dernier vainqueur", "Last winner"))
        .description(WL("Le vainqueur du dernier Grand Prix.", "Winner of the latest Grand Prix."))
        .supportedFamilies([.accessoryCircular, .accessoryRectangular, .accessoryInline])
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
            // Avance du leader sur le 2e, en anneau.
            let first = Double(entry.rows.first?.value ?? "") ?? 0
            let second = Double(entry.rows.dropFirst().first?.value ?? "") ?? 0
            Gauge(value: first > 0 ? second / first : 0) {
                Text("P1")
            } currentValueLabel: {
                VStack(spacing: -1) {
                    Text(entry.rows.first?.code ?? "F1X").font(.system(size: 12, weight: .black))
                    Text("+\(Int(first - second))").font(.system(size: 10, weight: .semibold))
                }
            }
            .gaugeStyle(.accessoryCircularCapacity)
            .tint(teamColor(entry.rows.first?.team ?? ""))
            .widgetAccentable()
        case .accessoryInline:
            Text(entry.rows.first.map { "🏆 \($0.name) · \($0.value) pts" } ?? "F1X")
        default:
            VStack(alignment: .leading, spacing: 1) {
                ForEach(entry.rows.prefix(3), id: \.self) { r in
                    HStack(spacing: 4) {
                        Text(r.position).font(.caption.monospacedDigit().bold()).frame(width: 12, alignment: .trailing)
                        RoundedRectangle(cornerRadius: 1).fill(teamColor(r.team)).frame(width: 3, height: 11)
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
            LeaderView(entry: entry)
                .environment(\.locale, wLocale)
                .containerBackground(.clear, for: .widget)
        }
        .configurationDisplayName(WL("Championnat", "Championship"))
        .description(WL("Le top 3 du championnat pilotes.", "Drivers' championship top 3."))
        .supportedFamilies([.accessoryCircular, .accessoryRectangular, .accessoryInline])
    }
}
