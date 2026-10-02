import SwiftUI

struct CalendarView: View {
    @State private var season = "current"
    @State private var state: Loadable<[Race]> = .loading
    @State private var winners: [String: RaceResult] = [:]

    var body: some View {
        List {
            Section { SeasonPicker(season: $season) }
            switch state {
            case .loading:
                Section { ProgressView().frame(maxWidth: .infinity) }
            case .failed(let message):
                Section {
                    Text(message).foregroundStyle(.secondary)
                    Button(L("Réessayer", "Retry")) { Task { await load() } }
                }
            case .loaded(let races):
                let next = races.first { !$0.isOver() }
                let nextId = next?.id
                // La course qui arrive, avec sa date et le compte à rebours, tout en haut.
                if let next {
                    Section {
                        NavigationLink(value: next) { NextRaceHeader(race: next) }
                            .listRowBackground(Color.f1Red.opacity(0.18))
                    } header: {
                        Text(L("Prochaine course", "Next race"))
                    }
                }
                Section {
                    ForEach(races) { race in
                        // Saison en cours : les Grands Prix déjà courus passent en sombre.
                        let past = season == "current" && race.isOver()
                        NavigationLink(value: race) {
                            CalendarRow(race: race, isNext: race.id == nextId, isPast: past, winner: winners[race.round])
                        }
                        .listRowBackground(race.id == nextId ? Color.f1Red.opacity(0.18) : past ? Color(hex: 0x0A0A0F) : nil)
                    }
                }
            }
        }
        .listStyle(.insetGrouped)
        .navigationTitle(L("Calendrier", "Calendar"))
        .f1Destinations()
        .refreshable {
            await F1API.shared.clearCache()
            await load()
        }
        .task(id: season) { await load() }
    }

    private func load() async {
        do {
            async let w = try? F1API.shared.winners(season)
            let races = try await F1API.shared.schedule(season: season)
            state = .loaded(races)
            var map: [String: RaceResult] = [:]
            for r in await w ?? [] { if let first = r.results?.first { map[r.round] = first } }
            winners = map
        } catch {
            state = .failed(loadErrorMessage(error))
        }
    }
}

/// En-tête : prochain Grand Prix, date et heure de la course, compte à rebours.
private struct NextRaceHeader: View {
    let race: Race

    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            Text("\(Flag.country(race.circuit.location.country)) \(race.raceName)")
                .font(.headline.weight(.heavy))
                .lineLimit(1).minimumScaleFactor(0.7)
            HStack(spacing: 6) {
                Image(systemName: "calendar")
                Text(race.start?.f1DayTime ?? race.date)
                if race.isSprintWeekend { SprintTag() }
            }
            .font(.subheadline.weight(.semibold))
            .lineLimit(1).minimumScaleFactor(0.7)
            Text("\(race.circuit.circuitName) · \(race.circuit.location.locality)")
                .font(.footnote).foregroundStyle(.secondary)
                .lineLimit(1).minimumScaleFactor(0.7)
            if let start = race.start { CountdownView(target: start) }
        }
        .padding(.vertical, 4)
    }
}

private struct SprintTag: View {
    var body: some View {
        Text("SPRINT")
            .font(.caption2.bold())
            .padding(.horizontal, 5)
            .padding(.vertical, 1)
            .background(Color.f1Red.opacity(0.2), in: RoundedRectangle(cornerRadius: 5))
            .foregroundStyle(Color.f1Red)
            .fixedSize()
    }
}

/// Une ligne par Grand Prix : nom sur une ligne, date (et sprint) puis vainqueur sur une autre.
private struct CalendarRow: View {
    let race: Race
    let isNext: Bool
    let isPast: Bool
    let winner: RaceResult?

    var body: some View {
        HStack(spacing: 10) {
            Text("R\(race.round)")
                .font(.caption.weight(.heavy).monospacedDigit())
                .foregroundStyle(.secondary)
                .frame(width: 34, height: 34)
                .background(Color(uiColor: .tertiarySystemBackground), in: RoundedRectangle(cornerRadius: 8))
            VStack(alignment: .leading, spacing: 2) {
                Text("\(Flag.country(race.circuit.location.country)) \(race.raceName)")
                    .fontWeight(.bold)
                    .lineLimit(1).minimumScaleFactor(0.65)
                HStack(spacing: 5) {
                    if let start = race.start { Text(start.f1Day) }
                    if race.isSprintWeekend { SprintTag() }
                    if let w = winner {
                        Text("· 🏆 \(w.driver.givenName.prefix(1)). \(w.driver.familyName)")
                    } else if isNext {
                        Text(L("· Prochain", "· Next")).foregroundStyle(Color.f1Red).bold()
                    }
                }
                .font(.footnote)
                .foregroundStyle(.secondary)
                .lineLimit(1).minimumScaleFactor(0.65)
            }
            .frame(maxWidth: .infinity, alignment: .leading)
            if isPast {
                Image(systemName: "checkmark.circle.fill")
                    .foregroundStyle(Color(hex: 0x6C6C80))
                    .accessibilityLabel(L("Terminé", "Done"))
            }
        }
        .opacity(isPast ? 0.62 : race.isOver() && winner == nil ? 0.6 : 1)
    }
}
