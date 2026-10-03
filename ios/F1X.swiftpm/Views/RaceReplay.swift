import SwiftUI

/// Replay automatique d'une course terminée (2023 et après) sur la page du Grand Prix :
/// démarre tout seul, voitures à leurs vraies positions tour après tour sur le circuit 3D,
/// classement en direct, pause, avance/retour et vitesse.
struct RaceReplayCard: View {
    let year: Int
    let date: String
    @StateObject private var client = LiveClient()
    @State private var key: Int?
    @State private var searched = false
    @State private var started = false
    /// Séances du week-end qu'on peut rejouer (course, qualifs, sprint…) : (nom, clé).
    @State private var sessions: [(String, Int)] = []

    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            if sessions.count > 1 {
                Picker(L("Séance", "Session"), selection: Binding(get: { key ?? 0 }, set: { pick($0) })) {
                    ForEach(sessions, id: \.1) { Text(Self.label($0.0)).tag($0.1) }
                }
                .pickerStyle(.segmented)
            }
            if let snap = client.snapshot {
                header(snap)
                if let track = client.track {
                    let markers = snap.cars.filter { $0.lap_progress != nil && !$0.retired }.map {
                        TrackMarker(key: $0.code, label: "\($0.position) \($0.code)", colour: $0.colour, fraction: $0.lap_progress ?? 0)
                    }
                    Track3DView(map: track, markers: markers, ghost: false)
                }
                controls(snap)
                order(snap)
            } else if let loading = client.loading {
                HStack { ProgressView(); Text(loading).font(.footnote).foregroundStyle(.secondary) }
            } else if searched && key == nil {
                Text(L("Pas de replay disponible pour cette course.", "No replay available for this race."))
                    .font(.footnote).foregroundStyle(.secondary)
            } else {
                ProgressView().frame(maxWidth: .infinity)
            }
        }
        .task {
            defer { searched = true }
            if key == nil, year >= 2023, let race = of1Date("\(date)T12:00:00Z") {
                // Séances du week-end : course, qualifications, sprint et qualifs sprint.
                let order = ["Race", "Qualifying", "Sprint", "Sprint Qualifying", "Sprint Shootout"]
                let list = await of1("sessions", "year=\(year)", ttl: 3600)
                sessions = list.compactMap { s -> (String, Int)? in
                    guard let start = of1Date(s["date_start"].string), let k = s["session_key"].int,
                          order.contains(s["session_name"].string),
                          race.timeIntervalSince(start) < 4 * 86_400, start.timeIntervalSince(race) < 2 * 86_400 else { return nil }
                    return (s["session_name"].string, k)
                }
                .sorted { (order.firstIndex(of: $0.0) ?? 9) < (order.firstIndex(of: $1.0) ?? 9) }
                key = sessions.first?.1
            }
            // (Re)démarre à chaque affichage : la ligne de liste qui sort de l'écran coupe le replay.
            if let key, !started {
                started = true
                client.connect()
                client.replay(key: key, speed: 30)
            }
        }
        .onDisappear {
            client.stop()
            client.disconnect()
            started = false
        }
    }

    private func pick(_ k: Int) {
        guard k != key else { return }
        key = k
        client.replay(key: k, speed: 30)
    }

    /// « 2025-03-15T05:12:34Z » → heure locale « 06:12:34 ».
    static func clock(_ iso: String) -> String {
        guard let d = of1Date(iso) else { return "" }
        return d.formatted(.dateTime.hour(.twoDigits(amPM: .omitted)).minute(.twoDigits).second(.twoDigits))
    }

    static func label(_ name: String) -> String {
        switch name {
        case "Race": return L("Course", "Race")
        case "Qualifying": return L("Qualifs", "Quali")
        case "Sprint": return "Sprint"
        default: return L("Qualifs sprint", "Sprint quali")
        }
    }

    private func header(_ snap: Snapshot) -> some View {
        VStack(alignment: .leading, spacing: 6) {
            HStack {
                // Qualifications : l'heure de la séance plutôt qu'un compteur de tours.
                let racing = ["Race", "Sprint"].contains(sessions.first { $0.1 == key }?.0 ?? "Race")
                Text(racing ? (snap.total_laps.map { L("Tour \(snap.lap)/\($0)", "Lap \(snap.lap)/\($0)") } ?? L("Tour \(snap.lap)", "Lap \(snap.lap)"))
                            : "\(Self.label(sessions.first { $0.1 == key }?.0 ?? "")) · \(Self.clock(snap.clock))")
                    .font(.headline.monospacedDigit())
                Spacer()
                Text(trackStatusLabel(snap.track_status))
                    .font(.caption.bold())
                    .lineLimit(1)
                    .padding(.horizontal, 8).padding(.vertical, 3)
                    .background(trackStatusColor(snap.track_status).opacity(0.25), in: Capsule())
                    .foregroundStyle(trackStatusColor(snap.track_status))
            }
            ProgressView(value: snap.progress).tint(.f1Red)
        }
    }

    private func controls(_ snap: Snapshot) -> some View {
        HStack {
            Button { client.send(["type": "seek", "seconds": -120]) } label: { Image(systemName: "gobackward.120") }
            Spacer()
            Button { client.send(["type": snap.paused ? "resume" : "pause"]) } label: {
                Image(systemName: snap.paused ? "play.fill" : "pause.fill").font(.title2)
            }
            Spacer()
            Button { client.send(["type": "seek", "seconds": 120]) } label: { Image(systemName: "goforward.120") }
            Spacer()
            Menu("×\(snap.speed)") {
                ForEach([1, 5, 10, 30, 60], id: \.self) { v in
                    Button("×\(v)") { client.send(["type": "speed", "speed": v]) }
                }
            }
            .font(.body.monospacedDigit().bold())
        }
        .buttonStyle(.borderless)
    }

    /// Classement du moment (10 premiers), une ligne par pilote.
    private func order(_ snap: Snapshot) -> some View {
        VStack(alignment: .leading, spacing: 4) {
            ForEach(snap.cars.sorted { $0.position < $1.position }.prefix(10)) { c in
                HStack(spacing: 8) {
                    Text("\(c.position)").font(.footnote.monospacedDigit().bold()).frame(width: 22, alignment: .trailing)
                    Rectangle().fill(Color(hexString: c.colour)).frame(width: 3, height: 16)
                    Text(c.name).font(.footnote.weight(.semibold)).lineLimit(1)
                    Spacer(minLength: 4)
                    if c.in_pit { Text("PIT").font(.caption2.bold()).foregroundStyle(.yellow) }
                    Text(c.position == 1 ? L("Leader", "Leader") : c.gap)
                        .font(.caption.monospacedDigit()).foregroundStyle(.secondary).lineLimit(1)
                }
            }
        }
    }
}
