import SwiftUI

extension Color {
    static let f1Red = Color(red: 225 / 255, green: 6 / 255, blue: 0)
    static let f1Purple = Color(red: 168 / 255, green: 85 / 255, blue: 247 / 255)

    init(hex: UInt32) {
        self.init(
            red: Double((hex >> 16) & 0xFF) / 255,
            green: Double((hex >> 8) & 0xFF) / 255,
            blue: Double(hex & 0xFF) / 255
        )
    }
}

enum Team {
    static func color(_ constructorId: String?) -> Color {
        switch constructorId {
        case "mercedes": Color(hex: 0x27F4D2)
        case "ferrari": Color(hex: 0xE8002D)
        case "red_bull": Color(hex: 0x3671C6)
        case "mclaren": Color(hex: 0xFF8000)
        case "aston_martin": Color(hex: 0x229971)
        case "alpine": Color(hex: 0xFF87BC)
        case "williams": Color(hex: 0x64C4FF)
        case "rb": Color(hex: 0x6692FF)
        case "haas": Color(hex: 0xB6BABD)
        case "sauber": Color(hex: 0x52E252)
        case "audi": Color(hex: 0xF50537)
        case "cadillac": Color(hex: 0xC9A96E)
        default: .gray
        }
    }
}

enum Flag {
    static func country(_ country: String) -> String {
        let codes: [String: String] = [
            "Australia": "AU", "Austria": "AT", "Azerbaijan": "AZ", "Bahrain": "BH",
            "Belgium": "BE", "Brazil": "BR", "Canada": "CA", "China": "CN", "France": "FR",
            "Germany": "DE", "Hungary": "HU", "Italy": "IT", "Japan": "JP", "Malaysia": "MY",
            "Mexico": "MX", "Monaco": "MC", "Netherlands": "NL", "Portugal": "PT", "Qatar": "QA",
            "Saudi Arabia": "SA", "Singapore": "SG", "Spain": "ES", "UAE": "AE",
            "United Arab Emirates": "AE", "UK": "GB", "United Kingdom": "GB", "USA": "US",
            "United States": "US",
        ]
        return codes[country].map(emoji) ?? "🏁"
    }

    static func nationality(_ nationality: String?) -> String {
        let codes: [String: String] = [
            "American": "US", "Argentine": "AR", "Argentinian": "AR", "Australian": "AU",
            "Austrian": "AT", "Belgian": "BE", "Brazilian": "BR", "British": "GB",
            "Canadian": "CA", "Chinese": "CN", "Danish": "DK", "Dutch": "NL", "Finnish": "FI",
            "French": "FR", "German": "DE", "Italian": "IT", "Japanese": "JP", "Mexican": "MX",
            "Monegasque": "MC", "New Zealander": "NZ", "Polish": "PL", "Spanish": "ES",
            "Swiss": "CH", "Thai": "TH",
        ]
        return nationality.flatMap { codes[$0] }.map(emoji) ?? "🏳️"
    }

    private static func emoji(_ isoCode: String) -> String {
        var flag = ""
        for scalar in isoCode.unicodeScalars {
            if let regional = UnicodeScalar(127397 + scalar.value) {
                flag.unicodeScalars.append(regional)
            }
        }
        return flag
    }
}

extension Date {
    /// « ven. 2 oct. »
    var f1Day: String {
        formatted(.dateTime.weekday(.abbreviated).day().month(.abbreviated).locale(Locale(identifier: "fr_FR")))
    }

    /// « ven. 2 oct. · 10:30 » (heure locale de l'appareil)
    var f1DayTime: String {
        "\(f1Day) · \(formatted(.dateTime.hour(.twoDigits(amPM: .omitted)).minute(.twoDigits).locale(Locale(identifier: "fr_FR"))))"
    }
}
