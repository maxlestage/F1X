import Foundation

// Modèles de l'API Jolpica (ex-Ergast). L'API renvoie les nombres sous forme de chaînes.

struct Race: Decodable, Identifiable, Hashable, Sendable {
    let season: String
    let round: String
    let raceName: String
    let circuit: Circuit
    let date: String
    let time: String?
    let firstPractice: Session?
    let secondPractice: Session?
    let thirdPractice: Session?
    let sprintQualifying: Session?
    let sprintShootout: Session?
    let sprint: Session?
    let qualifying: Session?
    let results: [RaceResult]?
    let sprintResults: [RaceResult]?
    let qualifyingResults: [QualifyingResult]?

    var id: String { "\(season)-\(round)" }
    var roundNumber: Int { Int(round) ?? 0 }
    var start: Date? { Session(date: date, time: time).start }
    var isSprintWeekend: Bool { sprint != nil }

    /// La course est considérée terminée ~2 h après le départ.
    func isOver(now: Date = .now) -> Bool {
        guard let start else { return false }
        return now > start.addingTimeInterval(2 * 3600)
    }

    /// Sessions du week-end, dans l'ordre chronologique.
    var sessions: [(name: String, date: Date)] {
        let all: [(String, Session?)] = [
            ("Essais libres 1", firstPractice),
            ("Essais libres 2", secondPractice),
            ("Essais libres 3", thirdPractice),
            ("Qualifs sprint", sprintQualifying ?? sprintShootout),
            ("Sprint", sprint),
            ("Qualifications", qualifying),
            ("Course", Session(date: date, time: time)),
        ]
        return all
            .compactMap { entry -> (name: String, date: Date)? in
                guard let start = entry.1?.start else { return nil }
                return (name: entry.0, date: start)
            }
            .sorted { $0.date < $1.date }
    }

    static func == (lhs: Race, rhs: Race) -> Bool { lhs.id == rhs.id }
    func hash(into hasher: inout Hasher) { hasher.combine(id) }

    enum CodingKeys: String, CodingKey {
        case season, round, raceName, date, time
        case circuit = "Circuit"
        case firstPractice = "FirstPractice"
        case secondPractice = "SecondPractice"
        case thirdPractice = "ThirdPractice"
        case sprintQualifying = "SprintQualifying"
        case sprintShootout = "SprintShootout"
        case sprint = "Sprint"
        case qualifying = "Qualifying"
        case results = "Results"
        case sprintResults = "SprintResults"
        case qualifyingResults = "QualifyingResults"
    }
}

struct Session: Decodable, Sendable {
    let date: String
    let time: String?

    var start: Date? {
        let iso = "\(date)T\(time ?? "00:00:00Z")"
        return Session.parser.date(from: iso) ?? Session.parserFractional.date(from: iso)
    }

    private static let parser: ISO8601DateFormatter = {
        let f = ISO8601DateFormatter()
        f.formatOptions = [.withInternetDateTime]
        return f
    }()

    private static let parserFractional: ISO8601DateFormatter = {
        let f = ISO8601DateFormatter()
        f.formatOptions = [.withInternetDateTime, .withFractionalSeconds]
        return f
    }()
}

struct Circuit: Decodable, Sendable {
    let circuitName: String
    let location: Location

    enum CodingKeys: String, CodingKey {
        case circuitName
        case location = "Location"
    }
}

struct Location: Decodable, Sendable {
    let locality: String
    let country: String
}

struct Driver: Decodable, Hashable, Sendable {
    let driverId: String
    let permanentNumber: String?
    let code: String?
    let givenName: String
    let familyName: String
    let nationality: String?

    var fullName: String { "\(givenName) \(familyName)" }
}

struct Constructor: Decodable, Hashable, Sendable {
    let constructorId: String
    let name: String
}

struct TimeValue: Decodable, Sendable {
    let time: String
}

struct FastestLap: Decodable, Sendable {
    let rank: String?
}

struct RaceResult: Decodable, Identifiable, Sendable {
    let position: String
    let positionText: String
    let points: String
    let driver: Driver
    let constructor: Constructor
    let grid: String?
    let status: String?
    let time: TimeValue?
    let fastestLap: FastestLap?

    var id: String { driver.driverId }
    var hasFastestLap: Bool { fastestLap?.rank == "1" }
    var scoredPoints: Bool { (Double(points) ?? 0) > 0 }

    /// Temps / écart, ou statut d'abandon traduit.
    var outcome: String {
        if let time { return time.time }
        switch status ?? "" {
        case "Finished": return ""
        case "Retired", "Accident", "Collision": return "Abandon"
        case "Did not start": return "Non partant"
        case "Disqualified": return "Disqualifié"
        case "Lapped": return "Doublé"
        case let s where s.hasPrefix("+") && s.contains("Lap"):
            let n = s.dropFirst().split(separator: " ").first.map { String($0) } ?? "1"
            return "+\(n) tour\(n == "1" ? "" : "s")"
        case let s: return s
        }
    }

    enum CodingKeys: String, CodingKey {
        case position, positionText, points, grid, status
        case driver = "Driver"
        case constructor = "Constructor"
        case time = "Time"
        case fastestLap = "FastestLap"
    }
}

struct QualifyingResult: Decodable, Identifiable, Sendable {
    let position: String
    let driver: Driver
    let constructor: Constructor
    let q1: String?
    let q2: String?
    let q3: String?

    var id: String { driver.driverId }

    var best: (segment: String, time: String)? {
        for (label, value) in [("Q3", q3), ("Q2", q2), ("Q1", q1)] {
            if let value, !value.isEmpty { return (label, value) }
        }
        return nil
    }

    enum CodingKeys: String, CodingKey {
        case position
        case driver = "Driver"
        case constructor = "Constructor"
        case q1 = "Q1", q2 = "Q2", q3 = "Q3"
    }
}

struct DriverStanding: Decodable, Identifiable, Sendable {
    let position: String?
    let positionText: String
    let points: String
    let wins: String
    let driver: Driver
    let constructors: [Constructor]

    var id: String { driver.driverId }
    var rank: String { position ?? positionText }
    var team: Constructor? { constructors.last }

    enum CodingKeys: String, CodingKey {
        case position, positionText, points, wins
        case driver = "Driver"
        case constructors = "Constructors"
    }

    init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        position = try c.decodeIfPresent(String.self, forKey: .position)
        positionText = try c.decode(String.self, forKey: .positionText)
        points = try c.decode(String.self, forKey: .points)
        wins = try c.decode(String.self, forKey: .wins)
        driver = try c.decode(Driver.self, forKey: .driver)
        constructors = try c.decodeIfPresent([Constructor].self, forKey: .constructors) ?? []
    }
}

struct ConstructorStanding: Decodable, Identifiable, Sendable {
    let position: String?
    let positionText: String
    let points: String
    let wins: String
    let constructor: Constructor

    var id: String { constructor.constructorId }
    var rank: String { position ?? positionText }

    enum CodingKeys: String, CodingKey {
        case position, positionText, points, wins
        case constructor = "Constructor"
    }
}
