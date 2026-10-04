import SwiftUI

/// Mise à jour automatique : recharge quand l'app revient au premier plan
/// et périodiquement tant que l'écran est affiché (pas besoin de tirer pour rafraîchir).
private struct AutoRefresh: ViewModifier {
    let interval: TimeInterval
    let action: () async -> Void
    @Environment(\.scenePhase) private var scenePhase
    @State private var lastRun = Date.now

    func body(content: Content) -> some View {
        content
            .task {
                while !Task.isCancelled {
                    try? await Task.sleep(for: .seconds(interval))
                    guard !Task.isCancelled, scenePhase == .active else { continue }
                    await run()
                }
            }
            .onChange(of: scenePhase) { _, phase in
                // Retour au premier plan après au moins 30 s : données fraîches.
                guard phase == .active, Date.now.timeIntervalSince(lastRun) > 30 else { return }
                Task { await run() }
            }
    }

    private func run() async {
        lastRun = .now
        await F1API.shared.expireCache()
        await action()
    }
}

extension View {
    /// Recharge `action` au retour dans l'app et toutes les `interval` secondes.
    func autoRefresh(every interval: TimeInterval = 120, _ action: @escaping () async -> Void) -> some View {
        modifier(AutoRefresh(interval: interval, action: action))
    }
}
