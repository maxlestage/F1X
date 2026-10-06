import SwiftUI
import WidgetKit

/// App F1X pour l'Apple Watch : prochain Grand Prix, dernière course, classements.
@main
struct F1XWatchApp: App {
    @Environment(\.scenePhase) private var scenePhase

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

    var body: some View {
        NavigationStack {
            TabView {
                NextRacePage(race: model.next, loaded: model.loaded)
                LastRacePage(race: model.last, loaded: model.loaded)
                StandingsPage(title: WL("Pilotes", "Drivers"), rows: model.drivers, loaded: model.loaded)
                StandingsPage(title: WL("Écuries", "Teams"), rows: model.teams, loaded: model.loaded)
            }
            .tabViewStyle(.verticalPage)
        }
        .tint(f1Red)
        .task { await model.load(force: true) }
        .onChange(of: scenePhase) { _, phase in
            if phase == .active { Task { await model.load() } }
        }
    }
}

private struct Logo: View {
    var body: some View {
        HStack(spacing: 0) {
            Text("F1").font(.caption.weight(.black).italic())
            Text("X").font(.caption.weight(.black).italic()).foregroundStyle(f1Red)
        }
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

// MARK: - Prochain Grand Prix

private struct NextRacePage: View {
    let race: WRace?
    let loaded: Bool

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 6) {
                HStack {
                    Logo()
                    Spacer()
                    if let race { Text("R\(race.round)").font(.caption2.bold()).foregroundStyle(.secondary) }
                }
                if let race {
                    Text("\(race.flag) \(race.shortName)").font(.headline).lineLimit(2)
                    if let s = race.nextSession {
                        Text(s.name.uppercased()).font(.caption2.weight(.heavy)).foregroundStyle(.secondary)
                        if s.date > Date() {
                            Text(s.date, style: .relative)
                                .font(.title3.weight(.heavy).monospacedDigit())
                                .foregroundStyle(f1Red)
                        } else {
                            Text(WL("En cours", "Live now")).font(.title3.weight(.heavy)).foregroundStyle(f1Red)
                        }
                        Text(s.date.formatted(.dateTime.weekday(.wide).hour().minute()))
                            .font(.caption)
                    }
                    Divider().padding(.vertical, 2)
                    ForEach(race.sessions, id: \.self) { s in
                        HStack {
                            Circle()
                                .fill(s.date.addingTimeInterval(3600) < Date() ? Color.secondary : f1Red)
                                .frame(width: 5, height: 5)
                            Text(s.name).font(.caption2).lineLimit(1)
                            Spacer(minLength: 2)
                            Text(s.date.formatted(.dateTime.weekday(.abbreviated).hour().minute()))
                                .font(.caption2.monospacedDigit()).foregroundStyle(.secondary)
                        }
                    }
                } else {
                    Placeholder(loaded: loaded)
                }
            }
        }
        .navigationTitle(WL("Prochain GP", "Next GP"))
    }
}

// MARK: - Dernière course

private struct LastRacePage: View {
    let race: WLastRace?
    let loaded: Bool

    var body: some View {
        List {
            if let race {
                Section {
                    if let winner = race.rows.first {
                        VStack(alignment: .leading, spacing: 2) {
                            Text(WL("🏆 VAINQUEUR", "🏆 WINNER")).font(.caption2.weight(.heavy)).foregroundStyle(f1Gold)
                            Text(winner.name).font(.title3.weight(.heavy))
                            Text("\(race.flag) \(race.name.replacingOccurrences(of: " Grand Prix", with: ""))")
                                .font(.caption2).foregroundStyle(.secondary)
                        }
                    }
                }
                Section {
                    ForEach(race.rows.dropFirst(), id: \.self) { r in
                        Row(row: r, trailing: r.value)
                    }
                }
            } else {
                Placeholder(loaded: loaded)
            }
        }
        .navigationTitle(WL("Dernière course", "Last race"))
    }
}

// MARK: - Classements

private struct StandingsPage: View {
    let title: String
    let rows: [WRow]
    let loaded: Bool

    var body: some View {
        List {
            if rows.isEmpty {
                Placeholder(loaded: loaded)
            }
            ForEach(rows, id: \.self) { r in
                Row(row: r, trailing: "\(r.value) pts")
            }
        }
        .navigationTitle(title)
    }
}

private struct Row: View {
    let row: WRow
    let trailing: String

    var body: some View {
        HStack(spacing: 6) {
            Text(row.position).font(.caption.monospacedDigit().bold()).frame(width: 20, alignment: .trailing)
            RoundedRectangle(cornerRadius: 1.5).fill(teamColor(row.team)).frame(width: 3, height: 16)
            Text(row.name).font(.footnote.weight(.semibold)).lineLimit(1).minimumScaleFactor(0.7)
            Spacer(minLength: 2)
            Text(trailing).font(.caption2.monospacedDigit()).foregroundStyle(.secondary).lineLimit(1)
        }
    }
}
