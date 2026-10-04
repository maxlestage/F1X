import SwiftUI

struct HomeView: View {
    struct Content {
        var next: Race?
        var last: Race?
        var drivers: [DriverStanding]
        var teams: [ConstructorStanding]
        var season: String
        var races: [Race] = []
    }

    @State private var state: Loadable<Content> = .loading

    var body: some View {
        LoadableView(state: state, retry: { await load(force: false) }) { data in
            ScrollView(.vertical) {
                VStack(spacing: 16) {
                    if let race = data.next {
                        NextRaceCard(race: race)
                        ReminderToggle(races: data.races)
                        WeekendActivityToggle(races: data.races, last: data.last)
                        WeatherCard(race: race, full: false)
                    } else {
                        HeroCard {
                            Text(L("Saison \(data.season) terminée", "\(data.season) season over")).font(.title2.bold())
                            Text(L("Rendez-vous la saison prochaine !", "See you next season!")).foregroundStyle(.secondary)
                        }
                    }
                    if let last = data.last, let results = last.results, !results.isEmpty {
                        LastRaceCard(race: last, results: results)
                    }
                    if !data.drivers.isEmpty {
                        FavoritesCard(drivers: data.drivers, teams: data.teams, last: data.last)
                    }
                    NavigationLink { SeasonStatsView() } label: {
                        Label(L("Statistiques de la saison : duels, records, évolution", "Season stats: battles, records, progression"),
                              systemImage: "chart.line.uptrend.xyaxis")
                            .font(.subheadline.weight(.semibold))
                            .frame(maxWidth: .infinity, alignment: .leading)
                            .padding(14)
                            .background(Color.cardBackground, in: RoundedRectangle(cornerRadius: 16))
        .shadow(color: .cardShadow, radius: 10, y: 3)
                    }
                    .buttonStyle(.plain)
                    if !data.drivers.isEmpty {
                        SectionCard(title: L("Pilotes", "Drivers")) {
                            ForEach(data.drivers.prefix(5)) { s in
                                NavigationLink(value: s.driver) {
                                    StandingRow(
                                        position: s.rank,
                                        teamId: s.team?.constructorId,
                                        title: driverTitle(s.driver, flag: true),
                                        subtitle: s.team?.name ?? "",
                                        avatar: s.driver
                                    ) { PointsLabel(value: s.points) }
                                }
                                .buttonStyle(.plain)
                            }
                        }
                    }
                    if !data.teams.isEmpty {
                        SectionCard(title: L("Écuries", "Teams")) {
                            ForEach(data.teams.prefix(3)) { s in
                                NavigationLink(value: s.constructor) {
                                    StandingRow(
                                        position: s.rank,
                                        teamId: s.constructor.constructorId,
                                        title: Text(s.constructor.name),
                                        subtitle: winsLabel(s.wins)
                                    ) { PointsLabel(value: s.points) }
                                }
                                .buttonStyle(.plain)
                            }
                        }
                    }
                }
                .padding(16)
            }
            .background(Color.pageBackground)
            .refreshable { await load(force: true) }
        }
        .navigationTitle("F1X")
        .f1Destinations()
        .task { if case .loading = state { await F1API.instant { await load(force: false) } } }
        .autoRefresh { await load(force: false) }
    }

    private func load(force: Bool) async {
        if force { await F1API.shared.clearCache() }
        do {
            async let schedule = F1API.shared.schedule()
            async let last = try? F1API.shared.lastResults()
            async let drivers = try? F1API.shared.driverStandings()
            async let teams = try? F1API.shared.constructorStandings()
            let races = try await schedule
            let content = Content(
                next: races.first { !$0.isOver() },
                last: await last,
                drivers: await drivers ?? [],
                teams: await teams ?? [],
                season: races.first?.season ?? "",
                races: races
            )
            state = .loaded(content)
            await SessionReminders.schedule(races: races)
            await WeekendActivity.sync(races: races, last: content.last)
        } catch {
            state = .failed(loadErrorMessage(error))
        }
    }
}

func winsLabel(_ wins: String) -> String {
    switch wins {
    case "0": ""
    case "1": L("1 victoire", "1 win")
    default: L("\(wins) victoires", "\(wins) wins")
    }
}

struct SectionCard<Content: View>: View {
    let title: String
    @ViewBuilder let content: () -> Content

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            Text(title).font(.title3.bold())
            content()
        }
        .padding(16)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(Color.cardBackground, in: RoundedRectangle(cornerRadius: 16))
        .shadow(color: .cardShadow, radius: 10, y: 3)
    }
}

private struct NextRaceCard: View {
    let race: Race

    var body: some View {
        HeroCard {
            Eyebrow(text: L("Prochain Grand Prix · Manche \(race.round)", "Next Grand Prix · Round \(race.round)"))
            Text("\(Flag.country(race.circuit.location.country)) \(race.raceName)")
                .font(.title.weight(.heavy))
                .fixedSize(horizontal: false, vertical: true)
            Text("\(race.circuit.circuitName) — \(race.circuit.location.locality)")
                .foregroundStyle(.secondary)
            TrackOutline(circuitId: race.circuit.circuitId)
            if let start = race.start { CountdownView(target: start) }
            SessionsList(race: race)
            NavigationLink(value: race) {
                Text(L("Voir le Grand Prix", "Open the Grand Prix"))
                    .fontWeight(.bold)
                    .frame(maxWidth: .infinity)
                    .padding(.vertical, 12)
                    .background(Color.f1Red, in: RoundedRectangle(cornerRadius: 12))
                    .foregroundStyle(.white)
            }
        }
    }
}

private struct LastRaceCard: View {
    let race: Race
    let results: [RaceResult]

    var body: some View {
        SectionCard(title: L("Dernier résultat", "Latest result")) {
            Text("\(Flag.country(race.circuit.location.country)) \(race.raceName)")
                .foregroundStyle(.secondary)
            // Podium : 3 colonnes de largeur égale, 2 – 1 – 3.
            let podium = Array(results.prefix(3))
            HStack(alignment: .bottom, spacing: 8) {
                ForEach([1, 0, 2].filter { $0 < podium.count }, id: \.self) { i in
                    PodiumStep(result: podium[i], tall: i == 0)
                }
            }
            NavigationLink(value: race) {
                Text(L("Détails de la course", "Race details")).fontWeight(.semibold)
            }
        }
    }
}

private struct PodiumStep: View {
    let result: RaceResult
    let tall: Bool

    var body: some View {
        VStack(spacing: 2) {
            Avatar(name: result.driver.fullName, wikipedia: result.driver.url,
                   color: Team.color(result.constructor.constructorId), size: tall ? 56 : 46)
            Text(result.position).font(.title2.weight(.black)).italic()
            Text(result.driver.familyName)
                .font(.subheadline.bold())
                .lineLimit(1)
                .minimumScaleFactor(0.6)
            Text(result.constructor.name)
                .font(.caption2)
                .foregroundStyle(.secondary)
                .lineLimit(2)
                .multilineTextAlignment(.center)
        }
        .frame(maxWidth: .infinity)
        .padding(.vertical, tall ? 20 : 12)
        .padding(.horizontal, 4)
        .background(Color.tile, in: RoundedRectangle(cornerRadius: 12))
        .contentShape(Rectangle())
        .overlay(alignment: .top) {
            UnevenRoundedRectangle(topLeadingRadius: 12, topTrailingRadius: 12)
                .fill(Team.color(result.constructor.constructorId))
                .frame(height: 4)
        }
    }
}

/// Pilote et écurie favoris : classement, dernier résultat, choix rapide.
private struct FavoritesCard: View {
    let drivers: [DriverStanding]
    let teams: [ConstructorStanding]
    let last: Race?
    @AppStorage("f1x-fav-driver") private var driverId = ""
    @AppStorage("f1x-fav-team") private var teamId = ""

    var body: some View {
        SectionCard(title: L("Mes favoris", "My favourites")) {
            if let s = drivers.first(where: { $0.driver.driverId == driverId }) {
                NavigationLink(value: s.driver) {
                    StandingRow(position: s.rank, teamId: s.team?.constructorId,
                                title: driverTitle(s.driver, flag: true),
                                subtitle: lastLine(s.driver.driverId), avatar: s.driver) { PointsLabel(value: s.points) }
                }
                .buttonStyle(.plain)
            }
            if let t = teams.first(where: { $0.constructor.constructorId == teamId }) {
                NavigationLink(value: t.constructor) {
                    StandingRow(position: t.rank, teamId: t.constructor.constructorId,
                                title: Text(t.constructor.name).bold(), subtitle: winsLabel(t.wins)) { PointsLabel(value: t.points) }
                }
                .buttonStyle(.plain)
            }
            Menu {
                Picker(L("Pilote", "Driver"), selection: $driverId) {
                    Text(L("Aucun", "None")).tag("")
                    ForEach(drivers) { Text($0.driver.fullName).tag($0.driver.driverId) }
                }
                .pickerStyle(.menu)
                Picker(L("Écurie", "Team"), selection: $teamId) {
                    Text(L("Aucune", "None")).tag("")
                    ForEach(teams) { Text($0.constructor.name).tag($0.constructor.constructorId) }
                }
                .pickerStyle(.menu)
            } label: {
                Label(driverId.isEmpty && teamId.isEmpty ? L("Choisir mon pilote et mon écurie", "Pick my driver and team")
                                                         : L("Changer", "Change"),
                      systemImage: "star")
                    .font(.subheadline.weight(.semibold))
            }
        }
    }

    private func lastLine(_ id: String) -> String {
        guard let last, let r = last.results?.first(where: { $0.driver.driverId == id }) else { return "" }
        let place = Int(r.positionText).map { "P\($0)" } ?? L("abandon", "DNF")
        return L("Dernière course : \(place) · +\(r.points) pts", "Last race: \(place) · +\(r.points) pts")
    }
}
