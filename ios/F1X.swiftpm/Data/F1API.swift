import Foundation

/// Adresse du serveur F1X (cache partagé des données F1, photos, tracés, actus, direct).
enum Server {
    static let base = URL(string: "https://f1x-29170430865f.herokuapp.com/")!
    static let wsURL = URL(string: "wss://f1x-29170430865f.herokuapp.com/ws")!
}

/// Client des données F1 (API Jolpica, ex-Ergast) via le serveur F1X, avec cache mémoire de 5 min
/// et dernière copie servie hors ligne.
actor F1API {
    static let shared = F1API()

    private let ttl: TimeInterval = 300
    private var cache: [String: (date: Date, data: Data)] = [:]

    /// Vrai pendant un premier affichage : la dernière copie connue (même ancienne) est servie
    /// tout de suite, sans attendre le réseau.
    @TaskLocal static var preferCached = false

    /// Affiche immédiatement les données gardées sur l'appareil, puis recharge depuis le réseau.
    static func instant(_ load: () async -> Void) async {
        await $preferCached.withValue(true) { await load() }
        await load()
    }

    /// Précharge les données de base au lancement de l'app.
    func warmUp() async {
        async let a = try? schedule()
        async let b = try? driverStandings()
        async let c = try? constructorStandings()
        async let d = try? lastResults()
        _ = await (a, b, c, d)
    }

    // MARK: - Calendrier et résultats

    func schedule(season: String = "current") async throws -> [Race] {
        try await races("\(season).json?limit=100")
    }

    func lastResults() async throws -> Race? {
        try await races("current/last/results.json?limit=100").first
    }

    func results(season: String = "current", round: Int) async throws -> [RaceResult] {
        try await races("\(season)/\(round)/results.json?limit=100").first?.results ?? []
    }

    func sprint(season: String = "current", round: Int) async throws -> [RaceResult] {
        try await races("\(season)/\(round)/sprint.json?limit=100").first?.sprintResults ?? []
    }

    func qualifying(season: String = "current", round: Int) async throws -> [QualifyingResult] {
        try await races("\(season)/\(round)/qualifying.json?limit=100").first?.qualifyingResults ?? []
    }

    func pitStops(season: String = "current", round: Int) async throws -> [PitStop] {
        try await races("\(season)/\(round)/pitstops.json?limit=100").first?.pitStops ?? []
    }

    /// Tous les tours d'une course (positions et temps de chaque pilote), pages fusionnées.
    func laps(season: String = "current", round: Int) async throws -> [LapData] {
        let pages = try await races("\(season)/\(round)/laps.json", all: true)
        var byLap: [Int: [LapTiming]] = [:]
        for race in pages {
            for lap in race.laps ?? [] {
                byLap[Int(lap.number) ?? 0, default: []] += lap.timings
            }
        }
        return byLap.keys.sorted().map { LapData(number: String($0), timings: byLap[$0] ?? []) }
    }

    /// Vainqueurs de chaque saison ou d'un circuit (`results/1`).
    func winners(_ prefix: String) async throws -> [Race] {
        try await races("\(prefix)/results/1.json?limit=100", all: true)
    }

    /// Toute une saison, course par course (résultats, sprints, qualifications), pages fusionnées.
    func seasonRaces(_ season: String, kind: String = "results") async throws -> [Race] {
        let pages = try await races("\(season)/\(kind).json", all: true)
        var order: [String] = []
        var byRound: [String: Race] = [:]
        for race in pages {
            if let prev = byRound[race.round] {
                byRound[race.round] = prev.merging(race)
            } else {
                order.append(race.round)
                byRound[race.round] = race
            }
        }
        return order.compactMap { byRound[$0] }.sorted { $0.roundNumber < $1.roundNumber }
    }

    // MARK: - Classements

    func driverStandings(season: String = "current") async throws -> [DriverStanding] {
        try await fetch("\(season)/driverStandings.json?limit=100", as: StandingsResponse.self)
            .lists.first?.driverStandings ?? []
    }

    func constructorStandings(season: String = "current") async throws -> [ConstructorStanding] {
        try await fetch("\(season)/constructorStandings.json?limit=100", as: StandingsResponse.self)
            .lists.first?.constructorStandings ?? []
    }

    // MARK: - Pilotes, écuries, circuits

    func driverResults(driverId: String, season: String = "current") async throws -> [Race] {
        try await races("\(season)/drivers/\(driverId)/results.json?limit=100")
    }

    /// Toute la carrière d'un pilote (toutes les pages fusionnées par le serveur).
    func career(driverId: String) async throws -> [Race] {
        try await races("drivers/\(driverId)/results.json", all: true)
    }

    func driver(_ id: String) async throws -> Driver? {
        try await fetch("drivers/\(id).json?limit=1", as: Table<DriverTable>.self).table.Drivers.first
    }

    func constructor(_ id: String) async throws -> Constructor? {
        try await fetch("constructors/\(id).json?limit=1", as: Table<ConstructorTable>.self).table.Constructors.first
    }

    func circuitRaces(_ id: String) async throws -> [Race] {
        try await races("circuits/\(id)/races.json", all: true)
    }

    func seasons() async throws -> [String] {
        try await fetch("seasons.json?limit=100", as: Table<SeasonTable>.self).table.Seasons.map(\.season)
    }

    /// Nombre total de résultats d'une requête (champ `total`), ex. victoires d'une écurie.
    func total(_ path: String) async throws -> Int {
        let sep = path.contains("?") ? "&" : "?"
        return try await fetch("\(path)\(sep)limit=1", as: Total.self).total
    }

    // MARK: - Réseau

    func clearCache() { cache.removeAll() }

    /// Périme le cache sans l'effacer : la prochaine lecture repart au réseau,
    /// mais la dernière copie reste disponible hors ligne.
    func expireCache() {
        for (key, hit) in cache { cache[key] = (.distantPast, hit.data) }
    }

    private func races(_ path: String, all: Bool = false) async throws -> [Race] {
        try await fetch(path, as: RaceResponse.self, all: all).races
    }

    private func fetch<T: Decodable>(_ path: String, as type: T.Type, all: Bool = false) async throws -> T {
        let key = (all ? "all/" : "f1/") + path
        if cache[key] == nil, let disk = DiskCache.read(key) { cache[key] = disk }
        if let hit = cache[key], F1API.preferCached || Date().timeIntervalSince(hit.date) < ttl {
            return try JSONDecoder().decode(T.self, from: hit.data)
        }
        guard let url = URL(string: "api/\(key)", relativeTo: Server.base) else { throw URLError(.badURL) }
        do {
            let (data, response) = try await URLSession.shared.data(from: url)
            guard let http = response as? HTTPURLResponse, (200..<300).contains(http.statusCode) else {
                throw URLError(.badServerResponse)
            }
            let decoded = try JSONDecoder().decode(T.self, from: data)
            cache[key] = (Date(), data)
            DiskCache.write(key, data)
            return decoded
        } catch {
            // Hors ligne : on sert la dernière copie connue plutôt qu'une erreur.
            if let stale = cache[key] {
                return try JSONDecoder().decode(T.self, from: stale.data)
            }
            throw error
        }
    }
}

// MARK: - Enveloppes de réponse

private struct AnyKey: CodingKey {
    var stringValue: String
    var intValue: Int? { nil }
    init(_ s: String) { stringValue = s }
    init?(stringValue: String) { self.stringValue = stringValue }
    init?(intValue: Int) { nil }
}

/// `{"MRData": {"RaceTable" | "StandingsTable" | …: {...}}}`
private struct MRData<Inner: Decodable>: Decodable {
    let table: Inner

    init(from decoder: Decoder) throws {
        let root = try decoder.container(keyedBy: AnyKey.self)
        let inner = try root.nestedContainer(keyedBy: AnyKey.self, forKey: AnyKey("MRData"))
        for name in ["RaceTable", "StandingsTable", "DriverTable", "ConstructorTable", "SeasonTable", "CircuitTable"] {
            if let t = try? inner.decode(Inner.self, forKey: AnyKey(name)) {
                table = t
                return
            }
        }
        throw DecodingError.dataCorrupted(.init(codingPath: [], debugDescription: "Table absente"))
    }
}

struct Table<Inner: Decodable>: Decodable {
    let table: Inner
    init(from decoder: Decoder) throws { table = try MRData<Inner>(from: decoder).table }
}

struct DriverTable: Decodable { let Drivers: [Driver] }
struct ConstructorTable: Decodable { let Constructors: [Constructor] }
struct SeasonTable: Decodable {
    struct Season: Decodable { let season: String }
    let Seasons: [Season]
}

private struct Total: Decodable {
    let total: Int
    init(from decoder: Decoder) throws {
        let root = try decoder.container(keyedBy: AnyKey.self)
        let inner = try root.nestedContainer(keyedBy: AnyKey.self, forKey: AnyKey("MRData"))
        total = Int(try inner.decode(String.self, forKey: AnyKey("total"))) ?? 0
    }
}

struct RaceResponse: Decodable, Sendable {
    let races: [Race]

    private struct Inner: Decodable { let Races: [Race] }

    init(from decoder: Decoder) throws {
        races = try MRData<Inner>(from: decoder).table.Races
    }
}

struct StandingsResponse: Decodable, Sendable {
    let lists: [StandingsList]

    struct StandingsList: Decodable, Sendable {
        let driverStandings: [DriverStanding]?
        let constructorStandings: [ConstructorStanding]?

        enum CodingKeys: String, CodingKey {
            case driverStandings = "DriverStandings"
            case constructorStandings = "ConstructorStandings"
        }
    }

    private struct Inner: Decodable { let StandingsLists: [StandingsList] }

    init(from decoder: Decoder) throws {
        lists = try MRData<Inner>(from: decoder).table.StandingsLists
    }
}

/// Copie des réponses sur l'appareil : l'app s'ouvre instantanément, même hors ligne.
enum DiskCache {
    private static let dir: URL = {
        let base = FileManager.default.urls(for: .cachesDirectory, in: .userDomainMask)[0]
        let dir = base.appendingPathComponent("f1x-api", isDirectory: true)
        try? FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        return dir
    }()

    private static func file(_ key: String) -> URL {
        let name = Data(key.utf8).base64EncodedString()
            .replacingOccurrences(of: "/", with: "_")
            .replacingOccurrences(of: "+", with: "-")
        return dir.appendingPathComponent(String(name.suffix(200)))
    }

    static func read(_ key: String) -> (date: Date, data: Data)? {
        let url = file(key)
        guard let data = try? Data(contentsOf: url) else { return nil }
        let attributes = try? FileManager.default.attributesOfItem(atPath: url.path)
        let date = attributes?[.modificationDate] as? Date ?? .distantPast
        return (date, data)
    }

    static func write(_ key: String, _ data: Data) {
        try? data.write(to: file(key), options: .atomic)
    }
}
