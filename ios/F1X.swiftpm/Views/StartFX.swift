import AVFoundation
import CoreHaptics

/// Son et vibrations du démarrage : vrai enregistrement d'une Formule 1 (Ferrari F60 de 2009
/// au départ du Festival of Speed de Goodwood — « Ferrari F60 (2009) », Edvvc, Wikimedia
/// Commons, CC BY-SA 3.0, raccourci et mis en fondu). Coups de gaz pendant les feux rouges,
/// départ calé sur le passage au vert, moteur hurlant pendant l'arrivée du logo.
/// Les vibrations suivent le volume réel de l'enregistrement.
/// Le son respecte le mode silencieux (catégorie « ambient ») ; les vibrations restent.
final class StartFX {
    /// Volume de l'enregistrement toutes les 0,1 s (0 à 1), mesuré sur le fichier.
    static let envelope: [Float] = [
        0.06, 0.1, 0.14, 0.09, 0.06, 0.08, 0.11, 0.18, 0.47, 0.33, 0.38, 0.52, 0.51, 0.4, 0.42, 0.53,
        0.55, 0.57, 0.61, 0.7, 0.94, 0.77, 0.85, 0.74, 0.91, 0.82, 1.0, 0.79, 0.53, 0.41, 0.17, 0.04,
    ]
    static let step = 0.1

    private var player: AVAudioPlayer?
    private var haptics: CHHapticEngine?

    /// Garde le son en vie jusqu'à la fin, même une fois l'écran de démarrage disparu.
    private static var current: StartFX?

    func play() {
        StartFX.current = self
        playSound()
        playHaptics()
        let total = Double(StartFX.envelope.count) * StartFX.step
        DispatchQueue.main.asyncAfter(deadline: .now() + total + 0.3) { [weak self] in
            if StartFX.current === self { StartFX.current = nil }
        }
    }

    func stop() {
        player?.setVolume(0, fadeDuration: 0.25)
        let p = player
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.3) { p?.stop() }
        player = nil
        haptics?.stop()
        haptics = nil
        if StartFX.current === self { StartFX.current = nil }
    }

    // MARK: Son

    private func playSound() {
        guard let url = Bundle.main.url(forResource: "start_engine", withExtension: "wav") else { return }
        let session = AVAudioSession.sharedInstance()
        try? session.setCategory(.ambient, options: [.mixWithOthers])
        try? session.setActive(true)
        guard let p = try? AVAudioPlayer(contentsOf: url) else { return }
        p.volume = 1
        p.prepareToPlay()
        p.play()
        player = p
    }

    // MARK: Vibrations

    private func playHaptics() {
        guard CHHapticEngine.capabilitiesForHardware().supportsHaptics,
              let engine = try? CHHapticEngine() else { return }
        engine.playsHapticsOnly = true
        engine.isAutoShutdownEnabled = true
        let env = StartFX.envelope
        let total = Double(env.count) * StartFX.step
        var events: [CHHapticEvent] = []
        // Grondement continu du moteur, modulé par le volume de l'enregistrement.
        events.append(CHHapticEvent(eventType: .hapticContinuous, parameters: [
            CHHapticEventParameter(parameterID: .hapticIntensity, value: 1),
            CHHapticEventParameter(parameterID: .hapticSharpness, value: 0.45),
        ], relativeTime: 0, duration: total))
        // Coups secs : chaque feu rouge, l'orange, le vert et le départ.
        let taps: [(Double, Float, Float)] = [
            (0.07, 0.45, 0.8), (0.14, 0.5, 0.8), (0.21, 0.55, 0.8), (0.28, 0.6, 0.8), (0.35, 0.65, 0.8),
            (0.46, 0.7, 0.6), (0.62, 0.9, 0.7), (0.80, 1, 0.3), (0.84, 1, 0.2),
        ]
        for (t, i, s) in taps {
            events.append(CHHapticEvent(eventType: .hapticTransient, parameters: [
                CHHapticEventParameter(parameterID: .hapticIntensity, value: i),
                CHHapticEventParameter(parameterID: .hapticSharpness, value: s),
            ], relativeTime: t))
        }
        // Courbes d'intensité et de finesse (16 points au plus par courbe : découpage).
        var curves: [CHHapticParameterCurve] = []
        var start = 0
        while start < env.count {
            let end = min(start + 15, env.count - 1)
            let base = Double(start) * StartFX.step
            let idx = Array(start...end)
            let intensity = idx.map {
                CHHapticParameterCurve.ControlPoint(relativeTime: Double($0) * StartFX.step - base, value: 0.15 + 0.85 * env[$0])
            }
            let sharpness = idx.map {
                CHHapticParameterCurve.ControlPoint(relativeTime: Double($0) * StartFX.step - base, value: 0.2 + 0.6 * env[$0])
            }
            curves.append(CHHapticParameterCurve(parameterID: .hapticIntensityControl, controlPoints: intensity, relativeTime: base))
            curves.append(CHHapticParameterCurve(parameterID: .hapticSharpnessControl, controlPoints: sharpness, relativeTime: base))
            if end == env.count - 1 { break }
            start = end
        }
        do {
            let pattern = try CHHapticPattern(events: events, parameterCurves: curves)
            try engine.start()
            let hp = try engine.makePlayer(with: pattern)
            try hp.start(atTime: CHHapticTimeImmediate)
            haptics = engine
        } catch {
            haptics = nil
        }
    }
}
