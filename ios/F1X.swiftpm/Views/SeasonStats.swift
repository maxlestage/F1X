import Charts
import SwiftUI

/// Statistiques d'une saison : évolution du championnat, classements par catégorie
/// et duels entre coéquipiers (course et qualifications).
struct SeasonStatsView: View {
    var season: String = "current"
    @State private var stats: SeasonStats?
    @State private var failed = false

    var body: some View {
        List {
            if let stats {
                if !stats.progression.isEmpty {
                    FoldSection(L("Évolution du championnat", "Championship progression")) {
                        ProgressionChart(series: stats.progression)
                            .frame(height: 260)
                            .padding(.vertical, 8)
                    }
                }
                if !stats.duels.isEmpty {
                    FoldSection(L("Duels entre coéquipiers", "Teammate battles")) {
                        ForEach(stats.duels) { DuelRow(duel: $0) }
                    }
                }
                ForEach(stats.boards) { board in
                    FoldSection(board.title) {
                        ForEach(Array(board.rows.enumerated()), id: \.offset) { i, row in
                            NavigationLink(value: row.driver) {
                                HStack(spacing: 10) {
                                    Text("\(i + 1)")
                                        .font(.callout.monospacedDigit().bold())
                                        .foregroundStyle(.secondary)
                                        .frame(width: 20, alignment: .leading)
                                    RoundedRectangle(cornerRadius: 1.5)
                                        .fill(Team.color(row.team))
                                        .frame(width: 3, height: 18)
                                    driverTitle(row.driver).lineLimit(1)
                                    Spacer()
                                    Text(row.value).font(.body.weight(.heavy).monospacedDigit())
                                }
                            }
                        }
                    }
                }
            } else if failed {
                ContentUnavailableView(L("Données indisponibles", "Data unavailable"), systemImage: "wifi.slash")
            } else {
                ProgressView().frame(maxWidth: .infinity)
            }
        }
        .listStyle(.insetGrouped)
        .navigationTitle(L("Statistiques", "Statistics"))
        .task(id: season) { await F1API.instant { await load() } }
        .autoRefresh { await load() }
        .refreshable {
            await F1API.shared.expireCache()
            await load()
        }
    }

    private func load() async {
        async let r = try? F1API.shared.seasonRaces(season)
        async let s = try? F1API.shared.seasonRaces(season, kind: "sprint")
        async let q = try? F1API.shared.seasonRaces(season, kind: "qualifying")
        let (results, sprints, quali) = await (r, s, q)
        guard let results, !results.isEmpty else {
            if stats == nil { failed = true }
            return
        }
        failed = false
        stats = SeasonStats.build(results: results, sprints: sprints ?? [], quali: quali ?? [])
    }
}

// MARK: - Calculs

struct StatRow {
    let driver: Driver
    let team: String
    let value: String
}

struct StatBoard: Identifiable {
    let id: String
    let title: String
    let rows: [StatRow]
}

struct ProgressPoint: Identifiable {
    let round: Int
    let total: Double
    var id: Int { round }
}

struct ProgressSeries: Identifiable {
    let id: String
    let name: String
    let team: String
    let dashed: Bool
    let points: [ProgressPoint]
}

struct TeammateDuel: Identifiable {
    let id: String
    let team: String
    let a: Driver
    let b: Driver
    let race: (Int, Int)
    let quali: (Int, Int)
    let points: (Double, Double)
}

private struct DuelCount {
    var team: String
    var a: String
    var b: String
    var race = (0, 0)
    var quali = (0, 0)
}

struct SeasonStats {
    let progression: [ProgressSeries]
    let boards: [StatBoard]
    let duels: [TeammateDuel]

    static func build(results: [Race], sprints: [Race], quali: [Race]) -> SeasonStats {
        var drivers: [String: Driver] = [:]
        var team: [String: String] = [:]
        var wins: [String: Int] = [:], podiums: [String: Int] = [:], poles: [String: Int] = [:]
        var fastest: [String: Int] = [:], dnfs: [String: Int] = [:], scored: [String: Int] = [:]
        var gained: [String: Int] = [:]
        var finishes: [String: [Int]] = [:]
        var perRound: [Int: [String: Double]] = [:]
        var duels: [String: DuelCount] = [:]

        func tally(_ entries: [(team: String, id: String, pos: Int)], race: Bool) {
            let byTeam = Dictionary(grouping: entries, by: { $0.team })
            for (t, list) in byTeam where list.count >= 2 {
                let sorted = list.sorted { $0.id < $1.id }
                for i in 0..<(sorted.count - 1) {
                    for j in (i + 1)..<sorted.count {
                        let x = sorted[i], y = sorted[j]
                        let key = "\(t)|\(x.id)|\(y.id)"
                        var d = duels[key] ?? DuelCount(team: t, a: x.id, b: y.id)
                        let first = x.pos < y.pos
                        if race {
                            if first { d.race.0 += 1 } else { d.race.1 += 1 }
                        } else {
                            if first { d.quali.0 += 1 } else { d.quali.1 += 1 }
                        }
                        duels[key] = d
                    }
                }
            }
        }

        for race in results {
            var entries: [(team: String, id: String, pos: Int)] = []
            for r in race.results ?? [] {
                let id = r.driver.driverId
                drivers[id] = r.driver
                team[id] = r.constructor.constructorId
                let classified = Int(r.positionText)
                if classified == 1 { wins[id, default: 0] += 1 }
                if let p = classified, p <= 3 { podiums[id, default: 0] += 1 }
                if let p = classified { finishes[id, default: []].append(p) } else { dnfs[id, default: 0] += 1 }
                if r.hasFastestLap { fastest[id, default: 0] += 1 }
                if r.scoredPoints { scored[id, default: 0] += 1 }
                if let p = classified, let g = Int(r.grid ?? ""), g > 0 { gained[id, default: 0] += g - p }
                perRound[race.roundNumber, default: [:]][id, default: 0] += Double(r.points) ?? 0
                if let p = Int(r.position) { entries.append((team: r.constructor.constructorId, id: id, pos: p)) }
            }
            tally(entries, race: true)
        }
        for race in sprints {
            for r in race.sprintResults ?? [] {
                drivers[r.driver.driverId] = drivers[r.driver.driverId] ?? r.driver
                perRound[race.roundNumber, default: [:]][r.driver.driverId, default: 0] += Double(r.points) ?? 0
            }
        }
        for race in quali {
            var entries: [(team: String, id: String, pos: Int)] = []
            for q in race.qualifyingResults ?? [] {
                if q.position == "1" { poles[q.driver.driverId, default: 0] += 1 }
                if let p = Int(q.position) { entries.append((team: q.constructor.constructorId, id: q.driver.driverId, pos: p)) }
            }
            tally(entries, race: false)
        }

        // Points cumulés course après course.
        var totals: [String: Double] = [:]
        var history: [String: [ProgressPoint]] = [:]
        for round in perRound.keys.sorted() {
            for (id, pts) in perRound[round] ?? [:] { totals[id, default: 0] += pts }
            for (id, total) in totals { history[id, default: []].append(ProgressPoint(round: round, total: total)) }
        }
        let top = totals.sorted { $0.value > $1.value }.prefix(6).map { $0.key }
        var seenTeams: Set<String> = []
        let progression = top.compactMap { id -> ProgressSeries? in
            guard let d = drivers[id] else { return nil }
            let t = team[id] ?? ""
            let dashed = seenTeams.contains(t)
            seenTeams.insert(t)
            return ProgressSeries(id: id, name: d.code ?? d.familyName, team: t, dashed: dashed, points: history[id] ?? [])
        }

        func board(_ id: String, _ title: String, _ counts: [String: Int]) -> StatBoard? {
            let rows = counts
                .filter { $0.value > 0 }
                .sorted { $0.value != $1.value ? $0.value > $1.value : $0.key < $1.key }
                .prefix(5)
                .compactMap { e -> StatRow? in
                    guard let d = drivers[e.key] else { return nil }
                    return StatRow(driver: d, team: team[e.key] ?? "", value: e.value > 0 && id == "gained" ? "+\(e.value)" : "\(e.value)")
                }
            return rows.isEmpty ? nil : StatBoard(id: id, title: title, rows: Array(rows))
        }

        // Position moyenne à l'arrivée (pilotes ayant vu au moins la moitié des arrivées).
        let minimum = max(1, results.count / 2)
        let average = finishes
            .filter { $0.value.count >= minimum }
            .map { (id: $0.key, avg: Double($0.value.reduce(0, +)) / Double($0.value.count)) }
            .sorted { $0.avg < $1.avg }
            .prefix(5)
            .compactMap { e -> StatRow? in
                guard let d = drivers[e.id] else { return nil }
                return StatRow(driver: d, team: team[e.id] ?? "", value: String(format: "%.1f", e.avg))
            }

        var boards = [
            board("wins", L("Victoires", "Wins"), wins),
            board("podiums", L("Podiums", "Podiums"), podiums),
            board("poles", L("Pole positions", "Pole positions"), poles),
            board("scored", L("Arrivées dans les points", "Points finishes"), scored),
            board("gained", L("Places gagnées (départ → arrivée)", "Places gained (grid → finish)"), gained),
            board("fastest", L("Meilleurs tours en course", "Fastest laps"), fastest),
            board("dnfs", L("Abandons", "Retirements"), dnfs),
        ].compactMap { $0 }
        if !average.isEmpty {
            boards.insert(StatBoard(id: "average", title: L("Position moyenne à l'arrivée", "Average finish"), rows: Array(average)), at: min(3, boards.count))
        }

        let teammateDuels = duels.values
            .filter { $0.race.0 + $0.race.1 + $0.quali.0 + $0.quali.1 >= 4 }
            .compactMap { d -> TeammateDuel? in
                guard let a = drivers[d.a], let b = drivers[d.b] else { return nil }
                return TeammateDuel(id: "\(d.team)-\(d.a)-\(d.b)", team: d.team, a: a, b: b,
                                    race: d.race, quali: d.quali,
                                    points: (totals[d.a] ?? 0, totals[d.b] ?? 0))
            }
            .sorted { ($0.points.0 + $0.points.1) > ($1.points.0 + $1.points.1) }

        return SeasonStats(progression: progression, boards: boards, duels: teammateDuels)
    }
}

// MARK: - Vues

private struct ProgressionChart: View {
    let series: [ProgressSeries]

    var body: some View {
        Chart {
            ForEach(series) { s in
                ForEach(s.points) { p in
                    LineMark(x: .value("Round", p.round), y: .value("Points", p.total))
                        .foregroundStyle(by: .value("Driver", s.name))
                        .lineStyle(StrokeStyle(lineWidth: 2.5, dash: s.dashed ? [5, 3] : []))
                        .interpolationMethod(.monotone)
                }
            }
        }
        .chartForegroundStyleScale(domain: series.map(\.name), range: series.map { Team.color($0.team) })
        .chartLegend(position: .bottom, alignment: .leading)
        .chartXAxisLabel(L("Manche", "Round"))
    }
}

private struct DuelRow: View {
    let duel: TeammateDuel

    var body: some View {
        let color = Team.color(duel.team)
        VStack(alignment: .leading, spacing: 8) {
            HStack {
                Text(duel.a.familyName).font(.subheadline.bold()).lineLimit(1)
                Spacer()
                Text(duel.b.familyName).font(.subheadline.bold()).lineLimit(1)
            }
            bar(L("Course", "Race"), duel.race, color)
            bar(L("Qualifs", "Quali"), duel.quali, color)
            HStack {
                Text(points(duel.points.0)).font(.caption.monospacedDigit().bold())
                Spacer()
                Text(L("points", "points")).font(.caption2).foregroundStyle(.secondary)
                Spacer()
                Text(points(duel.points.1)).font(.caption.monospacedDigit().bold())
            }
        }
        .padding(.vertical, 4)
    }

    private func points(_ value: Double) -> String {
        value.rounded() == value ? String(Int(value)) : String(value)
    }

    private func bar(_ label: String, _ v: (Int, Int), _ color: Color) -> some View {
        let total = max(v.0 + v.1, 1)
        return VStack(spacing: 3) {
            HStack {
                Text("\(v.0)").bold().monospacedDigit()
                Spacer()
                Text(label).foregroundStyle(.secondary)
                Spacer()
                Text("\(v.1)").bold().monospacedDigit()
            }
            .font(.caption)
            GeometryReader { geo in
                HStack(spacing: 2) {
                    Capsule().fill(color)
                        .frame(width: max(4, (geo.size.width - 2) * CGFloat(v.0) / CGFloat(total)))
                    Capsule().fill(color.opacity(0.3))
                }
            }
            .frame(height: 6)
        }
    }
}
