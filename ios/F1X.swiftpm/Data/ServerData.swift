import Foundation

// Données propres au serveur F1X : tracés GPS, photos libres, actualités, champions, sessions.

struct TrackPoint: Decodable, Sendable {
    let x: Double
    let y: Double
    let z: Double?
    let t: Double
    let speed: Int
    let gear: Int
    let throttle: Int
    let brake: Bool
}

struct TrackStats: Decodable, Sendable {
    let top_speed: Int
    let min_speed: Int
    let avg_speed: Double
    let full_throttle_pct: Double
    let braking_pct: Double
    let length_km: Double
    let gear_changes: Int
}

struct TrackMap: Decodable, Sendable {
    let circuit_id: String
    let year: Int
    let event: String
    let driver: String
    let team: String
    let colour: String
    let lap_time: Double
    let width: Double
    let height: Double
    let points: [TrackPoint]
    let stats: TrackStats

    var hasRelief: Bool { (points.map { $0.z ?? 0 }.max() ?? 0) > 1 }
}

struct Photo: Codable, Sendable, Hashable {
    let src: String
    let credit: String
}

struct Article: Decodable, Identifiable, Sendable {
    let title: String
    let link: String
    let source: String
    let date: String
    let excerpt: String
    let image: String?
    var id: String { link }
}

struct Champion: Decodable, Identifiable, Sendable {
    let season: String
    let in_progress: Bool?
    let driver: Driver?
    let driver_team: Constructor?
    let driver_points: String?
    let driver_wins: String?
    let constructor: Constructor?
    let constructor_points: String?
    var id: String { season }
}

struct SessionSummary: Codable, Identifiable, Hashable, Sendable {
    let session_key: Int
    let session_name: String
    let session_type: String
    let location: String
    let country: String
    let circuit: String
    let date_start: String
    let date_end: String
    let year: Int
    var id: Int { session_key }
}

/// Requêtes vers le serveur F1X (hors données Jolpica), avec cache mémoire.
actor ServerAPI {
    static let shared = ServerAPI()

    private var cache: [String: (Date, Data)] = [:]
    private var photos: [String: Photo?] = [:]

    func get<T: Decodable>(_ path: String, as type: T.Type, ttl: TimeInterval = 600) async throws -> T {
        if let hit = cache[path], Date().timeIntervalSince(hit.0) < ttl {
            return try JSONDecoder().decode(T.self, from: hit.1)
        }
        guard let url = URL(string: path, relativeTo: Server.base) else { throw URLError(.badURL) }
        // Délai borné : jamais de chargement infini (le bouton « Réessayer » prend le relais).
        var request = URLRequest(url: url)
        request.timeoutInterval = 30
        do {
            let (data, response) = try await URLSession.shared.data(for: request)
            guard let http = response as? HTTPURLResponse else { throw URLError(.badServerResponse) }
            if http.statusCode == 404 { throw URLError(.fileDoesNotExist) }
            guard (200..<300).contains(http.statusCode) else { throw URLError(.badServerResponse) }
            let decoded = try JSONDecoder().decode(T.self, from: data)
            cache[path] = (Date(), data)
            return decoded
        } catch let error as URLError where error.code != .fileDoesNotExist {
            // Réseau ou serveur indisponible : dernière copie connue plutôt qu'une erreur.
            if let stale = cache[path], let decoded = try? JSONDecoder().decode(T.self, from: stale.1) {
                return decoded
            }
            throw error
        }
    }

    func track(_ circuitId: String) async throws -> TrackMap {
        try await get("api/track/\(circuitId)", as: TrackMap.self, ttl: 86_400)
    }

    func news(lang: String) async throws -> [Article] {
        try await get("api/news/\(lang)", as: [Article].self, ttl: 600)
    }

    func champions() async throws -> [Champion] {
        try await get("api/champions", as: [Champion].self, ttl: 3600)
    }

    func sessions(year: Int) async throws -> [SessionSummary] {
        try await get("api/live/sessions/\(year)", as: [SessionSummary].self, ttl: 300)
    }

    /// Photo libre (Wikimedia Commons) d'un article Wikipédia, `nil` s'il n'y en a pas.
    func photo(wikipedia url: String?) async -> Photo? {
        guard let url, let title = url.components(separatedBy: "/wiki/").last, !title.isEmpty else { return nil }
        if let hit = photos[title] { return hit }
        let key = "f1x-photo960-\(title)"
        if let data = UserDefaults.standard.data(forKey: key), let stored = try? JSONDecoder().decode(Photo.self, from: data) {
            photos[title] = stored
            return stored
        }
        guard let api = URL(string: "api/photo/\(title)", relativeTo: Server.base),
              let result = try? await URLSession.shared.data(from: api),
              let http = result.1 as? HTTPURLResponse
        else { return nil }
        let data = result.0
        if http.statusCode == 404 {
            photos[title] = .some(nil)
            return nil
        }
        guard http.statusCode == 200, let photo = try? JSONDecoder().decode(Photo.self, from: data) else { return nil }
        photos[title] = photo
        UserDefaults.standard.set(data, forKey: key)
        return photo
    }
}

/// Langue de l'appareil : français ou anglais.
/// Langue choisie dans Explorer : « auto » (celle de l'iPhone), « fr » ou « en ».
enum AppLanguage {
    static let key = "language"
    static func resolve() -> Bool {
        switch UserDefaults.standard.string(forKey: key) {
        case "fr": return true
        case "en": return false
        default: return (Locale.preferredLanguages.first ?? "fr").hasPrefix("fr")
        }
    }
}

/// Vrai si l'app est en français (mis à jour quand on change de langue).
var isFrench: Bool = AppLanguage.resolve()

/// Groupe partagé avec les widgets et Live Activities (langue choisie).
let appGroup = "group.com.maxlestage.f1x"

extension AppLanguage {
    /// Publie la langue effective (« fr » / « en ») pour les widgets et la montre.
    static func share() {
        UserDefaults(suiteName: appGroup)?.set(isFrench ? "fr" : "en", forKey: key)
        WatchSync.shared.send()
    }
}

/// Format des dates dans la langue de l'app.
var appLocale: Locale { Locale(identifier: isFrench ? "fr_FR" : "en_GB") }

/// Texte dans la langue de l'appareil.
func L(_ fr: String, _ en: String) -> String { isFrench ? fr : en }

/// « 92.345 » → « 1:32.345 ».
func formatLap(_ seconds: Double) -> String {
    guard seconds.isFinite, abs(seconds) < 1e7 else { return "–" }
    let m = Int(seconds) / 60
    let s = seconds - Double(m * 60)
    return m > 0 ? String(format: "%d:%06.3f", m, s) : String(format: "%.3f", s)
}
