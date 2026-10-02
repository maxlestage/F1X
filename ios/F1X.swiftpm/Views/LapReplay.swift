import SwiftUI

/// Replay d'un tour (fiche pilote) : la voiture parcourt le circuit au rythme réel du tour,
/// avec le tableau de bord synchronisé (vitesse, rapport, régime, accélérateur, frein, DRS).
struct LapReplayView: View {
    /// Télémétrie du tour (distance, temps, vitesse, rpm, gaz, frein, rapport, DRS, circuit).
    let tel: JSONValue
    let code: String
    let colour: String

    @State private var map: TrackMap?
    @State private var cumulative: [Double] = []
    @State private var playing = true
    @State private var speed = 1.0
    @State private var origin = Date()
    @State private var offset = 0.0

    private var times: [Double] { tel["time"].doubles }
    private var duration: Double { tel["lap_duration"].double ?? times.last ?? 1 }

    var body: some View {
        let ready = !times.isEmpty
        VStack(alignment: .leading, spacing: 10) {
            if ready {
                TimelineView(.animation(paused: !playing)) { ctx in
                    let e = elapsed(at: ctx.date)
                    let i = index(at: e)
                    VStack(alignment: .leading, spacing: 10) {
                        if let map {
                            TrackMapView(map: map, markers: [TrackMarker(key: "car", label: code, colour: colour, fraction: fraction(i, map))],
                                         showTelemetry: false)
                        }
                        dashboard(i, e)
                    }
                }
                controls
            } else {
                Text(L("Replay indisponible pour ce tour.", "Replay unavailable for this lap.")).font(.footnote).foregroundStyle(.secondary)
            }
        }
        .task(id: tel["circuit_id"].string) {
            let id = tel["circuit_id"].string
            guard !id.isEmpty, let m = try? await ServerAPI.shared.track(id) else { return }
            var cum = [0.0]
            for k in 1..<max(m.points.count, 1) {
                let a = m.points[k - 1], b = m.points[k]
                cum.append(cum[k - 1] + hypot(b.x - a.x, b.y - a.y))
            }
            cumulative = cum
            map = m
        }
        .onChange(of: tel) { _, _ in restart() }
    }

    // MARK: Lecture

    private func elapsed(at date: Date) -> Double {
        let e = offset + (playing ? date.timeIntervalSince(origin) * speed : 0)
        return e.truncatingRemainder(dividingBy: max(duration, 1))
    }

    private func index(at e: Double) -> Int {
        let t = times
        var lo = 0, hi = max(t.count - 1, 0)
        while lo < hi {
            let mid = (lo + hi) / 2
            if t[mid] < e { lo = mid + 1 } else { hi = mid }
        }
        return lo
    }

    /// Avancement (en temps du tour de référence de la carte) correspondant à la distance parcourue.
    private func fraction(_ i: Int, _ map: TrackMap) -> Double {
        let dist = tel["distance"].doubles
        guard let total = dist.last, total > 0, i < dist.count, let mapTotal = cumulative.last, mapTotal > 0,
              cumulative.count == map.points.count else { return 0 }
        let target = dist[i] / total * mapTotal
        let k = cumulative.firstIndex { $0 >= target } ?? (map.points.count - 1)
        return map.points[k].t / max(map.lap_time, 1)
    }

    private func restart() {
        origin = Date()
        offset = 0
        playing = true
    }

    private var controls: some View {
        HStack(spacing: 18) {
            Button {
                if playing { offset = elapsed(at: Date()) } else { origin = Date() }
                playing.toggle()
            } label: { Image(systemName: playing ? "pause.fill" : "play.fill").font(.title3) }
            Button { restart() } label: { Image(systemName: "backward.end.fill") }
            Spacer()
            Menu("×\(Int(speed))") {
                ForEach([1.0, 2.0, 4.0], id: \.self) { v in
                    Button("×\(Int(v))") {
                        offset = elapsed(at: Date())
                        origin = Date()
                        speed = v
                    }
                }
            }
            .font(.body.monospacedDigit().bold())
        }
        .buttonStyle(.borderless)
    }

    // MARK: Tableau de bord

    private func value(_ field: String, _ i: Int) -> Double {
        let v = tel[field].doubles
        return i < v.count ? v[i] : 0
    }

    private func dashboard(_ i: Int, _ e: Double) -> some View {
        let kmh = Int(value("speed", i)), gear = Int(value("gear", i)), rpm = value("rpm", i)
        let throttle = value("throttle", i), brake = value("brake", i), drs = value("drs", i)
        return VStack(alignment: .leading, spacing: 8) {
            HStack(alignment: .firstTextBaseline, spacing: 14) {
                HStack(alignment: .firstTextBaseline, spacing: 3) {
                    Text("\(kmh)").font(.system(size: 34, weight: .black).italic().monospacedDigit())
                    Text("km/h").font(.caption.bold()).foregroundStyle(.secondary)
                }
                VStack(spacing: 0) {
                    Text(gear == 0 ? "N" : "\(gear)").font(.system(size: 30, weight: .black).monospacedDigit())
                    Text(L("rapport", "gear")).font(.caption2).foregroundStyle(.secondary)
                }
                Spacer()
                VStack(alignment: .trailing, spacing: 2) {
                    Text(formatLap(e)).font(.headline.monospacedDigit())
                    Text("/ \(formatLap(duration))").font(.caption.monospacedDigit()).foregroundStyle(.secondary)
                }
            }
            .lineLimit(1)
            bar(L("Régime", "RPM"), "\(Int(rpm)) tr/min", min(rpm / 13_000, 1), LinearGradient(colors: [.green, .yellow, .red], startPoint: .leading, endPoint: .trailing))
            bar(L("Accélérateur", "Throttle"), "\(Int(throttle)) %", throttle / 100, LinearGradient(colors: [.green, .green], startPoint: .leading, endPoint: .trailing))
            HStack(spacing: 10) {
                Label(L("Frein", "Brake"), systemImage: "exclamationmark.octagon.fill")
                    .padding(.horizontal, 10).padding(.vertical, 4)
                    .background(brake > 0 ? Color.red : Color.secondary.opacity(0.15), in: Capsule())
                    .foregroundStyle(brake > 0 ? .white : .secondary)
                Text(drs >= 100 ? L("DRS ouvert", "DRS open") : drs >= 50 ? L("DRS autorisé", "DRS armed") : "DRS")
                    .padding(.horizontal, 10).padding(.vertical, 4)
                    .background(drs >= 100 ? Color.green : drs >= 50 ? Color.yellow.opacity(0.6) : Color.secondary.opacity(0.15), in: Capsule())
                    .foregroundStyle(drs >= 50 ? .black : .secondary)
            }
            .font(.caption.bold())
            .lineLimit(1)
        }
    }

    private func bar(_ title: String, _ text: String, _ ratio: Double, _ fill: LinearGradient) -> some View {
        VStack(alignment: .leading, spacing: 3) {
            HStack {
                Text(title).font(.caption.weight(.semibold)).foregroundStyle(.secondary)
                Spacer()
                Text(text).font(.caption.monospacedDigit())
            }
            GeometryReader { g in
                ZStack(alignment: .leading) {
                    Capsule().fill(Color.secondary.opacity(0.15))
                    Capsule().fill(fill).frame(width: g.size.width * max(0, min(ratio, 1)))
                }
            }
            .frame(height: 8)
        }
    }
}
