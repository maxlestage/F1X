import SwiftUI

/// Race Center : direct (si disponible) et replay de n'importe quelle session depuis 2023,
/// reçus en temps réel par WebSocket depuis le serveur F1X.
struct LiveView: View {
    @StateObject private var client = LiveClient()
    @ObservedObject private var activity = LiveActivityManager.shared
    @State private var year = Calendar.current.component(.year, from: .now)
    @State private var sessions: [SessionSummary] = []
    @State private var speed = 10
    @State private var tab = 0
    @State private var map3D = false

    var body: some View {
        Group {
            if let snap = client.snapshot {
                board(snap)
            } else if let message = client.loading {
                VStack(spacing: 12) {
                    ProgressView()
                    Text(message).foregroundStyle(.secondary).multilineTextAlignment(.center)
                    Button(L("Annuler", "Cancel")) { client.stop() }
                }
                .padding()
                .frame(maxWidth: .infinity, maxHeight: .infinity)
            } else {
                lobby
            }
        }
        .navigationTitle(L("Direct", "Live"))
        .onAppear { client.connect() }
        // Live Activity en cours : la connexion reste ouverte pour continuer à la mettre à jour.
        .onDisappear { if !activity.running { client.disconnect() } }
        .onChange(of: client.snapshot?.clock) { _, _ in
            if let snap = client.snapshot { activity.update(snap) }
        }
        .task(id: year) { sessions = Array(((try? await ServerAPI.shared.sessions(year: year)) ?? []).filter { isPast($0) }.reversed()) }
    }

    private func isPast(_ s: SessionSummary) -> Bool {
        let f = ISO8601DateFormatter()
        f.formatOptions = [.withInternetDateTime, .withFractionalSeconds]
        let g = ISO8601DateFormatter()
        guard let end = f.date(from: s.date_end) ?? g.date(from: s.date_end) else { return true }
        return end < .now
    }

    // MARK: Accueil du direct

    private var lobby: some View {
        List {
            Section {
                HStack {
                    Circle().fill(client.connected ? Color.green : Color.gray).frame(width: 10, height: 10)
                    Text(client.connected ? L("Connecté au serveur F1X", "Connected to F1X") : L("Connexion…", "Connecting…"))
                    Spacer()
                    if client.viewers > 0 { Text("👀 \(client.viewers)").foregroundStyle(.secondary) }
                }
                if !client.status.isEmpty { Text(client.status).font(.footnote).foregroundStyle(.secondary) }
                if client.liveActive {
                    Button { client.followLive() } label: {
                        Label(L("Suivre la session en direct", "Follow the live session"), systemImage: "dot.radiowaves.left.and.right")
                    }
                    .buttonStyle(.borderedProminent).tint(.f1Red)
                }
                if let e = client.error { Text(e).foregroundStyle(.red).font(.footnote) }
            }
            Section(L("Rejouer une session", "Replay a session")) {
                Text(L("Revis n'importe quelle session depuis 2023 comme en direct : classement, écarts, pneus, arrêts, drapeaux, météo et direction de course.",
                       "Relive any session since 2023 as if it were live: order, gaps, tyres, pit stops, flags, weather and race control."))
                    .font(.footnote).foregroundStyle(.secondary)
                Picker(L("Année", "Year"), selection: $year) {
                    ForEach((2023...max(2023, Calendar.current.component(.year, from: .now))).reversed(), id: \.self) { Text(String($0)).tag($0) }
                }
                Picker(L("Vitesse", "Speed"), selection: $speed) {
                    ForEach([1, 5, 10, 30, 60], id: \.self) { Text("×\($0)").tag($0) }
                }
                .pickerStyle(.segmented)
                ForEach(sessions) { s in
                    Button { client.replay(s, speed: speed) } label: {
                        VStack(alignment: .leading, spacing: 2) {
                            Text("\(s.location) · \(s.session_name)").bold().foregroundStyle(.primary)
                            Text("\(s.country) · \(String(s.date_start.prefix(10)))").font(.footnote).foregroundStyle(.secondary)
                        }
                    }
                }
            }
        }
        .listStyle(.insetGrouped)
    }

    // MARK: Session en cours

    private func board(_ snap: Snapshot) -> some View {
        List {
            Section {
                VStack(alignment: .leading, spacing: 8) {
                    Text("\(snap.session.location) · \(snap.session.session_name)").font(.headline)
                    HStack {
                        Text(snap.total_laps.map { L("Tour \(snap.lap)/\($0)", "Lap \(snap.lap)/\($0)") } ?? L("Tour \(snap.lap)", "Lap \(snap.lap)"))
                            .font(.title3.bold().monospacedDigit())
                        Spacer()
                        Text(trackStatusLabel(snap.track_status))
                            .font(.caption.bold())
                            .padding(.horizontal, 8).padding(.vertical, 4)
                            .background(trackStatusColor(snap.track_status).opacity(0.25), in: Capsule())
                            .foregroundStyle(trackStatusColor(snap.track_status))
                    }
                    ProgressView(value: snap.progress).tint(.f1Red)
                    // Live Activity : classement sur l'écran verrouillé et dans la Dynamic Island.
                    if activity.available {
                        Button { activity.toggle(snap) } label: {
                            Label(activity.running ? L("Retirer de l'écran verrouillé", "Remove from Lock Screen")
                                                   : L("Suivre sur l'écran verrouillé", "Follow on Lock Screen"),
                                  systemImage: activity.running ? "lock.slash" : "lock.iphone")
                                .font(.footnote.bold())
                        }
                        .buttonStyle(.bordered)
                        .controlSize(.small)
                    }
                    if snap.mode == "replay" {
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
                        }
                        .buttonStyle(.borderless)
                    }
                    Button(role: .destructive) { client.stop() } label: { Text(L("Quitter", "Leave")).frame(maxWidth: .infinity) }
                        .buttonStyle(.bordered)
                }
            }
            Section {
                Picker("", selection: $tab) {
                    Text(L("Ordre", "Order")).tag(0)
                    Text(L("Carte", "Map")).tag(1)
                    Text(L("Pneus", "Tyres")).tag(2)
                    Text(L("Fil", "Feed")).tag(3)
                }
                .pickerStyle(.segmented)
            }
            switch tab {
            case 1: mapSection(snap)
            case 2: strategySection(snap)
            case 3: feedSection(snap)
            default: orderSection(snap)
            }
            if let w = snap.weather {
                Section(L("Météo", "Weather")) {
                    StatGrid(items: [
                        ("Air", String(format: "%.0f°", w.air_temperature)),
                        (L("Piste", "Track"), String(format: "%.0f°", w.track_temperature)),
                        (w.rainfall ? L("Pluie 🌧", "Rain 🌧") : L("Humidité", "Humidity"), String(format: "%.0f%%", w.humidity)),
                        (L("Vent", "Wind"), String(format: "%.0f km/h", w.wind_speed * 3.6) + (w.wind_direction.map { " " + WeatherText.compass($0) } ?? "")),
                        (L("Piste − air", "Track − air"), String(format: "%+.0f°", w.track_temperature - w.air_temperature)),
                        (L("Pression", "Pressure"), w.pressure.map { String(format: "%.0f hPa", $0) } ?? "–"),
                    ])
                }
            }
        }
        .listStyle(.insetGrouped)
    }

    @ViewBuilder
    private func orderSection(_ snap: Snapshot) -> some View {
        Section {
            ForEach(snap.cars) { car in
                HStack(spacing: 8) {
                    Text("\(car.position)").font(.headline.monospacedDigit()).frame(width: 26)
                    RoundedRectangle(cornerRadius: 2).fill(car.color).frame(width: 4, height: 34)
                    VStack(alignment: .leading, spacing: 2) {
                        HStack(spacing: 4) {
                            Text(car.code).bold()
                            if car.fastest { Text("⏱").foregroundStyle(Color.f1Purple) }
                            if car.in_pit { Text("PIT").font(.caption2.bold()).foregroundStyle(.yellow) }
                            if car.retired { Text(L("ABANDON", "OUT")).font(.caption2.bold()).foregroundStyle(.red) }
                        }
                        HStack(spacing: 3) {
                            ForEach(0..<3, id: \.self) { i in
                                Capsule()
                                    .fill(sectorColor(car.sector_flags.count > i ? car.sector_flags[i] : 0))
                                    .frame(width: 14, height: 4)
                            }
                            Text(car.last_lap.map(formatLap) ?? "").font(.caption2.monospacedDigit()).foregroundStyle(.secondary)
                        }
                    }
                    Spacer(minLength: 4)
                    VStack(alignment: .trailing, spacing: 2) {
                        Text(car.position == 1 ? L("Leader", "Leader") : car.interval).font(.footnote.monospacedDigit().bold())
                        Text(car.gap).font(.caption2.monospacedDigit()).foregroundStyle(.secondary)
                    }
                    ZStack {
                        Circle().stroke(tyreColor(car.compound), lineWidth: 3).frame(width: 24, height: 24)
                        Text(car.compound.map { String($0.prefix(1)) } ?? "?").font(.caption2.bold())
                    }
                    .overlay(alignment: .bottomTrailing) {
                        if let age = car.tyre_age { Text("\(age)").font(.system(size: 8).bold()).offset(x: 6, y: 4) }
                    }
                }
                .opacity(car.retired ? 0.45 : 1)
            }
        } footer: {
            Text(L("Secteurs : violet = meilleur de la session, vert = record personnel, jaune = plus lent. Pneu : S tendre, M medium, H dur, I intermédiaire, W pluie (chiffre = tours).",
                   "Sectors: purple = session best, green = personal best, yellow = slower. Tyre: S soft, M medium, H hard, I inter, W wet (number = laps)."))
        }
    }

    private func sectorColor(_ flag: Int) -> Color {
        switch flag {
        case 3: .f1Purple
        case 2: .green
        case 1: .yellow
        default: .gray.opacity(0.4)
        }
    }

    @ViewBuilder
    private func mapSection(_ snap: Snapshot) -> some View {
        Section {
            if let track = client.track {
                let markers = snap.cars.filter { $0.lap_progress != nil && !$0.retired }.map {
                    TrackMarker(key: $0.code, label: "\($0.position)", colour: $0.colour, fraction: $0.lap_progress ?? 0)
                }
                Picker("", selection: $map3D) {
                    Text(L("Plan 2D", "2D map")).tag(false)
                    Text(L("Relief 3D", "3D relief")).tag(true)
                }
                .pickerStyle(.segmented)
                if map3D {
                    Track3DView(map: track, markers: markers, ghost: false)
                } else {
                    TrackMapView(map: track, markers: markers, showTelemetry: false)
                }
                Text(L("Positions estimées d'après l'avancement de chaque pilote dans son tour (couleur = écurie, chiffre = position).",
                       "Positions estimated from each driver's progress through the lap (colour = team, number = position)."))
                    .font(.footnote).foregroundStyle(.secondary)
            } else {
                Text(L("Carte indisponible pour cette session.", "Map unavailable for this session.")).foregroundStyle(.secondary)
            }
        }
    }

    @ViewBuilder
    private func strategySection(_ snap: Snapshot) -> some View {
        let total = max(snap.total_laps ?? snap.lap, 1)
        Section {
            ForEach(snap.cars) { car in
                HStack(spacing: 8) {
                    Text(car.code).font(.caption.bold()).frame(width: 36, alignment: .leading)
                    GeometryReader { geo in
                        HStack(spacing: 2) {
                            ForEach(Array(car.stints.enumerated()), id: \.offset) { _, st in
                                RoundedRectangle(cornerRadius: 3)
                                    .fill(tyreColor(st.compound))
                                    .frame(width: max(2, geo.size.width * CGFloat(st.to - st.from + 1) / CGFloat(total) - 2))
                            }
                            Spacer(minLength: 0)
                        }
                    }
                    .frame(height: 12)
                    Text("\(car.pits)").font(.caption.monospacedDigit()).foregroundStyle(.secondary).frame(width: 18)
                }
            }
        } header: {
            Text(L("Relais de pneus", "Tyre stints"))
        } footer: {
            Text(snap.pit_loss.map { L(String(format: "Un arrêt coûte environ %.0f s. Chiffre = nombre d'arrêts.", $0), String(format: "A stop costs about %.0f s. Number = stops made.", $0)) } ?? "")
        }
    }

    @ViewBuilder
    private func feedSection(_ snap: Snapshot) -> some View {
        Section(L("Chronologie", "Timeline")) {
            if snap.events.isEmpty { Text(L("Rien pour l'instant.", "Nothing yet.")).foregroundStyle(.secondary) }
            ForEach(Array(snap.events.prefix(40).enumerated()), id: \.offset) { _, e in
                HStack(alignment: .top) {
                    Text(e.icon).frame(width: 24)
                    VStack(alignment: .leading) {
                        Text(e.text).fixedSize(horizontal: false, vertical: true)
                        Text(L("Tour \(e.lap)", "Lap \(e.lap)")).font(.caption2).foregroundStyle(.secondary)
                    }
                }
            }
        }
        Section(L("Direction de course", "Race control")) {
            ForEach(Array(snap.race_control.prefix(20).enumerated()), id: \.offset) { _, m in
                VStack(alignment: .leading, spacing: 2) {
                    Text(m.message).font(.footnote).fixedSize(horizontal: false, vertical: true)
                    Text([m.lap.map { L("Tour \($0)", "Lap \($0)") }, m.flag].compactMap { $0 }.joined(separator: " · "))
                        .font(.caption2).foregroundStyle(.secondary)
                }
            }
        }
    }
}
