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
    /// Taille totale des réponses gardées en mémoire, bornée par `cacheBudget`.
    private var cacheBytes = 0
    /// Les relais OpenF1 (télémétrie, positions) pèsent parfois plusieurs Mo : sans limite,
    /// chaque séance consultée resterait en mémoire jusqu'à la fermeture de l'app.
    private static let cacheBudget = 24 * 1024 * 1024
    private var photos: [String: Photo?] = [:]
    private var inflight: [String: Task<Data, Error>] = [:]
    private var photoInflight: [String: Task<PhotoFetch, Never>] = [:]

    func get<T: Decodable>(_ path: String, as type: T.Type, ttl: TimeInterval = 600) async throws -> T {
        if let hit = cache[path], Date().timeIntervalSince(hit.0) < ttl,
           let decoded = try? JSONDecoder().decode(T.self, from: hit.1) {
            return decoded
        }
        do {
            let data = try await download(path)
            let decoded = try JSONDecoder().decode(T.self, from: data)
            store(path, data)
            return decoded
        } catch let error as URLError where error.code != .fileDoesNotExist {
            // Réseau ou serveur indisponible : dernière copie connue plutôt qu'une erreur.
            if let stale = cache[path], let decoded = try? JSONDecoder().decode(T.self, from: stale.1) {
                return decoded
            }
            throw error
        }
    }

    /// Garde une réponse en mémoire ; au-delà du budget, les plus anciennes sont oubliées
    /// (la plus récente est toujours gardée, même si elle dépasse seule le budget).
    private func store(_ path: String, _ data: Data) {
        if let old = cache[path] { cacheBytes -= old.1.count }
        cache[path] = (Date(), data)
        cacheBytes += data.count
        guard cacheBytes > Self.cacheBudget else { return }
        for (key, entry) in cache.sorted(by: { $0.value.0 < $1.value.0 }) where key != path {
            cache[key] = nil
            cacheBytes -= entry.1.count
            if cacheBytes <= Self.cacheBudget { break }
        }
    }

    /// Fichier binaire (décor 3D d'un circuit) gardé en mémoire : rouvrir la vue 3D ne le
    /// re-télécharge pas (trop gros pour le cache HTTP d'URLSession). Seuls les derniers
    /// fichiers sont conservés pour borner la mémoire ; hors ligne, la dernière copie sert.
    private var blobs: [(path: String, date: Date, data: Data)] = []

    func data(_ path: String, ttl: TimeInterval) async throws -> Data {
        let hit = blobs.first { $0.path == path }
        if let hit, Date().timeIntervalSince(hit.date) < ttl { return hit.data }
        do {
            let data = try await download(path)
            blobs.removeAll { $0.path == path }
            blobs.insert((path: path, date: Date(), data: data), at: 0)
            if blobs.count > 3 { blobs.removeLast(blobs.count - 3) }
            return data
        } catch let error as URLError where error.code != .fileDoesNotExist {
            if let hit { return hit.data }
            throw error
        }
    }

    /// Une seule requête réseau par adresse à la fois : les vues ouvertes en même temps
    /// (tracé du circuit, télémétrie, relais OpenF1) attendent la même réponse.
    private func download(_ path: String) async throws -> Data {
        if let running = inflight[path] { return try await running.value }
        let task = Task { try await Self.fetch(path) }
        inflight[path] = task
        defer { inflight[path] = nil }
        return try await task.value
    }

    private static func fetch(_ path: String) async throws -> Data {
        guard let url = URL(string: path, relativeTo: Server.base) else { throw URLError(.badURL) }
        // Délai borné : jamais de chargement infini (le bouton « Réessayer » prend le relais).
        var request = URLRequest(url: url)
        request.timeoutInterval = 30
        let (data, response) = try await URLSession.shared.data(for: request)
        guard let http = response as? HTTPURLResponse else { throw URLError(.badServerResponse) }
        if http.statusCode == 404 { throw URLError(.fileDoesNotExist) }
        guard (200..<300).contains(http.statusCode) else { throw URLError(.badServerResponse) }
        return data
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
        // Une seule requête par photo à la fois : la même photo affichée par plusieurs vues
        // (avatar dans l'accueil et les classements, grande photo de la fiche) n'est téléchargée qu'une fois.
        let task: Task<PhotoFetch, Never>
        if let running = photoInflight[title] {
            task = running
        } else {
            task = Task { await Self.fetchPhoto(title) }
            photoInflight[title] = task
        }
        let result = await task.value
        if photoInflight[title] == task { photoInflight[title] = nil }
        switch result {
        case .found(let photo, let data):
            photos[title] = photo
            UserDefaults.standard.set(data, forKey: key)
            return photo
        case .missing:
            photos[title] = .some(nil)
            return nil
        case .failed:
            return nil
        }
    }

    private enum PhotoFetch: Sendable {
        case found(Photo, Data)
        case missing
        case failed
    }

    private static func fetchPhoto(_ title: String) async -> PhotoFetch {
        guard let api = URL(string: "api/photo/\(title)", relativeTo: Server.base) else { return .failed }
        // Délai borné : hors réseau, l'initiale s'affiche vite à la place de la photo.
        var request = URLRequest(url: api)
        request.timeoutInterval = 15
        guard let (data, response) = try? await URLSession.shared.data(for: request),
              let http = response as? HTTPURLResponse
        else { return .failed }
        if http.statusCode == 404 { return .missing }
        guard http.statusCode == 200, let photo = try? JSONDecoder().decode(Photo.self, from: data) else { return .failed }
        return .found(photo, data)
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

extension AppLanguage {
    /// Envoie la langue effective (« fr » / « en ») à la montre.
    static func share() {
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
