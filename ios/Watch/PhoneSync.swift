import Foundation
import WatchConnectivity
import WidgetKit

/// Reçoit de l'iPhone la langue choisie dans l'app, pour l'app montre et les complications.
final class PhoneSync: NSObject, WCSessionDelegate {
    static let shared = PhoneSync()

    func start() {
        guard WCSession.isSupported() else { return }
        WCSession.default.delegate = self
        WCSession.default.activate()
    }

    func session(_ session: WCSession, activationDidCompleteWith activationState: WCSessionActivationState, error: Error?) {
        apply(session.receivedApplicationContext)
    }

    func session(_ session: WCSession, didReceiveApplicationContext applicationContext: [String: Any]) {
        apply(applicationContext)
    }

    private func apply(_ context: [String: Any]) {
        guard let lang = context["language"] as? String else { return }
        let defaults = UserDefaults.standard
        guard defaults.string(forKey: "language") != lang else { return }
        defaults.set(lang, forKey: "language")
        WidgetCenter.shared.reloadAllTimelines()
    }
}
