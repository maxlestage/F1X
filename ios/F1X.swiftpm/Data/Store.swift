import SwiftUI

// Préférences et jeux enregistrés sur l'appareil uniquement (aucun compte, aucun serveur).

enum FavoriteKind: String { case driver, team }

enum Favorites {
    static func key(_ kind: FavoriteKind) -> String { "f1x-fav-\(kind.rawValue)" }
    static func get(_ kind: FavoriteKind) -> String? { UserDefaults.standard.string(forKey: key(kind)) }
}

struct FavoriteButton: View {
    let kind: FavoriteKind
    let id: String
    @AppStorage private var current: String

    init(kind: FavoriteKind, id: String) {
        self.kind = kind
        self.id = id
        _current = AppStorage(wrappedValue: "", Favorites.key(kind))
    }

    var body: some View {
        let on = current == id
        Button {
            current = on ? "" : id
        } label: {
            Label(on ? L("Favori", "Favourite") : L("Ajouter aux favoris", "Add to favourites"), systemImage: on ? "star.fill" : "star")
        }
        .buttonStyle(.bordered)
        .tint(on ? .yellow : .primary)
    }
}

/// Pronostic de podium pour une course.
struct Prediction: Codable, Hashable {
    var season: String
    var round: String
    var raceName: String
    var p1: String
    var p2: String
    var p3: String
}

enum Predictions {
    private static let key = "f1x-predictions"

    static func all() -> [Prediction] {
        guard let data = UserDefaults.standard.data(forKey: key) else { return [] }
        return (try? JSONDecoder().decode([Prediction].self, from: data)) ?? []
    }

    static func save(_ p: Prediction) {
        var list = all().filter { !($0.season == p.season && $0.round == p.round) }
        list.append(p)
        if let data = try? JSONEncoder().encode(list) { UserDefaults.standard.set(data, forKey: key) }
    }

    /// 10 points par pilote à la bonne place, 3 s'il est sur le podium à une autre place.
    static func score(_ p: Prediction, results: [RaceResult]) -> Int {
        let podium = results.prefix(3).map(\.driver.driverId)
        guard podium.count == 3 else { return 0 }
        var total = 0
        for (i, pick) in [p.p1, p.p2, p.p3].enumerated() {
            if podium[i] == pick { total += 10 } else if podium.contains(pick) { total += 3 }
        }
        return total
    }
}

/// Équipe Fantasy : 5 pilotes + 1 écurie, budget 100 M€.
struct FantasyTeam: Codable {
    var drivers: [String] = []
    var team: String?

    private static let key = "f1x-fantasy"
    static func load() -> FantasyTeam {
        guard let data = UserDefaults.standard.data(forKey: key),
              let t = try? JSONDecoder().decode(FantasyTeam.self, from: data) else { return FantasyTeam() }
        return t
    }
    func save() {
        if let data = try? JSONEncoder().encode(self) { UserDefaults.standard.set(data, forKey: Self.key) }
    }
}
