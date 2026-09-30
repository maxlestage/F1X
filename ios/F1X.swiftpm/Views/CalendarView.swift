import SwiftUI

struct CalendarView: View {
    @State private var state: Loadable<[Race]> = .loading

    var body: some View {
        LoadableView(state: state, retry: load) { races in
            let nextId = races.first { !$0.isOver() }?.id
            List(races) { race in
                NavigationLink(value: race) {
                    CalendarRow(race: race, isNext: race.id == nextId)
                }
                .listRowBackground(race.id == nextId ? Color.f1Red.opacity(0.18) : nil)
            }
            .listStyle(.insetGrouped)
            .refreshable {
                await F1API.shared.clearCache()
                await load()
            }
        }
        .navigationTitle("Calendrier")
        .navigationDestination(for: Race.self) { RaceDetailView(race: $0) }
        .navigationDestination(for: Driver.self) { DriverDetailView(driver: $0) }
        .task { if case .loading = state { await load() } }
    }

    private func load() async {
        do {
            state = .loaded(try await F1API.shared.schedule())
        } catch {
            state = .failed(loadErrorMessage(error))
        }
    }
}

private struct CalendarRow: View {
    let race: Race
    let isNext: Bool

    var body: some View {
        HStack(spacing: 12) {
            Text("R\(race.round)")
                .font(.subheadline.weight(.heavy))
                .foregroundStyle(.secondary)
                .frame(width: 44, height: 44)
                .background(Color(uiColor: .tertiarySystemBackground), in: RoundedRectangle(cornerRadius: 10))
            VStack(alignment: .leading, spacing: 2) {
                Text("\(Flag.country(race.circuit.location.country)) \(race.raceName)")
                    .fontWeight(.bold)
                    .fixedSize(horizontal: false, vertical: true)
                HStack(spacing: 6) {
                    if let start = race.start { Text(start.f1Day) }
                    if race.isSprintWeekend {
                        Text("SPRINT")
                            .font(.caption2.bold())
                            .padding(.horizontal, 6)
                            .padding(.vertical, 1)
                            .background(Color.f1Red.opacity(0.2), in: RoundedRectangle(cornerRadius: 6))
                            .foregroundStyle(Color.f1Red)
                    }
                }
                .font(.footnote)
                .foregroundStyle(.secondary)
            }
            .frame(maxWidth: .infinity, alignment: .leading)
            if isNext {
                Text("Prochain")
                    .font(.caption2.bold())
                    .padding(.horizontal, 8)
                    .padding(.vertical, 4)
                    .background(Color.f1Red, in: Capsule())
                    .foregroundStyle(.white)
            }
        }
        .opacity(race.isOver() ? 0.6 : 1)
    }
}
