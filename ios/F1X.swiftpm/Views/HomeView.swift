import SwiftUI

struct HomeView: View {
    struct Content {
        var next: Race?
        var last: Race?
        var drivers: [DriverStanding]
        var teams: [ConstructorStanding]
        var season: String
    }

    @State private var state: Loadable<Content> = .loading

    var body: some View {
        LoadableView(state: state, retry: { await load(force: false) }) { data in
            ScrollView(.vertical) {
                VStack(spacing: 16) {
                    if let race = data.next {
                        NextRaceCard(race: race)
                        WeatherCard(race: race, full: false)
                    } else {
                        HeroCard {
                            Text("Saison \(data.season) terminée").font(.title2.bold())
                            Text("Rendez-vous la saison prochaine !").foregroundStyle(.secondary)
                        }
                    }
                    if let last = data.last, let results = last.results, !results.isEmpty {
                        LastRaceCard(race: last, results: results)
                    }
                    if !data.drivers.isEmpty {
                        SectionCard(title: "Pilotes") {
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
                        SectionCard(title: "Écuries") {
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
            .refreshable { await load(force: true) }
        }
        .navigationTitle("F1X")
        .f1Destinations()
        .task { if case .loading = state { await load(force: false) } }
    }

    private func load(force: Bool) async {
        if force { await F1API.shared.clearCache() }
        do {
            async let schedule = F1API.shared.schedule()
            async let last = try? F1API.shared.lastResults()
            async let drivers = try? F1API.shared.driverStandings()
            async let teams = try? F1API.shared.constructorStandings()
            let races = try await schedule
            state = .loaded(Content(
                next: races.first { !$0.isOver() },
                last: await last,
                drivers: await drivers ?? [],
                teams: await teams ?? [],
                season: races.first?.season ?? ""
            ))
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
        .background(Color(uiColor: .secondarySystemBackground), in: RoundedRectangle(cornerRadius: 16))
    }
}

private struct NextRaceCard: View {
    let race: Race

    var body: some View {
        HeroCard {
            Eyebrow(text: "Prochain Grand Prix · Manche \(race.round)")
            Text("\(Flag.country(race.circuit.location.country)) \(race.raceName)")
                .font(.title.weight(.heavy))
                .fixedSize(horizontal: false, vertical: true)
            Text("\(race.circuit.circuitName) — \(race.circuit.location.locality)")
                .foregroundStyle(.secondary)
            if let start = race.start { CountdownView(target: start) }
            SessionsList(race: race)
            NavigationLink(value: race) {
                Text("Voir le Grand Prix")
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
        SectionCard(title: "Dernier résultat") {
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
                Text("Détails de la course").fontWeight(.semibold)
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
        .background(Color(uiColor: .tertiarySystemBackground), in: RoundedRectangle(cornerRadius: 12))
        .contentShape(Rectangle())
        .overlay(alignment: .top) {
            UnevenRoundedRectangle(topLeadingRadius: 12, topTrailingRadius: 12)
                .fill(Team.color(result.constructor.constructorId))
                .frame(height: 4)
        }
    }
}
