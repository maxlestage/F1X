import Foundation

/// Client de l'API F1 Jolpica (https://api.jolpi.ca), avec cache mémoire de 5 minutes.
actor F1API {
    static let shared = F1API()

    private let base = URL(string: "https://api.jolpi.ca/ergast/f1/")!
    private let ttl: TimeInterval = 300
    private var cache: [String: (date: Date, data: Data)] = [:]

    // MARK: - Endpoints

    func schedule() async throws -> [Race] {
        try await fetch("current.json", as: RaceResponse.self).races
    }

    func lastResults() async throws -> Race? {
        try await fetch("current/last/results.json", as: RaceResponse.self).races.first
    }

    func results(round: Int) async throws -> [RaceResult] {
        try await fetch("current/\(round)/results.json", as: RaceResponse.self).races.first?.results ?? []
    }

    func sprint(round: Int) async throws -> [RaceResult] {
        try await fetch("current/\(round)/sprint.json", as: RaceResponse.self).races.first?.sprintResults ?? []
    }

    func qualifying(round: Int) async throws -> [QualifyingResult] {
        try await fetch("current/\(round)/qualifying.json", as: RaceResponse.self).races.first?.qualifyingResults ?? []
    }

    func driverStandings() async throws -> [DriverStanding] {
        try await fetch("current/driverStandings.json", as: StandingsResponse.self)
            .lists.first?.driverStandings ?? []
    }

    func constructorStandings() async throws -> [ConstructorStanding] {
        try await fetch("current/constructorStandings.json", as: StandingsResponse.self)
            .lists.first?.constructorStandings ?? []
    }

    func driverResults(driverId: String) async throws -> [Race] {
        try await fetch("current/drivers/\(driverId)/results.json?limit=100", as: RaceResponse.self).races
    }

    // MARK: - Réseau

    func clearCache() { cache.removeAll() }

    private func fetch<T: Decodable>(_ path: String, as type: T.Type) async throws -> T {
        if let hit = cache[path], Date().timeIntervalSince(hit.date) < ttl {
            return try JSONDecoder().decode(T.self, from: hit.data)
        }
        guard let url = URL(string: path, relativeTo: base) else { throw URLError(.badURL) }
        do {
            let (data, response) = try await URLSession.shared.data(from: url)
            guard let http = response as? HTTPURLResponse, (200..<300).contains(http.statusCode) else {
                throw URLError(.badServerResponse)
            }
            let decoded = try JSONDecoder().decode(T.self, from: data)
            cache[path] = (Date(), data)
            return decoded
        } catch {
            // Hors ligne : on sert la dernière copie connue plutôt qu'une erreur.
            if let stale = cache[path] {
                return try JSONDecoder().decode(T.self, from: stale.data)
            }
            throw error
        }
    }
}

// MARK: - Enveloppes de réponse

private struct MRData<Table: Decodable>: Decodable {
    let table: Table

    enum CodingKeys: String, CodingKey { case MRData }
    enum InnerKeys: String, CodingKey { case RaceTable, StandingsTable }

    init(from decoder: Decoder) throws {
        let root = try decoder.container(keyedBy: CodingKeys.self)
        let inner = try root.nestedContainer(keyedBy: InnerKeys.self, forKey: .MRData)
        if let t = try? inner.decode(Table.self, forKey: .RaceTable) {
            table = t
        } else {
            table = try inner.decode(Table.self, forKey: .StandingsTable)
        }
    }
}

struct RaceResponse: Decodable, Sendable {
    let races: [Race]

    private struct Table: Decodable {
        let Races: [Race]
    }

    init(from decoder: Decoder) throws {
        races = try MRData<Table>(from: decoder).table.Races
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

    private struct Table: Decodable {
        let StandingsLists: [StandingsList]
    }

    init(from decoder: Decoder) throws {
        lists = try MRData<Table>(from: decoder).table.StandingsLists
    }
}
