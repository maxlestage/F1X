import ActivityKit
import SwiftUI
import UserNotifications

/// Démarre et tient à jour la Live Activity du week-end (prochaine séance + minuteur).
@MainActor
enum WeekendActivity {
    static let key = "weekendActivity"
    /// Activé par défaut ; désactivable depuis l'accueil.
    static var enabled: Bool { UserDefaults.standard.object(forKey: key) as? Bool ?? true }

    /// Grand Prix que l'utilisateur a choisi de suivre (« saison-manche ») : l'activité est relancée
    /// à chaque ouverture de l'app, même plusieurs jours avant le week-end.
    private static let followedKey = "weekendActivityFollowed"
    /// Début de l'activité en cours, pour prévenir avant la limite d'iOS.
    private static let startedKey = "weekendActivityStarted"
    /// iOS arrête une Live Activity au bout de 8 h : on prévient un peu avant.
    private static let lifetime: TimeInterval = 8 * 3600
    private static let warningID = "f1x-weekend-activity-expiry"

    /// Activités encore affichées (iOS garde un moment celles qu'il a arrêtées).
    private static var live: [Activity<WeekendActivityAttributes>] {
        Activity<WeekendActivityAttributes>.activities.filter { $0.activityState == .active || $0.activityState == .stale }
    }

    static var running: Bool { !live.isEmpty }

    /// Durée approximative d'une séance (une course dure ~2 h).
    private static func duration(_ name: String) -> TimeInterval {
        name == "Course" ? 2 * 3600 : 3600
    }

    private static func short(_ name: String) -> String {
        switch name {
        case "Essais libres 1": return isFrench ? "EL1" : "FP1"
        case "Essais libres 2": return isFrench ? "EL2" : "FP2"
        case "Essais libres 3": return isFrench ? "EL3" : "FP3"
        case "Qualifs sprint": return isFrench ? "Q. sprint" : "Sprint Q"
        case "Sprint": return "Sprint"
        case "Qualifications": return isFrench ? "Qualifs" : "Quali"
        default: return isFrench ? "Course" : "Race"
        }
    }

    /// Appelée à chaque chargement du calendrier. `force` : démarrage demandé par l'utilisateur,
    /// même si le week-end n'a pas encore commencé. `last` : dernière course (pour le vainqueur).
    static func sync(races: [Race], last: Race? = nil, force: Bool = false) async {
        guard ActivityAuthorizationInfo().areActivitiesEnabled else { return }
        guard enabled || force else { await endAll(); return }
        let now = Date()
        let first = last?.results?.first
        let winner = first.map { "\($0.driver.givenName.prefix(1)). \($0.driver.familyName)" }
        let code = first.map { $0.driver.code ?? String($0.driver.familyName.prefix(3)).uppercased() }
        let winnerRace = last?.raceName.replacingOccurrences(of: " Grand Prix", with: "")

        let upcoming = races.first(where: { !$0.isOver(now: now) })
        let nextSession = upcoming.flatMap { race in
            race.sessions.first(where: { $0.date.addingTimeInterval(duration($0.name)) > now }).map { (race, $0) }
        }

        let race: Race
        let state: WeekendActivityAttributes.ContentState
        let stale: Date
        let soon: Bool
        if let last, let start = last.start, winner != nil, last.isOver(now: now), now < start.addingTimeInterval(14 * 3600) {
            // Course tout juste terminée : le vainqueur pendant une douzaine d'heures.
            race = last
            state = .init(session: L("Vainqueur", "Winner"), short: "🏆", start: start,
                          next: nextSession.map { pair in
                              let name = pair.0.raceName.replacingOccurrences(of: " Grand Prix", with: "")
                              let when = pair.1.date.formatted(.dateTime.weekday(.abbreviated).day().hour().minute().locale(appLocale))
                              return "\(name) · \(sessionLabel(pair.1.name)) \(when)"
                          },
                          winner: winner, winnerCode: code, winnerRace: winnerRace, podium: true,
                          steps: last.sessions.map { short($0.name) }, stepIndex: last.sessions.count)
            stale = start.addingTimeInterval(14 * 3600)
            soon = true
        } else if let pair = nextSession {
            let r = pair.0, current = pair.1
            let sessions = r.sessions
            let index = sessions.firstIndex(where: { $0.date == current.date }) ?? 0
            let following = sessions.indices.contains(index + 1) ? sessions[index + 1] : nil
            race = r
            state = .init(session: sessionLabel(current.name), short: short(current.name), start: current.date,
                          next: following.map {
                              "\(sessionLabel($0.name)) · \($0.date.formatted(.dateTime.weekday(.abbreviated).hour().minute().locale(appLocale)))"
                          },
                          winner: winner, winnerCode: code, winnerRace: winnerRace, podium: false,
                          steps: sessions.map { short($0.name) }, stepIndex: index)
            stale = current.date.addingTimeInterval(duration(current.name))
            // Démarrage automatique dans les 4 h qui précèdent la séance : iOS n'affiche une activité
            // que 8 h, elle couvre ainsi la séance entière (démarrée la veille, elle s'éteignait avant).
            soon = current.date.timeIntervalSince(now) < 4 * 3600
        } else {
            await endAll()
            return
        }
        let content = ActivityContent(state: state, staleDate: stale)

        let existing = live
        let lang = isFrench ? "fr" : "en"
        let raceKey = "\(race.season)-\(race.round)"
        if force { UserDefaults.standard.set(raceKey, forKey: followedKey) }
        if let activity = existing.first(where: {
            $0.attributes.round == race.round && $0.attributes.raceName == race.raceName && $0.attributes.lang == lang
        }) {
            if activity.content.state != state { await activity.update(content) }
            for other in existing where other.id != activity.id { await other.end(nil, dismissalPolicy: .immediate) }
            await scheduleWarning(race: race)
            return
        }
        for other in existing { await other.end(nil, dismissalPolicy: .immediate) }
        // Relancée si l'utilisateur suit ce Grand Prix, ou automatiquement peu avant la séance.
        let followed = UserDefaults.standard.string(forKey: followedKey) == raceKey
        guard soon || force || followed else { return }
        let attributes = WeekendActivityAttributes(
            raceName: race.raceName,
            flag: Flag.country(race.circuit.location.country),
            round: race.round,
            circuit: race.circuit.location.locality,
            lang: lang)
        if (try? Activity.request(attributes: attributes, content: content)) != nil {
            UserDefaults.standard.set(Date.now, forKey: startedKey)
            await scheduleWarning(race: race)
        }
    }

    /// Notification un quart d'heure avant que iOS n'arrête l'activité (8 h d'affichage au plus) :
    /// ouvrir F1X la relance. Seulement si les notifications sont déjà autorisées.
    private static func scheduleWarning(race: Race) async {
        let center = UNUserNotificationCenter.current()
        center.removePendingNotificationRequests(withIdentifiers: [warningID])
        guard let started = UserDefaults.standard.object(forKey: startedKey) as? Date else { return }
        let settings = await center.notificationSettings()
        guard settings.authorizationStatus == .authorized || settings.authorizationStatus == .provisional else { return }
        let fire = started.addingTimeInterval(lifetime - 15 * 60)
        guard fire > .now.addingTimeInterval(60) else { return }
        let content = UNMutableNotificationContent()
        content.title = "\(Flag.country(race.circuit.location.country)) \(race.raceName)"
        content.body = L("Le suivi du week-end va s'arrêter (iOS limite l'affichage à 8 h). Ouvre F1X pour le relancer.",
                         "Weekend tracking is about to stop (iOS limits it to 8 h). Open F1X to restart it.")
        let trigger = UNTimeIntervalNotificationTrigger(timeInterval: fire.timeIntervalSinceNow, repeats: false)
        try? await center.add(UNNotificationRequest(identifier: warningID, content: content, trigger: trigger))
    }

    static func endAll() async {
        UNUserNotificationCenter.current().removePendingNotificationRequests(withIdentifiers: [warningID])
        UserDefaults.standard.removeObject(forKey: followedKey)
        for activity in Activity<WeekendActivityAttributes>.activities {
            await activity.end(nil, dismissalPolicy: .immediate)
        }
    }
}

/// Bouton « Suivre le week-end » (écran verrouillé + Dynamic Island).
struct WeekendActivityToggle: View {
    let races: [Race]
    var last: Race?
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
                        await WeekendActivity.sync(races: races, last: last, force: true)
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
            if running {
                Text(L("iOS affiche une Live Activity 8 h au plus : F1X la relance à chaque ouverture et te prévient avant (si les rappels sont activés).",
                       "iOS shows a Live Activity for 8 h at most: F1X restarts it each time you open the app and warns you first (if reminders are on)."))
                    .font(.caption)
                    .foregroundStyle(.secondary)
                    .fixedSize(horizontal: false, vertical: true)
            }
        }
    }
}
