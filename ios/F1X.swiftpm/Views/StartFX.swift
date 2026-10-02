import AVFoundation
import CoreHaptics

/// Son et vibrations du démarrage : moteur V6 synthétisé (aucun fichier audio) qui monte
/// dans les tours à chaque feu rouge, tient le régime à l'orange et au vert, puis le départ
/// et deux passages de rapport. Les vibrations suivent exactement le régime du moteur.
/// Le son respecte le mode silencieux (catégorie « ambient ») ; les vibrations restent.
final class StartFX {
    /// Régime (tr/min) au fil du temps, calé sur l'animation : feux rouges à 0,07 s
    /// d'intervalle, orange 0,46 s, vert 0,62 s, départ 0,80 s, sortie 1,59 s.
    static let rpm: [(t: Double, v: Double)] = [
        (0, 3000), (0.07, 8500), (0.11, 6000), (0.14, 9000), (0.18, 6500), (0.21, 9500),
        (0.25, 7000), (0.28, 10000), (0.32, 7500), (0.35, 10500), (0.46, 11000), (0.62, 11800),
        (0.80, 12000), (0.84, 7500), (1.05, 12200), (1.09, 9000), (1.30, 12400), (1.34, 9500),
        (1.59, 12000), (1.90, 9000),
    ]
    static let duration = 1.9

    static func value(at t: Double) -> Double {
        guard let first = rpm.first, t > first.t else { return rpm.first?.v ?? 0 }
        for i in 1..<rpm.count where t <= rpm[i].t {
            let a = rpm[i - 1], b = rpm[i]
            let k = (t - a.t) / max(b.t - a.t, 0.001)
            return a.v + (b.v - a.v) * k
        }
        return rpm.last?.v ?? 0
    }

    private var audio: AVAudioEngine?
    private var haptics: CHHapticEngine?

    func play() {
        playSound()
        playHaptics()
    }

    func stop() {
        audio?.stop()
        audio = nil
        haptics?.stop()
        haptics = nil
    }

    // MARK: Son

    private func playSound() {
        let session = AVAudioSession.sharedInstance()
        try? session.setCategory(.ambient, options: [.mixWithOthers])
        try? session.setActive(true)
        let engine = AVAudioEngine()
        let format = engine.outputNode.inputFormat(forBus: 0)
        let rate = format.sampleRate > 0 ? format.sampleRate : 44_100
        guard let mono = AVAudioFormat(standardFormatWithSampleRate: rate, channels: 1) else { return }
        var frame = 0.0, phase = 0.0, noise: UInt32 = 0x1234_5678
        let source = AVAudioSourceNode(format: mono) { _, _, count, buffers -> OSStatus in
            let list = UnsafeMutableAudioBufferListPointer(buffers)
            guard let out = list.first?.mData?.assumingMemoryBound(to: Float.self) else { return noErr }
            for i in 0..<Int(count) {
                let t = frame / rate
                frame += 1
                // V6 quatre temps : 3 explosions par tour.
                let freq = StartFX.value(at: t) / 60 * 3
                phase += freq / rate
                if phase >= 1 { phase -= 1 }
                // Dent de scie (explosions) + sous-harmonique + souffle, saturés.
                let saw: Double = phase * 2 - 1
                let sub: Double = sin(phase * Double.pi) * 0.6
                noise = noise &* 1_664_525 &+ 1_013_904_223
                let hiss: Double = (Double(noise >> 8) / 16_777_216 - 0.5) * 0.25
                let raw: Double = tanh((saw * 0.8 + sub + hiss) * 1.8)
                // Fondu d'entrée, plus fort au départ, fondu de sortie.
                let launch: Double = t < 0.8 ? 0.7 : 1.0
                let fadeIn: Double = min(t / 0.04, 1)
                let fadeOut: Double = max(0, min(1, (StartFX.duration - t) / 0.35))
                out[i] = Float(raw * fadeIn * launch * fadeOut * 0.22)
            }
            for b in list.dropFirst() {
                b.mData?.copyMemory(from: out, byteCount: Int(count) * MemoryLayout<Float>.size)
            }
            return noErr
        }
        engine.attach(source)
        engine.connect(source, to: engine.mainMixerNode, format: mono)
        do {
            try engine.start()
            audio = engine
            DispatchQueue.main.asyncAfter(deadline: .now() + StartFX.duration + 0.1) { [weak self] in
                self?.audio?.stop()
                self?.audio = nil
            }
        } catch {
            audio = nil
        }
    }

    // MARK: Vibrations

    private func playHaptics() {
        guard CHHapticEngine.capabilitiesForHardware().supportsHaptics,
              let engine = try? CHHapticEngine() else { return }
        engine.playsHapticsOnly = true
        engine.isAutoShutdownEnabled = true
        var events: [CHHapticEvent] = []
        // Grondement continu du moteur.
        events.append(CHHapticEvent(eventType: .hapticContinuous, parameters: [
            CHHapticEventParameter(parameterID: .hapticIntensity, value: 1),
            CHHapticEventParameter(parameterID: .hapticSharpness, value: 0.5),
        ], relativeTime: 0, duration: StartFX.duration))
        // Coups secs : chaque feu rouge, l'orange, le vert, le départ et les passages de rapport.
        let taps: [(Double, Float, Float)] = [
            (0.07, 0.45, 0.8), (0.14, 0.5, 0.8), (0.21, 0.55, 0.8), (0.28, 0.6, 0.8), (0.35, 0.65, 0.8),
            (0.46, 0.7, 0.6), (0.62, 0.9, 0.7), (0.80, 1, 0.3), (0.84, 1, 0.2), (1.09, 0.85, 0.5), (1.34, 0.85, 0.5),
        ]
        for (t, i, s) in taps {
            events.append(CHHapticEvent(eventType: .hapticTransient, parameters: [
                CHHapticEventParameter(parameterID: .hapticIntensity, value: i),
                CHHapticEventParameter(parameterID: .hapticSharpness, value: s),
            ], relativeTime: t))
        }
        // Intensité et finesse du grondement qui suivent le régime (16 points max par courbe).
        func curves(_ keys: ArraySlice<(t: Double, v: Double)>) -> [CHHapticParameterCurve] {
            guard let start = keys.first?.t else { return [] }
            let level = { (v: Double) in Float(min(max((v - 3000) / 9500, 0), 1)) }
            let fade = { (t: Double) in Float(max(0, min(1, (StartFX.duration - t) / 0.35))) }
            let intensity = keys.map {
                CHHapticParameterCurve.ControlPoint(relativeTime: $0.t - start, value: (0.2 + 0.7 * level($0.v)) * fade($0.t))
            }
            let sharpness = keys.map {
                CHHapticParameterCurve.ControlPoint(relativeTime: $0.t - start, value: 0.15 + 0.6 * level($0.v))
            }
            return [
                CHHapticParameterCurve(parameterID: .hapticIntensityControl, controlPoints: intensity, relativeTime: start),
                CHHapticParameterCurve(parameterID: .hapticSharpnessControl, controlPoints: sharpness, relativeTime: start),
            ]
        }
        let keys = StartFX.rpm[...]
        let half = keys.count / 2
        let params = curves(keys.prefix(half + 1)) + curves(keys.suffix(keys.count - half))
        do {
            let pattern = try CHHapticPattern(events: events, parameterCurves: params)
            try engine.start()
            let player = try engine.makePlayer(with: pattern)
            try player.start(atTime: CHHapticTimeImmediate)
            haptics = engine
        } catch {
            haptics = nil
        }
    }
}
