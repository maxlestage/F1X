import SwiftUI

struct DriverDetailView: View {
    let driver: Driver

    @State private var standing: DriverStanding?
    @State private var races: [Race] = []
    @State private var isLoading = true

    private var team: Constructor? {
        standing?.team ?? races.last?.results?.first?.constructor
    }

    var body: some View {
        List {
            Section {
                HeroCard(accent: Team.color(team?.constructorId)) {
                    Eyebrow(text: [driver.permanentNumber.map { "#\($0)" }, team?.name]
                        .compactMap { $0 }.joined(separator: " · "))
                    Text("\(Flag.nationality(driver.nationality)) \(driver.fullName)")
                        .font(.title.weight(.heavy))
                        .fixedSize(horizontal: false, vertical: true)
                    if let s = standing {
                        HStack(spacing: 8) {
                            stat("Position", s.rank)
                            stat("Points", s.points)
                            stat("Victoires", s.wins)
                        }
                    }
                }
                .listRowInsets(EdgeInsets())
                .listRowBackground(Color.clear)
            }

            if !races.isEmpty {
                Section("Saison") {
                    ForEach(races.reversed()) { race in
                        if let r = race.results?.first {
                            NavigationLink(value: race) {
                                StandingRow(
                                    position: r.positionText,
                                    teamId: r.constructor.constructorId,
                                    title: Text("\(Flag.country(race.circuit.location.country)) \(race.raceName)"),
                                    subtitle: ["Départ P\(r.grid ?? "-")", r.outcome]
                                        .filter { !$0.isEmpty }.joined(separator: " · ")
                                ) {
                                    PointsLabel(value: "+\(r.points)", suffix: "")
                                }
                            }
                        }
                    }
                }
            }
        }
        .listStyle(.insetGrouped)
        .navigationTitle(driver.familyName)
        .navigationBarTitleDisplayMode(.inline)
        .overlay { if isLoading { ProgressView() } }
        .task { await load() }
    }

    private func stat(_ label: String, _ value: String) -> some View {
        VStack(spacing: 2) {
            Text(label.uppercased())
                .font(.caption2.weight(.semibold))
                .foregroundStyle(.secondary)
                .lineLimit(1)
                .minimumScaleFactor(0.7)
            Text(value).font(.title2.weight(.heavy).monospacedDigit())
        }
        .frame(maxWidth: .infinity)
        .padding(.vertical, 10)
        .background(.black.opacity(0.35), in: RoundedRectangle(cornerRadius: 12))
    }

    private func load() async {
        defer { isLoading = false }
        async let standings = try? F1API.shared.driverStandings()
        async let results = try? F1API.shared.driverResults(driverId: driver.driverId)
        standing = (await standings ?? []).first { $0.driver.driverId == driver.driverId }
        races = await results ?? []
    }
}
