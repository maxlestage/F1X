import SwiftUI

struct DriverStandingsView: View {
    @State private var state: Loadable<[DriverStanding]> = .loading

    var body: some View {
        LoadableView(state: state, retry: load) { standings in
            List(standings) { s in
                NavigationLink(value: s.driver) {
                    StandingRow(
                        position: s.rank,
                        teamId: s.team?.constructorId,
                        title: driverTitle(s.driver, flag: true),
                        subtitle: [s.team?.name ?? "", s.wins == "0" ? "" : "\(s.wins) V"]
                            .filter { !$0.isEmpty }.joined(separator: " · ")
                    ) { PointsLabel(value: s.points) }
                }
            }
            .listStyle(.insetGrouped)
            .refreshable {
                await F1API.shared.clearCache()
                await load()
            }
        }
        .navigationTitle("Pilotes")
        .navigationDestination(for: Driver.self) { DriverDetailView(driver: $0) }
        .navigationDestination(for: Race.self) { RaceDetailView(race: $0) }
        .task { if case .loading = state { await load() } }
    }

    private func load() async {
        do {
            state = .loaded(try await F1API.shared.driverStandings())
        } catch {
            state = .failed(loadErrorMessage(error))
        }
    }
}

struct ConstructorStandingsView: View {
    @State private var state: Loadable<[ConstructorStanding]> = .loading

    var body: some View {
        LoadableView(state: state, retry: load) { standings in
            let leader = standings.first.flatMap { Double($0.points) } ?? 0
            List(standings) { s in
                let pts = Double(s.points) ?? 0
                VStack(alignment: .leading, spacing: 6) {
                    StandingRow(
                        position: s.rank,
                        teamId: s.constructor.constructorId,
                        title: Text(s.constructor.name).bold(),
                        subtitle: winsLabel(s.wins)
                    ) { PointsLabel(value: s.points) }
                    ProgressView(value: leader > 0 ? min(pts / leader, 1) : 0)
                        .tint(Team.color(s.constructor.constructorId))
                }
            }
            .listStyle(.insetGrouped)
            .refreshable {
                await F1API.shared.clearCache()
                await load()
            }
        }
        .navigationTitle("Écuries")
        .task { if case .loading = state { await load() } }
    }

    private func load() async {
        do {
            state = .loaded(try await F1API.shared.constructorStandings())
        } catch {
            state = .failed(loadErrorMessage(error))
        }
    }
}
