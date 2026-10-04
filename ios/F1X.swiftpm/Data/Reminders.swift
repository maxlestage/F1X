import SwiftUI
import UserNotifications

/// Rappels locaux (sans serveur ni compte) : 15 min avant chaque séance des prochains
/// Grands Prix, et à l'arrivée de la course pour consulter les résultats.
enum SessionReminders {
    static let key = "sessionReminders"
    static var enabled: Bool { UserDefaults.standard.bool(forKey: key) }

    /// Demande l'autorisation puis programme les rappels ; `false` si l'utilisateur refuse.
    static func enable(races: [Race]) async -> Bool {
        let center = UNUserNotificationCenter.current()
        let granted = (try? await center.requestAuthorization(options: [.alert, .sound, .badge])) ?? false
        guard granted else { return false }
        await schedule(races: races)
        return true
    }

    static func disable() {
        UNUserNotificationCenter.current().removeAllPendingNotificationRequests()
    }

    /// Reprogramme les rappels des 3 prochains Grands Prix (limite iOS : 64 en attente).
    static func schedule(races: [Race]) async {
        guard enabled else { return }
        let center = UNUserNotificationCenter.current()
        center.removeAllPendingNotificationRequests()
        for race in races.filter({ !$0.isOver() }).prefix(3) {
            let title = "\(Flag.country(race.circuit.location.country)) \(race.raceName)"
            for session in race.sessions {
                let name = sessionLabel(session.name)
                await add(center, id: "f1x-\(race.id)-\(session.name)", at: session.date.addingTimeInterval(-15 * 60),
                          title: title, body: L("\(name) dans 15 minutes", "\(name) in 15 minutes"))
            }
            if let start = race.start {
                await add(center, id: "f1x-\(race.id)-results", at: start.addingTimeInterval(2.5 * 3600),
                          title: "🏁 \(race.raceName)",
                          body: L("Course terminée : résultats et classements mis à jour dans F1X.",
                                  "Race over: results and standings updated in F1X."))
            }
        }
    }

    private static func add(_ center: UNUserNotificationCenter, id: String, at date: Date, title: String, body: String) async {
        guard date > .now else { return }
        let content = UNMutableNotificationContent()
        content.title = title
        content.body = body
        content.sound = .default
        let parts = Calendar.current.dateComponents([.year, .month, .day, .hour, .minute], from: date)
        let request = UNNotificationRequest(identifier: id, content: content,
                                            trigger: UNCalendarNotificationTrigger(dateMatching: parts, repeats: false))
        try? await center.add(request)
    }
}

/// Bouton « Me prévenir avant chaque séance ».
struct ReminderToggle: View {
    let races: [Race]
    @AppStorage(SessionReminders.key) private var on = false
    @State private var denied = false

    var body: some View {
        Button {
            Task {
                if on {
                    on = false
                    SessionReminders.disable()
                } else {
                    on = true
                    let granted = await SessionReminders.enable(races: races)
                    if !granted {
                        on = false
                        denied = true
                    }
                }
            }
        } label: {
            Label(on ? L("Rappels activés · 15 min avant chaque séance", "Reminders on · 15 min before each session")
                     : L("Me prévenir avant chaque séance", "Remind me before each session"),
                  systemImage: on ? "bell.fill" : "bell")
                .font(.subheadline.weight(.semibold))
                .frame(maxWidth: .infinity)
        }
        .buttonStyle(.bordered)
        .tint(on ? .yellow : .primary)
        .alert(L("Notifications désactivées", "Notifications disabled"), isPresented: $denied) {
            Button("OK", role: .cancel) {}
        } message: {
            Text(L("Autorise F1X dans Réglages › Notifications.", "Allow F1X in Settings › Notifications."))
        }
    }
}
