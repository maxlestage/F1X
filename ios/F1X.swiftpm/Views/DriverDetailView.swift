import SwiftUI

/// Statistiques de carrière calculées à partir de tous les résultats.
struct CareerStats {
    var starts = 0, wins = 0, podiums = 0, poles = 0, fastest = 0, points = 0.0
    var seasons: [String] = []
    var teams: [Constructor] = []

    init(_ races: [Race]) {
        for race in races {
            guard let r = race.results?.first else { continue }
            starts += 1
            if r.position == "1" { wins += 1 }
            if let p = Int(r.position), p <= 3 { podiums += 1 }
            if r.grid == "1" { poles += 1 }
            if r.fastestLap?.rank == "1" { fastest += 1 }
            points += Double(r.points) ?? 0
            if !seasons.contains(race.season) { seasons.append(race.season) }
            if !teams.contains(r.constructor) { teams.append(r.constructor) }
        }
    }
}

struct DriverDetailView: View {
    let driver: Driver

    @State private var info: Driver?
    @State private var career: [Race] = []
    @State private var titles: [String] = []
    @State private var isLoading = true

    private var d: Driver { info ?? driver }
    private var stats: CareerStats { CareerStats(career) }
    private var team: Constructor? { career.last?.results?.first?.constructor }
    /// Pilote en activité (a couru cette saison ou la précédente).
    private var recent: Bool {
        let year = Calendar.current.component(.year, from: .now)
        return career.last.flatMap { Int($0.season) }.map { $0 >= year - 1 } ?? false
    }

    var body: some View {
        List {
            Section {
                HeroCard(accent: Team.color(team?.constructorId)) {
                    WikiPhotoView(wikipedia: d.url)
                    Eyebrow(text: [d.permanentNumber.map { "#\($0)" }, d.code, d.nationality]
                        .compactMap { $0 }.joined(separator: " · "))
                    Text("\(Flag.nationality(d.nationality)) \(d.fullName)")
                        .font(.title.weight(.heavy))
                        .fixedSize(horizontal: false, vertical: true)
                    if let dob = d.dateOfBirth {
                        Text(L("Né le \(longDate(dob))", "Born \(longDate(dob))") + (recent ? (age(dob).map { L(" · \($0) ans", " · \($0) years old") } ?? "") : ""))
                            .foregroundStyle(.secondary)
                    }
                    if !career.isEmpty {
                        StatGrid(items: [
                            (L("Départs", "Starts"), "\(stats.starts)"),
                            (L("Victoires", "Wins"), "\(stats.wins)"),
                            ("Podiums", "\(stats.podiums)"),
                            ("Poles", "\(stats.poles)"),
                            (L("Meilleurs tours", "Fastest laps"), "\(stats.fastest)"),
                            (L("Saisons", "Seasons"), "\(stats.seasons.count)"),
                        ])
                    }
                    if !titles.isEmpty {
                        Text(L("🏆 Champion du monde ×\(titles.count) : ", "🏆 World champion ×\(titles.count): ") + titles.joined(separator: ", "))
                            .font(.subheadline.bold())
                            .foregroundStyle(.yellow)
                    }
                    FavoriteButton(kind: .driver, id: driver.driverId)
                }
                .listRowInsets(EdgeInsets())
                .listRowBackground(Color.clear)
            }

            if recent, let team {
                Section {
                    CarCard(constructorId: team.constructorId, teamName: team.name)
                        .listRowInsets(EdgeInsets())
                        .listRowBackground(Color.clear)
                }
            }

            if !stats.teams.isEmpty {
                Section(L("Écuries", "Teams")) {
                    ForEach(stats.teams, id: \.constructorId) { c in
                        NavigationLink(value: c) {
                            HStack {
                                RoundedRectangle(cornerRadius: 2).fill(Team.color(c.constructorId)).frame(width: 4, height: 22)
                                Text(c.name)
                            }
                        }
                    }
                }
            }

            let season = career.filter { $0.season == career.last?.season }
            if !season.isEmpty {
                Section(L("Saison \(season.first?.season ?? "")", "\(season.first?.season ?? "") season")) {
                    ForEach(season.reversed()) { race in
                        if let r = race.results?.first {
                            NavigationLink(value: race) {
                                StandingRow(
                                    position: r.positionText,
                                    teamId: r.constructor.constructorId,
                                    title: Text("\(Flag.country(race.circuit.location.country)) \(race.raceName)"),
                                    subtitle: [L("Départ P\(r.grid ?? "-")", "Started P\(r.grid ?? "-")"), r.outcome]
                                        .filter { !$0.isEmpty }.joined(separator: " · ")
                                ) {
                                    PointsLabel(value: "+\(r.points)", suffix: "")
                                }
                            }
                        }
                    }
                }
            }

            if let url = d.url.flatMap(URL.init(string:)) {
                Section { Link(L("Wikipédia ↗", "Wikipedia ↗"), destination: url) }
            }
        }
        .listStyle(.insetGrouped)
        .navigationTitle(d.familyName)
        .navigationBarTitleDisplayMode(.inline)
        .overlay { if isLoading { ProgressView() } }
        .task { await load() }
    }

    private func load() async {
        defer { isLoading = false }
        async let i = try? F1API.shared.driver(driver.driverId)
        async let c = try? F1API.shared.career(driverId: driver.driverId)
        async let champs = try? ServerAPI.shared.champions()
        info = await i ?? nil
        career = await c ?? []
        titles = (await champs ?? []).filter { $0.in_progress != true && $0.driver?.driverId == driver.driverId }.map(\.season)
    }
}

struct TeamDetailView: View {
    let team: Constructor

    @State private var info: Constructor?
    @State private var wins = "–"
    @State private var podiums = "–"
    @State private var poles = "–"
    @State private var titles: [String] = []
    @State private var drivers: [DriverStanding] = []
    @State private var standing: ConstructorStanding?

    private var c: Constructor { info ?? team }

    var body: some View {
        List {
            Section {
                HeroCard(accent: Team.color(team.constructorId)) {
                    Eyebrow(text: c.nationality ?? "")
                    Text("\(Flag.nationality(c.nationality)) \(c.name)")
                        .font(.title.weight(.heavy))
                        .fixedSize(horizontal: false, vertical: true)
                    StatGrid(items: [(L("Victoires", "Wins"), wins), ("Podiums", podiums), ("Poles", poles)])
                    if !titles.isEmpty {
                        Text(L("🏆 Champion constructeurs ×\(titles.count) : ", "🏆 Constructors' champion ×\(titles.count): ") + titles.joined(separator: ", "))
                            .font(.subheadline.bold())
                            .foregroundStyle(.yellow)
                    }
                    FavoriteButton(kind: .team, id: team.constructorId)
                }
                .listRowInsets(EdgeInsets())
                .listRowBackground(Color.clear)
            }

            Section {
                CarCard(constructorId: team.constructorId, teamName: c.name)
                    .listRowInsets(EdgeInsets())
                    .listRowBackground(Color.clear)
            }

            if let s = standing {
                Section(L("Saison en cours", "Current season")) {
                    StandingRow(position: s.rank, teamId: team.constructorId, title: Text(c.name).bold(), subtitle: winsLabel(s.wins)) {
                        PointsLabel(value: s.points)
                    }
                    ForEach(drivers) { d in
                        NavigationLink(value: d.driver) {
                            StandingRow(position: d.rank, teamId: team.constructorId, title: driverTitle(d.driver, flag: true),
                                        subtitle: winsLabel(d.wins), avatar: d.driver) { PointsLabel(value: d.points) }
                        }
                    }
                }
            }

            if let url = c.url.flatMap(URL.init(string:)) {
                Section { Link(L("Wikipédia ↗", "Wikipedia ↗"), destination: url) }
            }
        }
        .listStyle(.insetGrouped)
        .navigationTitle(c.name)
        .navigationBarTitleDisplayMode(.inline)
        .task { await load() }
    }

    private func load() async {
        let id = team.constructorId
        async let i = try? F1API.shared.constructor(id)
        async let w = try? F1API.shared.total("constructors/\(id)/results/1.json")
        async let p2 = try? F1API.shared.total("constructors/\(id)/results/2.json")
        async let p3 = try? F1API.shared.total("constructors/\(id)/results/3.json")
        async let pl = try? F1API.shared.total("constructors/\(id)/grid/1/results.json")
        async let champs = try? ServerAPI.shared.champions()
        async let ds = try? F1API.shared.driverStandings()
        async let cs = try? F1API.shared.constructorStandings()
        info = await i ?? nil
        let wv = await w
        wins = wv.map { "\($0)" } ?? "–"
        if let wv, let a = await p2, let b = await p3 { podiums = "\(wv + a + b)" }
        poles = (await pl).map { "\($0)" } ?? "–"
        titles = (await champs ?? []).filter { $0.in_progress != true && $0.constructor?.constructorId == id }.map(\.season)
        drivers = (await ds ?? []).filter { $0.team?.constructorId == id }
        standing = (await cs ?? []).first { $0.constructor.constructorId == id }
    }
}

struct CircuitDetailView: View {
    let circuit: Circuit

    @State private var races: [Race] = []
    @State private var winners: [Race] = []

    var body: some View {
        List {
            Section {
                VStack(alignment: .leading, spacing: 8) {
                    Eyebrow(text: "\(circuit.location.locality), \(circuit.location.country)")
                    Text("\(Flag.country(circuit.location.country)) \(circuit.circuitName)")
                        .font(.title2.weight(.heavy))
                        .fixedSize(horizontal: false, vertical: true)
                    if !races.isEmpty {
                        StatGrid(items: [
                            ("Grands Prix", "\(races.count)"),
                            (L("Premier", "First"), races.first?.season ?? "–"),
                            (L("Dernier", "Latest"), races.last?.season ?? "–"),
                        ])
                    }
                }
            }

            Section { WikiPhotoView(wikipedia: circuit.url, wide: true) }

            Section(L("Tracé", "Layout")) { TrackPanel(circuitId: circuit.circuitId, start3D: true) }

            let kings = Dictionary(grouping: winners.compactMap { $0.results?.first?.driver }, by: \.driverId)
                .map { ($0.value[0], $0.value.count) }
                .sorted { $0.1 > $1.1 }
            if !kings.isEmpty {
                Section(L("Rois du circuit", "Kings of the circuit")) {
                    ForEach(kings.prefix(5), id: \.0.driverId) { k in
                        NavigationLink(value: k.0) {
                            HStack {
                                Avatar(name: k.0.fullName, wikipedia: k.0.url, size: 30)
                                Text(k.0.fullName)
                                Spacer()
                                Text(L("\(k.1) victoire\(k.1 > 1 ? "s" : "")", "\(k.1) win\(k.1 > 1 ? "s" : "")")).foregroundStyle(.secondary)
                            }
                        }
                    }
                }
            }

            if !winners.isEmpty {
                Section(L("Palmarès", "Winners")) {
                    ForEach(winners.reversed()) { race in
                        if let r = race.results?.first {
                            NavigationLink(value: race) {
                                StandingRow(position: race.season, teamId: r.constructor.constructorId,
                                            title: driverTitle(r.driver, flag: true), subtitle: r.constructor.name) { EmptyView() }
                            }
                        }
                    }
                }
            }
        }
        .listStyle(.insetGrouped)
        .navigationTitle(circuit.circuitName)
        .navigationBarTitleDisplayMode(.inline)
        .task {
            async let r = try? F1API.shared.circuitRaces(circuit.circuitId)
            async let w = try? F1API.shared.winners("circuits/\(circuit.circuitId)")
            races = await r ?? []
            winners = await w ?? []
        }
    }
}
