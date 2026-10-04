import SwiftUI
import UIKit

/// État de chargement générique pour les écrans alimentés par l'API.
enum Loadable<Value> {
    case loading
    case loaded(Value)
    case failed(String)
}

struct LoadableView<Value, Content: View>: View {
    let state: Loadable<Value>
    let retry: () async -> Void
    @ViewBuilder let content: (Value) -> Content

    var body: some View {
        switch state {
        case .loading:
            ProgressView(L("Chargement…", "Loading…"))
                .frame(maxWidth: .infinity, maxHeight: .infinity)
        case .failed(let message):
            ContentUnavailableView {
                Label(L("Drapeau rouge", "Red flag"), systemImage: "flag.fill")
            } description: {
                Text(message)
            } actions: {
                Button(L("Réessayer", "Retry")) { Task { await retry() } }
                    .buttonStyle(.borderedProminent)
            }
        case .loaded(let value):
            content(value)
        }
    }
}

func loadErrorMessage(_ error: Error) -> String {
    L("Les données F1 sont momentanément indisponibles.", "F1 data is temporarily unavailable.") + "\n\(error.localizedDescription)"
}

/// Compte à rebours jusqu'à `target`, 4 cases de largeur égale (jamais plus large que l'écran).
struct CountdownView: View {
    let target: Date

    var body: some View {
        TimelineView(.periodic(from: .now, by: 1)) { context in
            let s = max(0, Int(target.timeIntervalSince(context.date)))
            HStack(spacing: 8) {
                cell(String(s / 86400), "jours")
                cell(String(format: "%02d", (s % 86400) / 3600), "heures")
                cell(String(format: "%02d", (s % 3600) / 60), "min")
                cell(String(format: "%02d", s % 60), "sec")
            }
        }
    }

    private func cell(_ value: String, _ label: String) -> some View {
        VStack(spacing: 2) {
            Text(value)
                .font(.title2.weight(.heavy).monospacedDigit())
                .lineLimit(1)
                .minimumScaleFactor(0.6)
            Text(label.uppercased())
                .font(.caption2.weight(.semibold))
                .foregroundStyle(.secondary)
                .lineLimit(1)
                .minimumScaleFactor(0.7)
        }
        .frame(maxWidth: .infinity)
        .padding(.vertical, 10)
        .background(Color.tile, in: RoundedRectangle(cornerRadius: 12))
    }
}

/// Programme du week-end.
struct SessionsList: View {
    let race: Race

    var body: some View {
        VStack(spacing: 0) {
            ForEach(Array(race.sessions.enumerated()), id: \.offset) { index, session in
                let isRace = session.name == "Course"
                // ViewThatFits : sur une ligne si possible, sinon empilé verticalement.
                ViewThatFits(in: .horizontal) {
                    HStack {
                        sessionName(session.name, isRace: isRace)
                        Spacer(minLength: 12)
                        sessionDate(session.date, isRace: isRace)
                    }
                    VStack(alignment: .leading, spacing: 2) {
                        sessionName(session.name, isRace: isRace)
                        sessionDate(session.date, isRace: isRace)
                    }
                    .frame(maxWidth: .infinity, alignment: .leading)
                }
                .padding(.vertical, 10)
                if index < race.sessions.count - 1 { Divider() }
            }
        }
    }

    private func sessionName(_ name: String, isRace: Bool) -> some View {
        Text(name)
            .fontWeight(.semibold)
            .foregroundStyle(isRace ? Color.f1Red : Color.primary)
    }

    private func sessionDate(_ date: Date, isRace: Bool) -> some View {
        Text(date.f1DayTime)
            .monospacedDigit()
            .fontWeight(isRace ? .semibold : .regular)
            .foregroundStyle(isRace ? Color.primary : Color.secondary)
    }
}

/// Ligne de classement / résultat : position, liseré couleur écurie, nom, points.
struct StandingRow<Trailing: View>: View {
    let position: String
    let teamId: String?
    let title: Text
    let subtitle: String
    /// Avatar facultatif (photo du pilote).
    var avatar: Driver? = nil
    @ViewBuilder let trailing: () -> Trailing

    var body: some View {
        HStack(spacing: 12) {
            RoundedRectangle(cornerRadius: 2)
                .fill(Team.color(teamId))
                .frame(width: 4)
                .padding(.vertical, 2)
            Text(position)
                .font(.body.weight(.heavy).monospacedDigit())
                .frame(minWidth: 24)
            if let d = avatar {
                Avatar(name: d.fullName, wikipedia: d.url, color: Team.color(teamId), size: 36)
            }
            VStack(alignment: .leading, spacing: 2) {
                title
                    .fixedSize(horizontal: false, vertical: true)
                if !subtitle.isEmpty {
                    Text(subtitle)
                        .font(.footnote)
                        .foregroundStyle(.secondary)
                        .fixedSize(horizontal: false, vertical: true)
                }
            }
            .frame(maxWidth: .infinity, alignment: .leading)
            trailing()
                .layoutPriority(1)
        }
        .padding(.vertical, 4)
    }
}

struct PointsLabel: View {
    let value: String
    var suffix: String = "pts"

    var body: some View {
        HStack(alignment: .firstTextBaseline, spacing: 2) {
            Text(value).font(.body.weight(.heavy).monospacedDigit())
            if !suffix.isEmpty {
                Text(suffix).font(.caption2).foregroundStyle(.secondary)
            }
        }
    }
}

func driverTitle(_ driver: Driver, flag: Bool = false) -> Text {
    let prefix = flag ? "\(Flag.nationality(driver.nationality)) " : ""
    return Text("\(prefix)\(driver.givenName) \(Text(driver.familyName).bold())")
}

/// Carte avec dégradé (couleur écurie ou rouge F1).
struct HeroCard<Content: View>: View {
    var accent: Color = .f1Red
    @ViewBuilder let content: () -> Content

    var body: some View {
        VStack(alignment: .leading, spacing: 12, content: content)
            .padding(16)
            .frame(maxWidth: .infinity, alignment: .leading)
            .background(
                LinearGradient(colors: [accent.opacity(0.3), .clear], startPoint: .topLeading, endPoint: .center),
                in: RoundedRectangle(cornerRadius: 16)
            )
            .background(Color.cardBackground, in: RoundedRectangle(cornerRadius: 16))
            .shadow(color: .cardShadow, radius: 10, y: 3)
    }
}

struct Eyebrow: View {
    let text: String
    var body: some View {
        Text(text.uppercased())
            .font(.caption.weight(.bold))
            .tracking(1)
            .foregroundStyle(.secondary)
    }
}


/// Destinations de navigation communes à tous les onglets.
struct F1Destinations: ViewModifier {
    func body(content: Content) -> some View {
        content
            .navigationDestination(for: Race.self) { RaceDetailView(race: $0).sectionLogo() }
            .navigationDestination(for: Driver.self) { DriverDetailView(driver: $0).sectionLogo() }
            .navigationDestination(for: Constructor.self) { TeamDetailView(team: $0).sectionLogo() }
            .navigationDestination(for: Circuit.self) { CircuitDetailView(circuit: $0).sectionLogo() }
    }
}

extension View {
    func f1Destinations() -> some View { modifier(F1Destinations()) }
}

/// Âge à partir d'une date « 1985-01-07 ».
func age(_ dob: String?) -> Int? {
    guard let dob, let date = ISO8601DateFormatter().date(from: dob + "T00:00:00Z") else { return nil }
    return Calendar.current.dateComponents([.year], from: date, to: .now).year
}

/// « 7 janvier 1985 ».
func longDate(_ iso: String?) -> String {
    guard let iso, let date = ISO8601DateFormatter().date(from: iso + "T12:00:00Z") else { return iso ?? "" }
    return date.formatted(.dateTime.day().month(.wide).year().locale(appLocale))
}
