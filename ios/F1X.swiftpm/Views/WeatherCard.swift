import Charts
import SwiftUI

// Météo du week-end : Open-Meteo (gratuit, sans clé). Conditions actuelles au circuit,
// jour par jour, séance par séance, évolution autour du départ et impact sur la course.

struct Forecast: Decodable {
    struct Hourly: Decodable {
        let time: [String]
        let temperature_2m: [Double?]?
        let apparent_temperature: [Double?]?
        let relative_humidity_2m: [Double?]?
        let dew_point_2m: [Double?]?
        let precipitation_probability: [Double?]?
        let precipitation: [Double?]?
        let weather_code: [Double?]?
        let cloud_cover: [Double?]?
        let pressure_msl: [Double?]?
        let wind_speed_10m: [Double?]?
        let wind_direction_10m: [Double?]?
        let wind_gusts_10m: [Double?]?
        let uv_index: [Double?]?
        let shortwave_radiation: [Double?]?
    }
    struct Daily: Decodable {
        let time: [String]
        let weather_code: [Double?]?
        let temperature_2m_max: [Double?]?
        let temperature_2m_min: [Double?]?
        let sunrise: [String?]?
        let sunset: [String?]?
        let precipitation_sum: [Double?]?
        let precipitation_probability_max: [Double?]?
        let wind_gusts_10m_max: [Double?]?
        let uv_index_max: [Double?]?
    }
    struct Current: Decodable {
        let time: String
        let temperature_2m: Double?
        let apparent_temperature: Double?
        let relative_humidity_2m: Double?
        let weather_code: Double?
        let cloud_cover: Double?
        let pressure_msl: Double?
        let wind_speed_10m: Double?
        let wind_direction_10m: Double?
        let wind_gusts_10m: Double?
        let precipitation: Double?
    }
    let hourly: Hourly
    let daily: Daily?
    let current: Current?
}

struct Hour {
    var date: Date
    var temp: Double?
    var feels: Double?
    var humidity: Double?
    var dew: Double?
    var rainProb: Double?
    var rainMM: Double?
    var code: Double?
    var cloud: Double?
    var pressure: Double?
    var wind: Double?
    var windDir: Double?
    var gusts: Double?
    var uv: Double?
    var radiation: Double?

    /// Piste estimée : l'asphalte chauffe au soleil bien au-delà de l'air.
    var track: Double? { temp.map { $0 + 1 + (radiation ?? 0) * 0.021 } }
}

private func utcDate(_ s: String) -> Date? {
    let f = ISO8601DateFormatter()
    f.formatOptions = [.withInternetDateTime]
    return f.date(from: s.count == 16 ? s + ":00Z" : s)
}

extension Forecast.Hourly {
    private func v(_ a: [Double?]?, _ i: Int) -> Double? {
        guard let a, i < a.count else { return nil }
        return a[i]
    }

    func hour(_ i: Int) -> Hour {
        Hour(
            date: utcDate(time[i]) ?? .distantPast,
            temp: v(temperature_2m, i), feels: v(apparent_temperature, i),
            humidity: v(relative_humidity_2m, i), dew: v(dew_point_2m, i),
            rainProb: v(precipitation_probability, i), rainMM: v(precipitation, i),
            code: v(weather_code, i), cloud: v(cloud_cover, i), pressure: v(pressure_msl, i),
            wind: v(wind_speed_10m, i), windDir: v(wind_direction_10m, i), gusts: v(wind_gusts_10m, i),
            uv: v(uv_index, i), radiation: v(shortwave_radiation, i)
        )
    }

    /// Heure pleine la plus proche.
    func at(_ date: Date) -> Hour? {
        let target = date.addingTimeInterval(1800)
        var cal = Calendar(identifier: .gregorian)
        cal.timeZone = TimeZone(identifier: "UTC")!
        let c = cal.dateComponents([.year, .month, .day, .hour], from: target)
        let key = String(format: "%04d-%02d-%02dT%02d:00", c.year ?? 0, c.month ?? 0, c.day ?? 0, c.hour ?? 0)
        guard let i = time.firstIndex(of: key) else { return nil }
        return hour(i)
    }
}

enum WeatherText {
    static func icon(_ code: Double?) -> String {
        switch Int(code ?? 999) {
        case 0: "☀️"
        case 1, 2: "🌤️"
        case 3: "☁️"
        case 45, 48: "🌫️"
        case 51...57: "🌦️"
        case 61...67, 80...82: "🌧️"
        case 71...77, 85, 86: "🌨️"
        case 95...99: "⛈️"
        default: "·"
        }
    }

    static func describe(_ code: Double?) -> String {
        switch Int(code ?? 999) {
        case 0: L("Ciel dégagé", "Clear sky")
        case 1: L("Plutôt dégagé", "Mainly clear")
        case 2: L("Partiellement nuageux", "Partly cloudy")
        case 3: L("Couvert", "Overcast")
        case 45, 48: L("Brouillard", "Fog")
        case 51...57: L("Bruine", "Drizzle")
        case 61: L("Pluie faible", "Light rain")
        case 63: L("Pluie", "Rain")
        case 65...67: L("Forte pluie", "Heavy rain")
        case 71...77, 85, 86: L("Neige", "Snow")
        case 80: L("Averses", "Showers")
        case 81, 82: L("Fortes averses", "Heavy showers")
        case 95...99: L("Orage", "Thunderstorm")
        default: "–"
        }
    }

    static func compass(_ deg: Double) -> String {
        let fr = ["N", "NE", "E", "SE", "S", "SO", "O", "NO"]
        let en = ["N", "NE", "E", "SE", "S", "SW", "W", "NW"]
        guard deg.isFinite else { return "" }
        let i = ((Int((deg.truncatingRemainder(dividingBy: 360) + 382.5) / 45) % 8) + 8) % 8
        return isFrench ? fr[i] : en[i]
    }

    static func deg(_ v: Double?) -> String { v.map { String(format: "%.0f°", $0) } ?? "–" }
    static func pct(_ v: Double?) -> String { v.map { String(format: "%.0f%%", $0) } ?? "–" }
    static func kmh(_ v: Double?) -> String { v.map { String(format: "%.0f km/h", $0) } ?? "–" }
}

/// Grille de valeurs (3 colonnes), toujours dans la largeur de l'écran.
struct StatGrid: View {
    let items: [(String, String)]
    var columns = 3

    var body: some View {
        LazyVGrid(columns: Array(repeating: GridItem(.flexible(), spacing: 8), count: columns), spacing: 8) {
            ForEach(Array(items.enumerated()), id: \.offset) { _, item in
                VStack(spacing: 2) {
                    Text(item.0.uppercased())
                        .font(.caption2.weight(.semibold))
                        .foregroundStyle(.secondary)
                        .lineLimit(2)
                        .multilineTextAlignment(.center)
                        .minimumScaleFactor(0.7)
                    Text(item.1)
                        .font(.headline.monospacedDigit())
                        .lineLimit(1)
                        .minimumScaleFactor(0.6)
                }
                .frame(maxWidth: .infinity)
                .padding(.vertical, 8)
                .background(Color.tile, in: RoundedRectangle(cornerRadius: 10))
            }
        }
    }
}

private func details(_ h: Hour) -> [(String, String)] {
    [
        (L("Air", "Air"), WeatherText.deg(h.temp)),
        (L("Ressenti", "Feels like"), WeatherText.deg(h.feels)),
        (L("Piste (est.)", "Track (est.)"), WeatherText.deg(h.track)),
        (L("Pluie", "Rain"), WeatherText.pct(h.rainProb)),
        (L("Cumul", "Amount"), h.rainMM.map { String(format: "%.1f mm", $0) } ?? "–"),
        (L("Humidité", "Humidity"), WeatherText.pct(h.humidity)),
        (L("Vent", "Wind"), h.wind.map { String(format: "%.0f km/h %@", $0, h.windDir.map(WeatherText.compass) ?? "") } ?? "–"),
        (L("Rafales", "Gusts"), WeatherText.kmh(h.gusts)),
        (L("Nuages", "Cloud"), WeatherText.pct(h.cloud)),
        (L("Rosée", "Dew point"), WeatherText.deg(h.dew)),
        (L("Pression", "Pressure"), h.pressure.map { String(format: "%.0f hPa", $0) } ?? "–"),
        ("UV", h.uv.map { String(format: "%.0f", $0) } ?? "–"),
    ]
}

private func impact(race: [Hour], quali: Hour?) -> [String] {
    guard !race.isEmpty else { return [] }
    let rain = race.compactMap(\.rainProb).max() ?? 0
    let mm = race.compactMap(\.rainMM).reduce(0, +)
    let heat = race.compactMap(\.temp).max() ?? 0
    let track = race.compactMap(\.track).max() ?? 0
    let coldTrack = race.compactMap(\.track).min() ?? 99
    let gusts = race.compactMap(\.gusts).max() ?? 0
    let humidity = race.compactMap(\.humidity).max() ?? 0
    var out: [String] = []
    if rain >= 60 || mm >= 1 {
        out.append(L(String(format: "🌧️ Pluie probable (%.0f %%, %.1f mm attendus) : pneus intermédiaires ou pluie, voiture de sécurité possible.", rain, mm),
                     String(format: "🌧️ Rain likely (%.0f%%, %.1f mm expected): intermediates or wets, possible safety car.", rain, mm)))
    } else if rain >= 30 {
        out.append(L(String(format: "🌦️ Risque d'averse (%.0f %%) : un changement de pneus au bon moment peut tout changer.", rain),
                     String(format: "🌦️ Chance of showers (%.0f%%): a well-timed tyre change could decide the race.", rain)))
    } else {
        out.append(L(String(format: "☀️ Course sèche attendue (pluie %.0f %%).", rain), String(format: "☀️ Dry race expected (rain %.0f%%).", rain)))
    }
    if track >= 45 || heat >= 30 {
        out.append(L(String(format: "🔥 Piste brûlante (≈ %.0f °C) : dégradation thermique des pneus, freins et moteurs à refroidir.", track),
                     String(format: "🔥 Scorching track (≈ %.0f°C): thermal tyre degradation, brakes and engines to cool.", track)))
    } else if coldTrack < 20 {
        out.append(L(String(format: "🥶 Piste fraîche (≈ %.0f °C) : pneus difficiles à mettre en température, risque de graining.", coldTrack),
                     String(format: "🥶 Cool track (≈ %.0f°C): tyres hard to warm up, risk of graining.", coldTrack)))
    }
    if gusts >= 40 {
        out.append(L(String(format: "💨 Rafales jusqu'à %.0f km/h : voitures instables au freinage et en courbe rapide.", gusts),
                     String(format: "💨 Gusts up to %.0f km/h: cars unsettled under braking and in fast corners.", gusts)))
    }
    if humidity >= 85 && rain < 60 {
        out.append(L(String(format: "💦 Air très humide (%.0f %%) : un peu moins de puissance, piste lente à sécher.", humidity),
                     String(format: "💦 Very humid air (%.0f%%): slightly less power, slow-drying track.", humidity)))
    }
    if let q = quali?.track, let r = race.first?.track, abs(r - q) >= 8 {
        out.append(L(String(format: "🌡️ Piste ≈ %.0f °C %@ qu'en qualifications : les réglages de samedi peuvent ne plus fonctionner.", abs(r - q), r > q ? "plus chaude" : "plus fraîche"),
                     String(format: "🌡️ Track ≈ %.0f°C %@ than in qualifying: Saturday's set-up may no longer work.", abs(r - q), r > q ? "hotter" : "cooler")))
    }
    return out
}

struct WeatherCard: View {
    let race: Race
    var full = true

    @State private var forecast: Forecast?
    @State private var failed = false
    @State private var picked: Date?

    private var inRange: Bool {
        guard let start = race.start, let first = race.sessions.first?.date else { return false }
        return start.addingTimeInterval(3 * 3600) > .now && first.timeIntervalSinceNow < 15 * 86400
    }

    var body: some View {
        if inRange, race.circuit.location.lat != nil {
            SectionCard(title: L("Météo du week-end", "Weekend weather")) {
                if let f = forecast {
                    content(f)
                } else if failed {
                    Text(L("Prévisions indisponibles.", "Forecast unavailable.")).foregroundStyle(.secondary)
                } else {
                    ProgressView().frame(maxWidth: .infinity)
                }
            }
            .task { await load() }
        }
    }

    @ViewBuilder
    private func content(_ f: Forecast) -> some View {
        let start = race.start ?? .now
        let raceHours = (0..<3).compactMap { f.hourly.at(start.addingTimeInterval(Double($0) * 3600)) }
        let quali = race.sessions.first { $0.name == "Qualifications" }.flatMap { f.hourly.at($0.date) }
        let shown = race.sessions.filter { full || $0.name == "Course" }

        if full, let c = f.current {
            VStack(alignment: .leading, spacing: 8) {
                HStack(spacing: 10) {
                    Text(WeatherText.icon(c.weather_code)).font(.largeTitle)
                    VStack(alignment: .leading) {
                        Text("\(WeatherText.deg(c.temperature_2m)) · \(WeatherText.describe(c.weather_code))").font(.headline)
                        Text(L("Maintenant au circuit", "Now at the circuit")).font(.caption).foregroundStyle(.secondary)
                    }
                }
                StatGrid(items: details(Hour(
                    date: .now, temp: c.temperature_2m, feels: c.apparent_temperature, humidity: c.relative_humidity_2m,
                    rainMM: c.precipitation, code: c.weather_code, cloud: c.cloud_cover, pressure: c.pressure_msl,
                    wind: c.wind_speed_10m, windDir: c.wind_direction_10m, gusts: c.wind_gusts_10m,
                    radiation: f.hourly.at(.now)?.radiation
                )))
            }
            .padding(12)
            .background(Color.tile, in: RoundedRectangle(cornerRadius: 12))
        }

        if full, let d = f.daily {
            Eyebrow(text: L("Jour par jour", "Day by day"))
            ForEach(Array(d.time.enumerated()), id: \.offset) { i, day in
                let g = { (a: [Double?]?) -> Double? in a.flatMap { i < $0.count ? $0[i] : nil } }
                let sunrise = d.sunrise.flatMap { i < $0.count ? $0[i] : nil }.flatMap(utcDate)
                let sunset = d.sunset.flatMap { i < $0.count ? $0[i] : nil }.flatMap(utcDate)
                VStack(alignment: .leading, spacing: 2) {
                    HStack {
                        Text("\(WeatherText.icon(g(d.weather_code))) \((utcDate(day + "T12:00") ?? .now).f1Day)").bold()
                        Spacer()
                        Text("\(WeatherText.deg(g(d.temperature_2m_min))) / \(WeatherText.deg(g(d.temperature_2m_max)))").bold().monospacedDigit()
                    }
                    Text("💧 \(WeatherText.pct(g(d.precipitation_probability_max))) · \(String(format: "%.1f", g(d.precipitation_sum) ?? 0)) mm · \(L("rafales", "gusts")) \(WeatherText.kmh(g(d.wind_gusts_10m_max))) · UV \(g(d.uv_index_max).map { String(format: "%.0f", $0) } ?? "–") · 🌅 \(sunrise?.formatted(Date.FormatStyle(date: .omitted, time: .shortened).locale(appLocale)) ?? "–") · 🌇 \(sunset?.formatted(Date.FormatStyle(date: .omitted, time: .shortened).locale(appLocale)) ?? "–")")
                        .font(.caption)
                        .foregroundStyle(.secondary)
                        .fixedSize(horizontal: false, vertical: true)
                }
                Divider()
            }
        }

        if full { Eyebrow(text: L("Séance par séance", "Session by session")) }
        ForEach(Array(shown.enumerated()), id: \.offset) { _, s in
            let h = f.hourly.at(s.date)
            DisclosureGroup {
                if let h {
                    Text(WeatherText.describe(h.code)).foregroundStyle(.secondary)
                    StatGrid(items: details(h))
                }
            } label: {
                VStack(alignment: .leading, spacing: 2) {
                    Text("\(sessionLabel(s.name)) · \(s.date.f1DayTime)").font(.subheadline.bold())
                    if let h {
                        Text("\(WeatherText.icon(h.code)) \(WeatherText.deg(h.temp)) · 💧\(WeatherText.pct(h.rainProb)) · 💨\(WeatherText.kmh(h.wind))")
                            .font(.footnote).foregroundStyle(.secondary)
                    } else {
                        Text(L("prévision pas encore disponible", "forecast not available yet")).font(.footnote).foregroundStyle(.secondary)
                    }
                }
            }
            .tint(.primary)
        }

        if full {
            let window = (-2...3).compactMap { f.hourly.at(start.addingTimeInterval(Double($0) * 3600)) }
            if window.count >= 3 {
                Eyebrow(text: L("Autour du départ", "Around the start"))
                if let p = picked, let h = window.min(by: { abs($0.date.timeIntervalSince(p)) < abs($1.date.timeIntervalSince(p)) }) {
                    Text("\(h.date.formatted(Date.FormatStyle(date: .omitted, time: .shortened).locale(appLocale))) · \(WeatherText.deg(h.temp)) · \(L("pluie", "rain")) \(WeatherText.pct(h.rainProb))")
                        .font(.subheadline.bold())
                } else {
                    Text(L("Touche un graphique pour lire les valeurs.", "Touch a chart to read the values.")).font(.caption).foregroundStyle(.secondary)
                }
                Text(L("Température de l'air (°C)", "Air temperature (°C)")).font(.caption).foregroundStyle(.secondary)
                Chart {
                    ForEach(window, id: \.date) { h in
                        if let t = h.temp {
                            LineMark(x: .value("Heure", h.date), y: .value("°C", t))
                                .foregroundStyle(Color(hex: 0xE5483F))
                            PointMark(x: .value("Heure", h.date), y: .value("°C", t))
                                .foregroundStyle(Color(hex: 0xE5483F))
                        }
                    }
                    RuleMark(x: .value("Départ", start)).foregroundStyle(Color.primary.opacity(0.4)).lineStyle(StrokeStyle(dash: [3, 3]))
                    if let p = picked { RuleMark(x: .value("Choix", p)).foregroundStyle(Color.primary.opacity(0.7)) }
                }
                .chartYScale(domain: .automatic(includesZero: false))
                .chartXSelection(value: $picked)
                .frame(height: 120)
                Text(L("Probabilité de pluie (%)", "Chance of rain (%)")).font(.caption).foregroundStyle(.secondary)
                Chart {
                    ForEach(window, id: \.date) { h in
                        BarMark(x: .value("Heure", h.date, unit: .hour), y: .value("%", h.rainProb ?? 0))
                            .foregroundStyle(Color(hex: 0x3B9FD8))
                            .cornerRadius(3)
                    }
                    RuleMark(x: .value("Départ", start)).foregroundStyle(Color.primary.opacity(0.4)).lineStyle(StrokeStyle(dash: [3, 3]))
                }
                .chartYScale(domain: 0...100)
                .chartXSelection(value: $picked)
                .frame(height: 100)
            }
        }

        let notes = impact(race: raceHours, quali: quali)
        if !notes.isEmpty {
            VStack(alignment: .leading, spacing: 6) {
                Text(L("Impact sur la course", "Race impact")).bold()
                ForEach(notes, id: \.self) { Text($0).font(.subheadline).fixedSize(horizontal: false, vertical: true) }
            }
            .padding(12)
            .frame(maxWidth: .infinity, alignment: .leading)
            .background(Color.tile, in: RoundedRectangle(cornerRadius: 12))
        }
        Text(L("Piste estimée d'après l'air et l'ensoleillement. Prévisions Open-Meteo.", "Track estimated from air and sunshine. Open-Meteo forecast."))
            .font(.caption2).foregroundStyle(.secondary)
    }

    private func load() async {
        guard forecast == nil, let lat = race.circuit.location.lat, let lon = race.circuit.location.long else { return }
        let first = race.sessions.first.map { String(Session.isoDay($0.date)) } ?? race.date
        let hourly = "temperature_2m,apparent_temperature,relative_humidity_2m,dew_point_2m,precipitation_probability,precipitation,weather_code,cloud_cover,pressure_msl,wind_speed_10m,wind_direction_10m,wind_gusts_10m,uv_index,shortwave_radiation"
        let daily = "weather_code,temperature_2m_max,temperature_2m_min,sunrise,sunset,precipitation_sum,precipitation_probability_max,wind_gusts_10m_max,uv_index_max"
        let current = "temperature_2m,apparent_temperature,relative_humidity_2m,weather_code,cloud_cover,pressure_msl,wind_speed_10m,wind_direction_10m,wind_gusts_10m,precipitation"
        let s = "https://api.open-meteo.com/v1/forecast?latitude=\(lat)&longitude=\(lon)&hourly=\(hourly)&daily=\(daily)&current=\(current)&timezone=UTC&start_date=\(first)&end_date=\(race.date)"
        guard let url = URL(string: s) else { return }
        do {
            let (data, _) = try await URLSession.shared.data(from: url)
            forecast = try JSONDecoder().decode(Forecast.self, from: data)
        } catch {
            failed = true
        }
    }
}

extension Session {
    /// « 2026-10-02 » (UTC).
    static func isoDay(_ date: Date) -> String {
        var cal = Calendar(identifier: .gregorian)
        cal.timeZone = TimeZone(identifier: "UTC")!
        let c = cal.dateComponents([.year, .month, .day], from: date)
        return String(format: "%04d-%02d-%02d", c.year ?? 0, c.month ?? 0, c.day ?? 0)
    }
}

/// Nom de séance dans la langue de l'appareil.
func sessionLabel(_ name: String) -> String {
    guard !isFrench else { return name }
    switch name {
    case "Essais libres 1": return "Practice 1"
    case "Essais libres 2": return "Practice 2"
    case "Essais libres 3": return "Practice 3"
    case "Qualifs sprint": return "Sprint qualifying"
    case "Qualifications": return "Qualifying"
    case "Course": return "Race"
    default: return name
    }
}
