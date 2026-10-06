import Foundation
import SwiftUI

// Données F1 pour l'Apple Watch (app et complications), lues sur le serveur F1X.

let f1Red = Color(red: 0.88, green: 0.02, blue: 0)
let f1Gold = Color(red: 1, green: 0.78, blue: 0.2)
/// Groupe partagé entre l'app montre et ses complications (langue reçue de l'iPhone).
let watchGroup = "group.com.maxlestage.f1x"

/// Langue choisie dans l'app iPhone, sinon celle de la montre.
var watchFrench: Bool {
    if let lang = UserDefaults(suiteName: watchGroup)?.string(forKey: "language") { return lang == "fr" }
    return Locale.preferredLanguages.first?.hasPrefix("fr") ?? true
}

/// Dates dans la langue choisie.
var wLocale: Locale { Locale(identifier: watchFrench ? "fr_FR" : "en_GB") }

/// Abréviation d'une séance (« EL1 », « Q », « Course »).
func shortSession(_ name: String) -> String {
    switch name {
    case "Essais libres 1", "Practice 1": return WL("EL1", "FP1")
    case "Essais libres 2", "Practice 2": return WL("EL2", "FP2")
    case "Essais libres 3", "Practice 3": return WL("EL3", "FP3")
    case "Qualifs sprint", "Sprint qualifying": return "SQ"
    case "Sprint": return "S"
    case "Qualifications", "Qualifying": return "Q"
    default: return WL("GP", "GP")
    }
}
func WL(_ fr: String, _ en: String) -> String { watchFrench ? fr : en }

struct WSession: Hashable, Codable {
    var name: String
    var date: Date
}

struct WRace: Hashable, Codable {
    var name: String
    var flag: String
    var round: String
    var circuit: String
    var start: Date
    var sessions: [WSession]

    var shortName: String { name.replacingOccurrences(of: " Grand Prix", with: "") }
    /// Prochaine séance pas encore terminée (ou la course).
    var nextSession: WSession? { sessions.first { $0.date.addingTimeInterval(3600) > Date() } }
}

struct WRow: Hashable, Codable {
    var position: String
    var name: String
    var code: String
    var team: String
    var value: String
}

struct WLastRace: Hashable, Codable {
    var name: String
    var flag: String
    var rows: [WRow]
}

func teamColor(_ id: String) -> Color {
    let map: [String: UInt32] = [
        "mercedes": 0x27F4D2, "ferrari": 0xE8002D, "red_bull": 0x3671C6, "mclaren": 0xFF8000,
        "aston_martin": 0x229971, "alpine": 0xFF87BC, "williams": 0x64C4FF, "rb": 0x6692FF,
        "haas": 0xB6BABD, "sauber": 0x52E252, "audi": 0xF50537, "cadillac": 0xC9A96E,
    ]
    let v = map[id] ?? 0x8A8A99
    return Color(red: Double(v >> 16 & 0xFF) / 255, green: Double(v >> 8 & 0xFF) / 255, blue: Double(v & 0xFF) / 255)
}

func countryFlag(_ country: String) -> String {
    let codes: [String: String] = [
        "Australia": "AU", "Austria": "AT", "Azerbaijan": "AZ", "Bahrain": "BH", "Belgium": "BE",
        "Brazil": "BR", "Canada": "CA", "China": "CN", "France": "FR", "Germany": "DE", "Hungary": "HU",
        "Italy": "IT", "Japan": "JP", "Malaysia": "MY", "Mexico": "MX", "Monaco": "MC", "Netherlands": "NL",
        "Portugal": "PT", "Qatar": "QA", "Saudi Arabia": "SA", "Singapore": "SG", "Spain": "ES",
        "UAE": "AE", "United Arab Emirates": "AE", "UK": "GB", "United Kingdom": "GB", "USA": "US",
        "United States": "US",
    ]
    guard let code = codes[country] else { return "🏁" }
    return code.unicodeScalars.compactMap { UnicodeScalar(127_397 + $0.value) }.map(String.init).joined()
}

/// « J-3 », « 5 h », « 12 min », « GO ».
func shortCountdown(_ date: Date, now: Date = Date()) -> String {
    let s = date.timeIntervalSince(now)
    if s <= 0 { return "GO" }
    if s < 3600 { return "\(Int(s / 60)) min" }
    if s < 86_400 { return "\(Int(s / 3600)) h" }
    return WL("J-\(Int(ceil(s / 86_400)))", "D-\(Int(ceil(s / 86_400)))")
}

enum WatchFeed {
    private static let server = URL(string: "https://f1x-29170430865f.herokuapp.com/")!
    private static let iso = ISO8601DateFormatter()

    private static func json(_ path: String) async -> [String: Any]? {
        guard let url = URL(string: "api/f1/\(path)", relativeTo: server) else { return nil }
        var request = URLRequest(url: url)
        request.timeoutInterval = 20
        guard let (data, _) = try? await URLSession.shared.data(for: request),
              let obj = try? JSONSerialization.jsonObject(with: data) as? [String: Any] else { return nil }
        return obj["MRData"] as? [String: Any]
    }

    private static func date(_ d: String?, _ t: String?) -> Date? {
        guard let d else { return nil }
        let time = t ?? "12:00:00Z"
        return iso.date(from: "\(d)T\(time.hasSuffix("Z") ? time : time + "Z")")
    }

    static func nextRace() async -> WRace? {
        guard let mr = await json("current.json?limit=100"),
              let races = (mr["RaceTable"] as? [String: Any])?["Races"] as? [[String: Any]] else { return nil }
        for r in races {
            guard let start = date(r["date"] as? String, r["time"] as? String),
                  start.addingTimeInterval(2 * 3600) > Date() else { continue }
            let circuit = r["Circuit"] as? [String: Any]
            let location = circuit?["Location"] as? [String: Any]
            let keys: [(String, String, String)] = [
                ("FirstPractice", "Essais libres 1", "Practice 1"),
                ("SecondPractice", "Essais libres 2", "Practice 2"),
                ("ThirdPractice", "Essais libres 3", "Practice 3"),
                ("SprintQualifying", "Qualifs sprint", "Sprint qualifying"),
                ("SprintShootout", "Qualifs sprint", "Sprint qualifying"),
                ("Sprint", "Sprint", "Sprint"),
                ("Qualifying", "Qualifications", "Qualifying"),
            ]
            var sessions: [WSession] = keys.compactMap { item in
                guard let s = r[item.0] as? [String: Any], let d = date(s["date"] as? String, s["time"] as? String) else { return nil }
                return WSession(name: WL(item.1, item.2), date: d)
            }
            sessions.append(WSession(name: WL("Course", "Race"), date: start))
            sessions.sort { $0.date < $1.date }
            return WRace(name: r["raceName"] as? String ?? "Grand Prix",
                         flag: countryFlag(location?["country"] as? String ?? ""),
                         round: r["round"] as? String ?? "",
                         circuit: location?["locality"] as? String ?? "",
                         start: start, sessions: sessions)
        }
        return nil
    }

    static func drivers(limit: Int = 22) async -> [WRow] {
        guard let mr = await json("current/driverStandings.json?limit=\(limit)"),
              let lists = (mr["StandingsTable"] as? [String: Any])?["StandingsLists"] as? [[String: Any]],
              let rows = lists.first?["DriverStandings"] as? [[String: Any]] else { return [] }
        return rows.map { s in
            let d = s["Driver"] as? [String: Any]
            let family = d?["familyName"] as? String ?? ""
            return WRow(position: s["position"] as? String ?? s["positionText"] as? String ?? "",
                        name: family,
                        code: d?["code"] as? String ?? String(family.prefix(3)).uppercased(),
                        team: ((s["Constructors"] as? [[String: Any]])?.last?["constructorId"] as? String) ?? "",
                        value: s["points"] as? String ?? "")
        }
    }

    static func teams() async -> [WRow] {
        guard let mr = await json("current/constructorStandings.json?limit=12"),
              let lists = (mr["StandingsTable"] as? [String: Any])?["StandingsLists"] as? [[String: Any]],
              let rows = lists.first?["ConstructorStandings"] as? [[String: Any]] else { return [] }
        return rows.map { s in
            let c = s["Constructor"] as? [String: Any]
            return WRow(position: s["position"] as? String ?? s["positionText"] as? String ?? "",
                        name: c?["name"] as? String ?? "", code: "",
                        team: c?["constructorId"] as? String ?? "", value: s["points"] as? String ?? "")
        }
    }

    static func lastRace() async -> WLastRace? {
        guard let mr = await json("current/last/results.json?limit=30"),
              let race = ((mr["RaceTable"] as? [String: Any])?["Races"] as? [[String: Any]])?.first,
              let results = race["Results"] as? [[String: Any]] else { return nil }
        let location = (race["Circuit"] as? [String: Any])?["Location"] as? [String: Any]
        let rows = results.map { r -> WRow in
            let d = r["Driver"] as? [String: Any]
            let family = d?["familyName"] as? String ?? ""
            let time = (r["Time"] as? [String: Any])?["time"] as? String ?? r["status"] as? String ?? ""
            return WRow(position: r["positionText"] as? String ?? "", name: family,
                        code: d?["code"] as? String ?? String(family.prefix(3)).uppercased(),
                        team: (r["Constructor"] as? [String: Any])?["constructorId"] as? String ?? "",
                        value: time)
        }
        return WLastRace(name: race["raceName"] as? String ?? "", flag: countryFlag(location?["country"] as? String ?? ""), rows: rows)
    }
}
