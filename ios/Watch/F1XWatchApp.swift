import SwiftUI
import WidgetKit

/// App F1X pour l'Apple Watch : prochain Grand Prix, dernière course, classements.
@main
struct F1XWatchApp: App {
    @Environment(\.scenePhase) private var scenePhase

    init() {
        PhoneSync.shared.start()
    }

    var body: some Scene {
        WindowGroup {
            WatchRoot()
        }
        .onChange(of: scenePhase) { _, phase in
            if phase == .background { WidgetCenter.shared.reloadAllTimelines() }
        }
    }
}

@MainActor
final class WatchModel: ObservableObject {
    @Published var next: WRace?
    @Published var last: WLastRace?
    @Published var drivers: [WRow] = []
    @Published var teams: [WRow] = []
    @Published var loaded = false
    private var updated = Date.distantPast

    func load(force: Bool = false) async {
        guard force || Date().timeIntervalSince(updated) > 120 else { return }
        async let n = WatchFeed.nextRace()
        async let l = WatchFeed.lastRace()
        async let d = WatchFeed.drivers()
        async let t = WatchFeed.teams()
        let (next, last, drivers, teams) = await (n, l, d, t)
        if next != nil || !drivers.isEmpty { updated = Date() }
        self.next = next ?? self.next
        self.last = last ?? self.last
        if !drivers.isEmpty { self.drivers = drivers }
        if !teams.isEmpty { self.teams = teams }
        loaded = true
    }
}

struct WatchRoot: View {
    @StateObject private var model = WatchModel()
    @Environment(\.scenePhase) private var scenePhase
    /// Langue envoyée par l'iPhone : l'interface se reconstruit quand elle change.
    @AppStorage("language") private var language = ""

    var body: some View {
        NavigationStack {
            TabView {
                NextRacePage(race: model.next, loaded: model.loaded)
                    .containerBackground(f1Red.gradient, for: .tabView)
                LastRacePage(race: model.last, loaded: model.loaded)
                    .containerBackground(f1Gold.opacity(0.55).gradient, for: .tabView)
                StandingsPage(title: WL("Pilotes", "Drivers"), rows: model.drivers, loaded: model.loaded, drivers: true)
                    .containerBackground(Color.blue.opacity(0.55).gradient, for: .tabView)
                StandingsPage(title: WL("Écuries", "Teams"), rows: model.teams, loaded: model.loaded, drivers: false)
                    .containerBackground(Color.teal.opacity(0.5).gradient, for: .tabView)
            }
            .tabViewStyle(.verticalPage)
        }
        .id(language)
        .environment(\.locale, wLocale)
        .tint(.white)
        .task { await model.load(force: true) }
        .onChange(of: scenePhase) { _, phase in
            if phase == .active { Task { await model.load() } }
        }
    }
}

/// Petit titre en capitales, comme dans les apps Apple (« RESSENTI », « VENT »).
private struct Caption: View {
    let text: String
    var body: some View {
        Text(text.uppercased()).font(.system(size: 12, weight: .semibold)).foregroundStyle(.white.opacity(0.7))
    }
}

private struct Placeholder: View {
    let loaded: Bool
    var body: some View {
        if loaded {
            Text(WL("Données indisponibles", "Data unavailable")).font(.footnote).foregroundStyle(.secondary)
        } else {
            ProgressView()
        }
    }
}

/// Séances du week-end : passées, prochaine, à venir.
private struct Steps: View {
    let race: WRace
    var body: some View {
        let next = race.nextSession
        HStack(spacing: 3) {
            ForEach(race.sessions, id: \.self) { s in
                let done = s.date.addingTimeInterval(3600) < Date()
                let current = s == next
                VStack(spacing: 2) {
                    Capsule().fill(current ? Color.white : .white.opacity(done ? 0.55 : 0.2)).frame(height: 4)
                    Text(shortSession(s.name))
                        .font(.system(size: 9, weight: current ? .heavy : .medium))
                        .foregroundStyle(current ? .white : .white.opacity(0.6))
                        .lineLimit(1).minimumScaleFactor(0.6)
                }
            }
        }
    }
}

// MARK: - Prochain Grand Prix

private struct NextRacePage: View {
    let race: WRace?
    let loaded: Bool

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 10) {
                if let race {
                    VStack(alignment: .leading, spacing: 0) {
                        Text("\(race.flag) \(race.shortName)").font(.title3.weight(.bold)).lineLimit(2)
                        Text(WL("Manche \(race.round) · \(race.circuit)", "Round \(race.round) · \(race.circuit)"))
                            .font(.footnote).foregroundStyle(.white.opacity(0.7))
                    }
                    if let s = race.nextSession {
                        VStack(alignment: .leading, spacing: 1) {
                            Caption(text: s.name)
                            if s.date > Date() {
                                Text(s.date, style: .relative)
                                    .font(.system(size: 26, weight: .semibold).monospacedDigit())
                                    .lineLimit(2).minimumScaleFactor(0.6)
                            } else {
                                Text(WL("En cours", "Live now")).font(.system(size: 26, weight: .semibold))
                            }
                            Text(s.date.formatted(.dateTime.weekday(.wide).hour().minute().locale(wLocale)))
                                .font(.footnote).foregroundStyle(.white.opacity(0.7))
                        }
                    }
                    Steps(race: race)
                    VStack(alignment: .leading, spacing: 6) {
                        Caption(text: WL("Programme", "Schedule"))
                        ForEach(race.sessions, id: \.self) { s in
                            HStack {
                                Text(s.name).font(.footnote).lineLimit(1)
                                Spacer(minLength: 2)
                                Text(s.date.formatted(.dateTime.weekday(.abbreviated).hour().minute().locale(wLocale)))
                                    .font(.footnote.monospacedDigit()).foregroundStyle(.white.opacity(0.7))
                            }
                            .opacity(s.date.addingTimeInterval(3600) < Date() ? 0.5 : 1)
                        }
                    }
                } else {
                    Placeholder(loaded: loaded)
                }
            }
            .frame(maxWidth: .infinity, alignment: .leading)
        }
        .navigationTitle(WL("Prochain GP", "Next GP"))
    }
}

// MARK: - Dernière course

private struct LastRacePage: View {
    let race: WLastRace?
    let loaded: Bool

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 10) {
                if let race, let winner = race.rows.first {
                    VStack(alignment: .leading, spacing: 0) {
                        Caption(text: WL("Vainqueur", "Winner"))
                        HStack(alignment: .firstTextBaseline, spacing: 6) {
                            Text("🏆").font(.title3)
                            Text(winner.name).font(.system(size: 26, weight: .semibold)).lineLimit(1).minimumScaleFactor(0.6)
                        }
                        Text("\(race.flag) \(race.name.replacingOccurrences(of: " Grand Prix", with: ""))")
                            .font(.footnote).foregroundStyle(.white.opacity(0.7))
                    }
                    if race.rows.count > 2 {
                        HStack(spacing: 6) {
                            ForEach(race.rows.prefix(3).dropFirst(), id: \.self) { r in
                                VStack(alignment: .leading, spacing: 0) {
                                    Caption(text: "P\(r.position)")
                                    Text(r.code).font(.headline)
                                    Text(r.value).font(.caption2.monospacedDigit()).foregroundStyle(.white.opacity(0.7)).lineLimit(1)
                                }
                                .frame(maxWidth: .infinity, alignment: .leading)
                            }
                        }
                    }
                    VStack(spacing: 5) {
                        ForEach(race.rows.dropFirst(3), id: \.self) { r in
                            Row(row: r, trailing: r.value)
                        }
                    }
                } else {
                    Placeholder(loaded: loaded)
                }
            }
            .frame(maxWidth: .infinity, alignment: .leading)
        }
        .navigationTitle(WL("Dernière course", "Last race"))
    }
}

// MARK: - Classements

private struct StandingsPage: View {
    let title: String
    let rows: [WRow]
    let loaded: Bool
    let drivers: Bool

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 8) {
                if let leader = rows.first {
                    VStack(alignment: .leading, spacing: 0) {
                        Caption(text: WL("Leader", "Leader"))
                        Text(leader.name).font(.system(size: 24, weight: .semibold)).lineLimit(1).minimumScaleFactor(0.6)
                        Text("\(leader.value) pts").font(.footnote.monospacedDigit()).foregroundStyle(.white.opacity(0.7))
                    }
                    let top = Double(leader.value) ?? 1
                    VStack(spacing: 6) {
                        ForEach(rows.dropFirst(), id: \.self) { r in
                            VStack(spacing: 2) {
                                Row(row: r, trailing: "\(r.value)")
                                // Écart au leader en barre.
                                GeometryReader { g in
                                    Capsule().fill(teamColor(r.team))
                                        .frame(width: max(3, g.size.width * CGFloat((Double(r.value) ?? 0) / max(top, 1))), height: 3)
                                }
                                .frame(height: 3)
                            }
                        }
                    }
                } else {
                    Placeholder(loaded: loaded)
                }
            }
            .frame(maxWidth: .infinity, alignment: .leading)
        }
        .navigationTitle(title)
    }
}

private struct Row: View {
    let row: WRow
    let trailing: String

    var body: some View {
        HStack(spacing: 6) {
            Text(row.position).font(.caption.monospacedDigit().bold()).frame(width: 18, alignment: .trailing)
            RoundedRectangle(cornerRadius: 1.5).fill(teamColor(row.team)).frame(width: 3, height: 14)
            Text(row.name).font(.footnote.weight(.semibold)).lineLimit(1).minimumScaleFactor(0.7)
            Spacer(minLength: 2)
            Text(trailing).font(.caption2.monospacedDigit()).foregroundStyle(.white.opacity(0.75)).lineLimit(1)
        }
    }
}
