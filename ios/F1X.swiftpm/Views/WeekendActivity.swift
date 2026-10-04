import ActivityKit
import SwiftUI

/// Démarre et tient à jour la Live Activity du week-end (prochaine séance + minuteur).
@MainActor
enum WeekendActivity {
    static let key = "weekendActivity"
    /// Activé par défaut ; désactivable depuis l'accueil.
    static var enabled: Bool { UserDefaults.standard.object(forKey: key) as? Bool ?? true }

    static var running: Bool { !Activity<WeekendActivityAttributes>.activities.isEmpty }

    /// Durée approximative d'une séance (une course dure ~2 h).
    private static func duration(_ name: String) -> TimeInterval {
        name == "Course" ? 2 * 3600 : 3600
    }

    private static func short(_ name: String) -> String {
        switch name {
        case "Essais libres 1": return isFrench ? "EL1" : "FP1"
        case "Essais libres 2": return isFrench ? "EL2" : "FP2"
        case "Essais libres 3": return isFrench ? "EL3" : "FP3"
        case "Qualifs sprint": return "SQ"
        case "Sprint": return "S"
        case "Qualifications": return "Q"
        default: return "GP"
        }
    }

    /// Appelée à chaque chargement du calendrier. `force` : démarrage demandé par l'utilisateur,
    /// même si le week-end n'a pas encore commencé.
    static func sync(races: [Race], force: Bool = false) async {
        guard ActivityAuthorizationInfo().areActivitiesEnabled else { return }
        guard enabled || force else { await endAll(); return }
        let now = Date()
        guard let race = races.first(where: { !$0.isOver(now: now) }) else { await endAll(); return }
        let sessions = race.sessions
        guard let index = sessions.firstIndex(where: { $0.date.addingTimeInterval(duration($0.name)) > now }) else {
            await endAll()
            return
        }
        let current = sessions[index]
        // Démarrage automatique dans les 24 h qui précèdent la séance (week-end en cours).
        let soon = current.date.timeIntervalSince(now) < 24 * 3600
        let following = sessions.indices.contains(index + 1) ? sessions[index + 1] : nil
        let state = WeekendActivityAttributes.ContentState(
            session: sessionLabel(current.name),
            short: short(current.name),
            start: current.date,
            next: following.map {
                "\(sessionLabel($0.name)) · \($0.date.formatted(.dateTime.weekday(.abbreviated).hour().minute()))"
            })
        let content = ActivityContent(state: state, staleDate: current.date.addingTimeInterval(duration(current.name)))

        let existing = Activity<WeekendActivityAttributes>.activities
        if let activity = existing.first(where: { $0.attributes.round == race.round && $0.attributes.raceName == race.raceName }) {
            if activity.content.state != state { await activity.update(content) }
            for other in existing where other.id != activity.id { await other.end(nil, dismissalPolicy: .immediate) }
            return
        }
        for other in existing { await other.end(nil, dismissalPolicy: .immediate) }
        guard soon || force else { return }
        let attributes = WeekendActivityAttributes(
            raceName: race.raceName,
            flag: Flag.country(race.circuit.location.country),
            round: race.round,
            circuit: race.circuit.location.locality)
        _ = try? Activity.request(attributes: attributes, content: content)
    }

    static func endAll() async {
        for activity in Activity<WeekendActivityAttributes>.activities {
            await activity.end(nil, dismissalPolicy: .immediate)
        }
    }
}

/// Bouton « Suivre le week-end » (écran verrouillé + Dynamic Island).
struct WeekendActivityToggle: View {
    let races: [Race]
    @AppStorage(WeekendActivity.key) private var enabled = true
    @State private var running = false

    var body: some View {
        if ActivityAuthorizationInfo().areActivitiesEnabled {
            Button {
                Task {
                    if running {
                        enabled = false
                        await WeekendActivity.endAll()
                    } else {
                        enabled = true
                        await WeekendActivity.sync(races: races, force: true)
                    }
                    running = WeekendActivity.running
                }
            } label: {
                Label(running ? L("Week-end suivi sur l'écran verrouillé et la Dynamic Island", "Weekend on Lock Screen & Dynamic Island")
                              : L("Suivre le week-end (écran verrouillé, Dynamic Island)", "Follow the weekend (Lock Screen, Dynamic Island)"),
                      systemImage: running ? "lock.iphone" : "platter.filled.top.iphone")
                    .font(.subheadline.weight(.semibold))
                    .frame(maxWidth: .infinity)
            }
            .buttonStyle(.bordered)
            .tint(running ? .f1Red : .primary)
            .task { running = WeekendActivity.running }
        }
    }
}
