import SwiftUI

struct CalendarView: View {
    @State private var season = "current"
    @State private var state: Loadable<[Race]> = .loading
    @State private var winners: [String: RaceResult] = [:]
    /// Course sur laquelle ouvrir la liste (la dernière disputée).
    @State private var scrollTarget: String?

    var body: some View {
        ScrollViewReader { proxy in
        List {
            Section {
                SeasonPicker(season: $season)
                // Abonnement : toutes les séances dans l'app Calendrier, mises à jour toutes seules.
                if let url = URL(string: "webcal://\(Server.base.host ?? "")/calendar.ics") {
                    Link(destination: url) {
                        Label(L("Ajouter les séances à mon calendrier", "Add sessions to my calendar"), systemImage: "calendar.badge.plus")
                    }
                }
            }
            switch state {
            case .loading:
                Section { StartLightsLoader() }
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
                            // Lueur rouge qui dérive et ligne de vitesse, comme la carte de l'accueil.
                            .listRowBackground(ZStack { Color.f1Red.opacity(0.18); HeroGlow() }.clipped())
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
                        .id(race.id)
                        .listRowBackground(race.id == nextId ? Color.f1Red.opacity(0.18) : past ? Color.primary.opacity(0.035) : nil)
                    }
                }
            }
        }
        .listStyle(.insetGrouped)
        // Ouvre directement sur la dernière course disputée (la prochaine juste en dessous).
        .onChange(of: scrollTarget) { _, target in
            guard let target else { return }
            DispatchQueue.main.async { proxy.scrollTo(target, anchor: .top) }
        }
        }
        .navigationTitle(L("Calendrier", "Calendar"))
        .f1Destinations()
        .refreshable {
            await F1API.shared.clearCache()
            await load()
        }
        .task(id: season) { await F1API.instant { await load() } }
        .autoRefresh { await load() }
    }

    private func load() async {
        do {
            async let w = try? F1API.shared.winners(season)
            let races = try await F1API.shared.schedule(season: season)
            state = .loaded(races)
            scrollTarget = (races.last { $0.isOver() } ?? races.first { !$0.isOver() })?.id
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

    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @State private var flipped = false

    var body: some View {
        HStack(spacing: 10) {
            // Le numéro de manche se retourne à l'arrivée ; la prochaine course respire.
            Text("R\(race.round)")
                .font(.caption.weight(.heavy).monospacedDigit())
                .foregroundStyle(isNext ? Color.white : Color.secondary)
                .lineLimit(1)
                .minimumScaleFactor(0.6)
                .frame(width: 38, height: 34)
                .background(isNext ? Color.f1Red : Color.tile, in: RoundedRectangle(cornerRadius: 8))
                .glow(.f1Red, active: isNext)
                .rotation3DEffect(.degrees(flipped ? 0 : -90), axis: (x: 0, y: 1, z: 0))
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
                    .scaleEffect(flipped ? 1 : 0.1)
                    .accessibilityLabel(L("Terminé", "Done"))
            }
        }
        .opacity(isPast ? 0.62 : race.isOver() && winner == nil ? 0.6 : 1)
        .cascadeIn()
        .onAppear {
            guard !flipped else { return }
            if reduceMotion { flipped = true; return }
            withAnimation(.spring(response: 0.6, dampingFraction: 0.7).delay(0.12)) { flipped = true }
        }
    }
}
