import Foundation
import SwiftUI

// Protocole WebSocket du serveur F1X (`/ws`) : messages JSON avec un champ `type`.

struct StintInfo: Decodable, Sendable, Hashable {
    let compound: String
    let from: Int
    let to: Int
}

struct LiveCar: Decodable, Identifiable, Sendable {
    let number: Int
    let code: String
    let name: String
    let team: String
    let colour: String
    let position: Int
    let gap: String
    let interval: String
    let last_lap: Double?
    let best_lap: Double?
    let fastest: Bool
    let lap: Int
    let compound: String?
    let tyre_age: Int?
    let pits: Int
    let in_pit: Bool
    let sectors: [Double?]
    let sector_flags: [Int]
    let lap_progress: Double?
    let stints: [StintInfo]
    let retired: Bool
    var id: Int { number }
    var color: Color { Color(hexString: colour) }
}

struct LiveWeather: Decodable, Sendable {
    let air_temperature: Double
    let track_temperature: Double
    let humidity: Double
    let wind_speed: Double
    let wind_direction: Double?
    let pressure: Double?
    let rainfall: Bool
}

struct RaceControlMsg: Decodable, Sendable, Identifiable, Hashable {
    let date: String
    let lap: Int?
    let category: String
    let flag: String?
    let message: String
    var id: String { date + message }
}

struct RaceEvent: Decodable, Sendable, Identifiable {
    let date: String
    let lap: Int
    let kind: String
    let driver: String?
    let passed: String?
    let position: Int?
    let duration: Double?
    let time: Double?
    let status: String?
    let message: String?
    var id: String { "\(date)-\(kind)-\(driver ?? "")-\(message ?? "")" }

    var text: String {
        switch kind {
        case "overtake": return L("\(driver ?? "") dépasse \(passed ?? "") → P\(position ?? 0)", "\(driver ?? "") passes \(passed ?? "") → P\(position ?? 0)")
        case "pit":
            let d = duration.map { String(format: " (%.1f s)", $0) } ?? ""
            return L("\(driver ?? "") s'arrête aux stands\(d)", "\(driver ?? "") pits\(d)")
        case "fastest_lap": return L("Meilleur tour : \(driver ?? "") en \(formatLap(time ?? 0))", "Fastest lap: \(driver ?? "") in \(formatLap(time ?? 0))")
        case "status": return trackStatusLabel(status ?? "")
        case "penalty": return message ?? ""
        case "retired": return L("Abandon de \(driver ?? "")", "\(driver ?? "") retires")
        default: return message ?? kind
        }
    }

    var icon: String {
        switch kind {
        case "overtake": "⇅"
        case "pit": "🔧"
        case "fastest_lap": "⏱"
        case "status": "🚩"
        case "penalty": "⚖️"
        case "retired": "✖︎"
        default: "•"
        }
    }
}

struct Snapshot: Decodable, Sendable {
    let mode: String
    let session: SessionSummary
    let clock: String
    let progress: Double
    let speed: Int
    let paused: Bool
    let lap: Int
    let total_laps: Int?
    let track_status: String
    let cars: [LiveCar]
    let weather: LiveWeather?
    let race_control: [RaceControlMsg]
    let events: [RaceEvent]
    let pit_loss: Double?
    let finished: Bool
}

func trackStatusLabel(_ s: String) -> String {
    switch s {
    case "green": L("Piste verte", "Green flag")
    case "yellow": L("Drapeau jaune", "Yellow flag")
    case "safety_car": L("Voiture de sécurité", "Safety car")
    case "virtual_safety_car": L("Voiture de sécurité virtuelle", "Virtual safety car")
    case "red": L("Drapeau rouge", "Red flag")
    case "chequered": L("Drapeau à damier", "Chequered flag")
    default: s
    }
}

func trackStatusColor(_ s: String) -> Color {
    switch s {
    case "yellow", "safety_car", "virtual_safety_car": .yellow
    case "red": .red
    case "chequered": .white
    default: .green
    }
}

func tyreColor(_ compound: String?) -> Color {
    switch (compound ?? "").uppercased() {
    case "SOFT": .red
    case "MEDIUM": .yellow
    case "HARD": .white
    case "INTERMEDIATE": .green
    case "WET": .blue
    default: .gray
    }
}

extension Color {
    /// « E10600 » ou « #E10600 ».
    init(hexString: String) {
        let h = hexString.trimmingCharacters(in: CharacterSet(charactersIn: "#"))
        self.init(hex: UInt32(h, radix: 16) ?? 0x8A8A99)
    }
}

/// Connexion au direct / replay du serveur F1X.
@MainActor
final class LiveClient: ObservableObject {
    @Published var status: String = ""
    @Published var liveAvailable = false
    @Published var liveActive = false
    @Published var viewers = 0
    @Published var loading: String?
    @Published var snapshot: Snapshot?
    @Published var track: TrackMap?
    @Published var error: String?
    @Published var connected = false

    private var task: URLSessionWebSocketTask?
    private var reconnect = true

    func connect() {
        guard task == nil else { return }
        reconnect = true
        let t = URLSession.shared.webSocketTask(with: Server.wsURL)
        task = t
        t.resume()
        connected = true
        receive()
    }

    func disconnect() {
        reconnect = false
        task?.cancel(with: .goingAway, reason: nil)
        task = nil
        connected = false
    }

    func send(_ json: [String: Any]) {
        guard let data = try? JSONSerialization.data(withJSONObject: json),
              let text = String(data: data, encoding: .utf8) else { return }
        task?.send(.string(text)) { _ in }
    }

    func replay(_ session: SessionSummary, speed: Int) {
        snapshot = nil
        track = nil
        error = nil
        loading = L("Chargement de la session…", "Loading session…")
        send(["type": "replay", "session_key": session.session_key, "speed": speed])
    }

    func followLive() {
        snapshot = nil
        track = nil
        send(["type": "live"])
    }

    func stop() {
        send(["type": "stop"])
        snapshot = nil
        track = nil
        loading = nil
    }

    private func receive() {
        task?.receive { [weak self] result in
            Task { @MainActor in
                guard let self else { return }
                switch result {
                case .success(let message):
                    if case .string(let text) = message { self.handle(text) }
                    self.receive()
                case .failure:
                    self.task = nil
                    self.connected = false
                    if self.reconnect {
                        try? await Task.sleep(nanoseconds: 3_000_000_000)
                        if self.reconnect { self.connect() }
                    }
                }
            }
        }
    }

    private struct Envelope: Decodable {
        let type: String
        let live_available: Bool?
        let live_active: Bool?
        let message: String?
        let count: Int?
    }

    private func handle(_ text: String) {
        guard let data = text.data(using: .utf8),
              let env = try? JSONDecoder().decode(Envelope.self, from: data) else { return }
        switch env.type {
        case "status":
            liveAvailable = env.live_available ?? false
            liveActive = env.live_active ?? false
            status = env.message ?? ""
        case "viewers":
            viewers = env.count ?? 0
        case "loading":
            loading = env.message
        case "snapshot":
            if let snap = try? JSONDecoder().decode(Snapshot.self, from: data) {
                snapshot = snap
                loading = nil
            }
        case "track":
            track = try? JSONDecoder().decode(TrackMap.self, from: data)
        case "stopped":
            snapshot = nil
            loading = nil
        case "error":
            error = env.message
            loading = nil
        default:
            break
        }
    }
}
