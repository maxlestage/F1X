import SwiftUI

struct HomeView: View {
    struct Content {
        var next: Race?
        var last: Race?
        var drivers: [DriverStanding]
        var teams: [ConstructorStanding]
        var season: String
        var races: [Race] = []
        /// Qualifications et qualifs sprint du prochain Grand Prix (vides avant les séances).
        var quali: [QualifyingResult] = []
        var sprintQuali: [QualifyingResult] = []
    }

    @State private var state: Loadable<Content> = .loading

    var body: some View {
        LoadableView(state: state, retry: { await load(force: false) }) { data in
            ScrollView(.vertical) {
                VStack(spacing: 16) {
                    if let race = data.next {
                        NextRaceCard(race: race)
                        // Ordre des qualifications dès la fin de la séance (qualifs, qualifs sprint).
                        QualiOrderCard(race: race, main: data.quali, sprint: data.sprintQuali)
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
                    .cascadeUp()
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
            let next = races.first { !$0.isOver() }
            // Ordre des qualifications : cherché seulement pendant le week-end du Grand Prix.
            var quali: (main: [QualifyingResult], sprint: [QualifyingResult])?
            if let next, let start = next.start, Date.now > start.addingTimeInterval(-4 * 86400) {
                quali = try? await F1API.shared.qualifyingWeekend(season: next.season, round: next.roundNumber)
            }
            let content = Content(
                next: next,
                last: await last,
                drivers: await drivers ?? [],
                teams: await teams ?? [],
                season: races.first?.season ?? "",
                races: races,
                quali: quali?.main ?? [],
                sprintQuali: quali?.sprint ?? []
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
            Text(title).font(.archivo(23, weight: 750, width: 88))
                .rise(delay: 0.1)
            content()
        }
        .padding(16)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(Color.cardBackground, in: RoundedRectangle(cornerRadius: 18))
        .overlay(RoundedRectangle(cornerRadius: 18).strokeBorder(Color.hairline))
        .shadow(color: .cardShadow, radius: 10, y: 3)
        .cascadeUp()
    }
}

private struct NextRaceCard: View {
    let race: Race

    var body: some View {
        HeroCard {
            Eyebrow(text: L("Prochain Grand Prix · Manche \(race.round)", "Next Grand Prix · Round \(race.round)"))
            Text("\(Flag.country(race.circuit.location.country)) \(race.raceName)")
                .font(.archivo(38, weight: 800, width: 84))
                .fixedSize(horizontal: false, vertical: true)
                .rise(delay: 0.05)
            Text("\(race.circuit.circuitName) — \(race.circuit.location.locality)")
                .foregroundStyle(.secondary)
                .rise(delay: 0.15)
            TrackOutline(circuitId: race.circuit.circuitId)
            if let start = race.start { CountdownView(target: start) }
            SessionsList(race: race)
            NavigationLink(value: race) {
                Text(L("Voir le Grand Prix", "Open the Grand Prix"))
                    .fontWeight(.bold)
                    .frame(maxWidth: .infinity)
                    .padding(.vertical, 14)
                    .background(Color.f1Red, in: Capsule())
                    // Un reflet passe sur le bouton de temps en temps.
                    .overlay { SweepShine().clipShape(Capsule()) }
                    .foregroundStyle(.white)
            }
            .buttonStyle(PressableStyle())
        }
    }
}

/// Ordre des qualifications du prochain Grand Prix : qualifs (grille du Grand Prix) et
/// qualifs sprint. N'apparaît qu'une fois une séance de qualifications terminée.
/// Les données viennent du chargement de l'accueil : une carte vide n'est jamais affichée,
/// donc elle ne pourrait pas se charger elle-même.
private struct QualiOrderCard: View {
    let race: Race
    let main: [QualifyingResult]
    let sprint: [QualifyingResult]

    /// Onglet choisi (sinon : les qualifs si elles ont eu lieu, sinon les qualifs sprint).
    @State private var showSprint: Bool?
    @State private var all = false

    var body: some View {
        let sprintShown = showSprint ?? main.isEmpty
        let list = sprintShown ? sprint : main
        if !list.isEmpty {
            SectionCard(title: L("Ordre des qualifications", "Qualifying order")) {
                if !main.isEmpty && !sprint.isEmpty {
                    Picker("", selection: Binding(get: { sprintShown }, set: { value in withAnimation(.snappy) { showSprint = value } })) {
                        Text(L("Qualifications", "Qualifying")).tag(false)
                        Text(L("Qualifs sprint", "Sprint quali")).tag(true)
                    }
                    .pickerStyle(.segmented)
                } else {
                    Eyebrow(text: sprintShown ? L("Qualifs sprint", "Sprint qualifying") : L("Qualifications", "Qualifying"))
                }
                ForEach(Array(list.prefix(all ? list.count : 10).enumerated()), id: \.offset) { _, q in
                    NavigationLink(value: q.driver) { QualifyingRow(result: q, sprint: sprintShown) }
                        .buttonStyle(.plain)
                }
                HStack {
                    if list.count > 10 {
                        Button(all ? L("Voir le top 10", "Show top 10") : L("Voir les \(list.count) pilotes", "See all \(list.count) drivers")) {
                            withAnimation(.snappy) { all.toggle() }
                        }
                        .font(.subheadline.weight(.semibold))
                    }
                    Spacer()
                    NavigationLink(value: race) {
                        Text(L("Détails", "Details")).font(.subheadline.weight(.semibold))
                    }
                }
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
                // Les marches montent, le vainqueur en dernier.
                ForEach([1, 0, 2].filter { $0 < podium.count }, id: \.self) { i in
                    PodiumStep(result: podium[i], tall: i == 0, delay: [0.35, 0, 0.12][i])
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
    var delay: Double = 0

    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @State private var up = false

    var body: some View {
        VStack(spacing: 2) {
            Avatar(name: result.driver.fullName, wikipedia: result.driver.url,
                   color: Team.color(result.constructor.constructorId), size: tall ? 56 : 46)
            Text(result.position).font(.title2.weight(.black)).italic()
                // Le vainqueur en or.
                .foregroundStyle(tall ? Color(hex: 0xFACC15) : Color.primary)
                .shadow(color: tall ? Color(hex: 0xFACC15).opacity(0.5) : .clear, radius: 8)
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
        // Reflet doré qui passe sur la marche du vainqueur.
        .overlay {
            if tall {
                SweepShine(color: Color(hex: 0xFFD666).opacity(0.28), period: 4, active: 0.4)
                    .clipShape(RoundedRectangle(cornerRadius: 12))
            }
        }
        .opacity(up ? 1 : 0)
        .offset(y: up ? 0 : 40)
        .onAppear {
            guard !up else { return }
            if reduceMotion { up = true; return }
            withAnimation(.spring(response: 0.6, dampingFraction: 0.78).delay(delay)) { up = true }
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
