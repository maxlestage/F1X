import ActivityKit
import Foundation

/// Live Activity d'une séance suivie (écran verrouillé et Dynamic Island), partagée entre
/// l'app (qui la démarre et la met à jour) et l'extension de widgets (qui l'affiche).
struct RaceActivityAttributes: ActivityAttributes {
    struct Entry: Codable, Hashable {
        var position: Int
        var code: String
        /// Couleur d'écurie (hexadécimal sans #).
        var colour: String
        var gap: String
    }

    struct ContentState: Codable, Hashable {
        var lap: Int
        var totalLaps: Int?
        /// « green », « yellow », « safety_car », « virtual_safety_car », « red », « chequered ».
        var status: String
        /// Les premiers du classement (3 au plus).
        var leaders: [Entry]
        var updated: Date
    }

    /// « Qualifying », « Race »…
    var sessionName: String
    var location: String
    /// Course ou sprint (compteur de tours) plutôt que qualifications / essais.
    var racing: Bool
}
