import ActivityKit
import Foundation

/// Live Activity « week-end de Grand Prix » : compte à rebours de la prochaine séance sur
/// l'écran verrouillé et dans la Dynamic Island. Le minuteur avance tout seul (aucune
/// mise à jour réseau nécessaire), l'app passe à la séance suivante dès qu'elle s'ouvre.
struct WeekendActivityAttributes: ActivityAttributes {
    struct ContentState: Codable, Hashable {
        /// « Essais libres 1 », « Qualifications », « Course »… (déjà traduit).
        var session: String
        /// Abréviation pour la Dynamic Island : « EL1 », « Q », « SQ », « S », « GP ».
        var short: String
        var start: Date
        /// Séance suivante (texte prêt à afficher), s'il y en a une.
        var next: String?
        /// Vainqueur du dernier Grand Prix (« M. Verstappen »), son code (« VER ») et la course.
        var winner: String?
        var winnerCode: String?
        var winnerRace: String?
        /// Vrai juste après l'arrivée : l'activité met le vainqueur à l'honneur 🏆.
        var podium: Bool?
    }

    var raceName: String
    var flag: String
    var round: String
    var circuit: String
}
