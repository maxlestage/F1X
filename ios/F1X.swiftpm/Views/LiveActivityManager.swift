import ActivityKit
import Foundation

/// Démarre, met à jour et arrête la Live Activity de la séance suivie dans le Race Center.
@MainActor
final class LiveActivityManager: ObservableObject {
    static let shared = LiveActivityManager()

    @Published private(set) var running = false
    /// L'utilisateur l'a retirée : pas de redémarrage automatique pour cette séance.
    private var dismissedSession: Int?
    private var activity: Activity<RaceActivityAttributes>?
    private var lastUpdate = Date.distantPast
    private var lastState: RaceActivityAttributes.ContentState?

    var available: Bool { ActivityAuthorizationInfo().areActivitiesEnabled }

    func toggle(_ snap: Snapshot) {
        if running {
            dismissedSession = snap.session.session_key
            stop()
        } else {
            dismissedSession = nil
            start(snap)
        }
    }

    /// En direct (pas en replay) : la Live Activity démarre toute seule.
    func autoStart(_ snap: Snapshot) {
        guard snap.mode == "live", !snap.finished, !running, dismissedSession != snap.session.session_key else { return }
        start(snap)
    }

    func start(_ snap: Snapshot) {
        guard available, activity == nil else { return }
        let racing = ["Race", "Sprint"].contains(snap.session.session_name)
        let attrs = RaceActivityAttributes(sessionName: snap.session.session_name, location: snap.session.location, racing: racing)
        let state = Self.state(snap)
        do {
            activity = try Activity.request(attributes: attrs, content: .init(state: state, staleDate: Date().addingTimeInterval(120)))
            running = true
            lastState = state
            lastUpdate = Date()
        } catch {
            running = false
        }
    }

    /// Appelée à chaque image du direct ; envoie au plus une mise à jour toutes les 3 s.
    func update(_ snap: Snapshot) {
        guard let activity, Date().timeIntervalSince(lastUpdate) >= 3 else { return }
        let state = Self.state(snap)
        guard state.leaders != lastState?.leaders || state.lap != lastState?.lap || state.status != lastState?.status else { return }
        lastUpdate = Date()
        lastState = state
        Task { await activity.update(.init(state: state, staleDate: Date().addingTimeInterval(120))) }
        if snap.finished { stop(after: state) }
    }

    func stop(after state: RaceActivityAttributes.ContentState? = nil) {
        guard let activity else { return }
        let final = state ?? lastState
        Task {
            await activity.end(final.map { .init(state: $0, staleDate: nil) }, dismissalPolicy: .after(Date().addingTimeInterval(15 * 60)))
        }
        self.activity = nil
        running = false
    }

    static func state(_ snap: Snapshot) -> RaceActivityAttributes.ContentState {
        let leaders = snap.cars.sorted { $0.position < $1.position }.prefix(3).map {
            RaceActivityAttributes.Entry(position: $0.position, code: $0.code, colour: $0.colour, gap: $0.position == 1 ? "" : $0.gap)
        }
        return .init(lap: snap.lap, totalLaps: snap.total_laps, status: snap.track_status, leaders: Array(leaders), updated: Date())
    }
}
