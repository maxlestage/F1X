import SwiftUI
import UIKit
import UIKit.UIGestureRecognizerSubclass

// Les animations du site, côté iPhone : lignes qui arrivent en cascade, chiffres qui comptent,
// feux de départ pendant le chargement, onde rouge au toucher, reflets, graphiques et tracés qui
// se dessinent, signaux du direct. Tout s'arrête avec « Réduire les animations ».

// MARK: - Cascade

/// Rang dans la vague d'apparition en cours : les lignes qui arrivent ensemble se suivent ;
/// une ligne qui arrive seule (en défilant) apparaît sans attendre.
enum Cascade {
    private static var waveStart = Date.distantPast
    private static var rank = 0

    static func nextDelay() -> Double {
        let now = Date()
        if now.timeIntervalSince(waveStart) > 0.18 {
            waveStart = now
            rank = 0
        } else {
            rank += 1
        }
        return Double(min(rank, 10)) * 0.045
    }
}

private struct CascadeIn: ViewModifier {
    let dx: CGFloat
    let dy: CGFloat
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @State private var shown = false

    func body(content: Content) -> some View {
        content
            .opacity(shown ? 1 : 0)
            .offset(x: shown ? 0 : dx, y: shown ? 0 : dy)
            .onAppear {
                guard !shown else { return }
                if reduceMotion { shown = true; return }
                withAnimation(.spring(response: 0.55, dampingFraction: 0.86).delay(Cascade.nextDelay())) { shown = true }
            }
    }
}

extension View {
    /// Ligne de liste : glisse depuis la gauche, en cascade avec ses voisines.
    func cascadeIn() -> some View { modifier(CascadeIn(dx: -22, dy: 0)) }

    /// Bloc : monte, en cascade avec ses voisins.
    func cascadeUp() -> some View { modifier(CascadeIn(dx: 0, dy: 26)) }
}

// MARK: - Valeur animée

/// Vue qui reçoit une progression 0 → 1 interpolée image par image (tracés, compteurs).
struct ProgressReveal<Content: View>: View, Animatable {
    var progress: Double
    @ViewBuilder let content: (Double) -> Content

    var animatableData: Double {
        get { progress }
        set { progress = newValue }
    }

    var body: some View { content(progress) }
}

// MARK: - Chiffres qui comptent

/// Nombre qui compte depuis 0 à son apparition (« 390 », « +25 », « 12.5 »).
/// Tout autre texte (temps au tour, « – », années) s'affiche tel quel.
struct CountingText: View {
    let text: String
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @State private var progress: Double = 0

    var body: some View {
        Group {
            if let n = Self.parse(text) {
                ProgressReveal(progress: progress) { k in
                    Text(n.prefix + String(format: "%.\(n.decimals)f", n.value * k) + n.suffix)
                }
            } else {
                Text(text)
            }
        }
        .accessibilityLabel(text)
        .onAppear {
            guard progress == 0 else { return }
            if reduceMotion { progress = 1; return }
            withAnimation(.easeOut(duration: 1.1)) { progress = 1 }
        }
    }

    /// Préfixe (« + », « P »…), valeur, nombre de décimales, suffixe (« % », « pts »…).
    static func parse(_ s: String) -> (prefix: String, value: Double, decimals: Int, suffix: String)? {
        guard let first = s.firstIndex(where: \.isNumber),
              let last = s.lastIndex(where: \.isNumber) else { return nil }
        let number = String(s[first...last])
        let prefix = String(s[..<first]), suffix = String(s[s.index(after: last)...])
        guard number.allSatisfy({ $0.isNumber || $0 == "." }), let value = Double(number), value > 2,
              !prefix.contains(where: \.isNumber), !suffix.contains(where: \.isNumber) else { return nil }
        let parts = number.split(separator: ".", omittingEmptySubsequences: false)
        guard parts.count <= 2 else { return nil }
        // Une année seule (« 2008 ») ne compte pas.
        if parts.count == 1, number.count == 4, prefix.isEmpty, suffix.isEmpty, (1900...2099).contains(Int(value)) { return nil }
        return (prefix, value, parts.count == 2 ? parts[1].count : 0, suffix)
    }
}

// MARK: - Chargement

/// Chargement : les cinq feux de départ s'allument un à un, puis s'éteignent (« départ ! »).
struct StartLightsLoader: View {
    var label: String? = L("Chargement…", "Loading…")
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    var body: some View {
        VStack(spacing: 14) {
            TimelineView(.animation(minimumInterval: 1 / 20, paused: reduceMotion)) { context in
                let t = context.date.timeIntervalSinceReferenceDate.truncatingRemainder(dividingBy: 2.6) / 2.6
                let lit = reduceMotion ? 5 : (t < 0.86 ? min(5, Int(t / 0.12)) : 0)
                HStack(spacing: 10) {
                    ForEach(0..<5, id: \.self) { i in
                        Circle()
                            .fill(i < lit ? Color.f1Red : Color.f1Red.opacity(0.14))
                            .frame(width: 16, height: 16)
                            .shadow(color: i < lit ? Color.f1Red.opacity(0.85) : .clear, radius: 7)
                    }
                }
            }
            if let label {
                Text(label.uppercased())
                    .font(.caption2.weight(.medium))
                    .tracking(2)
                    .foregroundStyle(.secondary)
            }
        }
        .padding(.vertical, 24)
        .frame(maxWidth: .infinity)
        .accessibilityElement(children: .ignore)
        .accessibilityLabel(label ?? L("Chargement…", "Loading…"))
    }
}

// MARK: - Reflets, lueurs, petits mouvements

/// Reflet qui traverse la vue de temps en temps (boutons pleins, podium).
struct SweepShine: View {
    var color: Color = .white.opacity(0.45)
    var period: Double = 5
    /// Part de la période pendant laquelle le reflet passe.
    var active: Double = 0.28
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    var body: some View {
        if !reduceMotion {
            GeometryReader { geo in
                TimelineView(.animation(minimumInterval: 1 / 30)) { context in
                    let t = context.date.timeIntervalSinceReferenceDate.truncatingRemainder(dividingBy: period) / period
                    let k = CGFloat(min(t / active, 1))
                    LinearGradient(colors: [.clear, color, .clear], startPoint: .leading, endPoint: .trailing)
                        .frame(width: geo.size.width * 0.4, height: geo.size.height * 1.8)
                        .rotationEffect(.degrees(16))
                        .offset(x: -geo.size.width * 0.5 + k * geo.size.width * 1.6, y: -geo.size.height * 0.4)
                        .opacity(k < 1 ? 1 : 0)
                }
            }
            .allowsHitTesting(false)
            .accessibilityHidden(true)
        }
    }
}

/// Trait rouge qui file sous la barre de navigation à l'arrivée sur une page.
struct PageSweep: View {
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @State private var done = false

    var body: some View {
        GeometryReader { geo in
            LinearGradient(colors: [.clear, .f1Red, .clear], startPoint: .leading, endPoint: .trailing)
                .frame(width: geo.size.width * 0.45, height: 2)
                .offset(x: done ? geo.size.width * 1.1 : -geo.size.width * 0.5)
                .opacity(done ? 0 : 1)
        }
        .frame(height: 2)
        .allowsHitTesting(false)
        .accessibilityHidden(true)
        .onAppear {
            guard !reduceMotion, !done else { return }
            withAnimation(.timingCurve(0.2, 0.7, 0.1, 1, duration: 1.2)) { done = true }
        }
    }
}

/// Lueur qui respire autour d'un élément (secteur violet, drapeau, prochaine course).
private struct Glow: ViewModifier {
    let color: Color
    let active: Bool
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @State private var on = false

    func body(content: Content) -> some View {
        content
            .shadow(color: active ? color.opacity(on ? 0.95 : 0.2) : .clear, radius: on ? 6 : 1)
            .onAppear { start() }
            .onChange(of: active) { _, _ in start() }
    }

    private func start() {
        guard active, !reduceMotion, !on else { return }
        Task { @MainActor in
            withAnimation(.easeInOut(duration: 0.9).repeatForever(autoreverses: true)) { on = true }
        }
    }
}

/// Arrivée en roulant (pneus).
private struct SpinIn: ViewModifier {
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @State private var shown = false

    func body(content: Content) -> some View {
        content
            .rotationEffect(.degrees(shown ? 0 : -300))
            .scaleEffect(shown ? 1 : 0.3)
            .opacity(shown ? 1 : 0)
            .onAppear {
                guard !shown else { return }
                if reduceMotion { shown = true; return }
                withAnimation(.spring(response: 0.7, dampingFraction: 0.72).delay(0.15)) { shown = true }
            }
    }
}

/// Petit flottement continu (icône météo).
private struct Floating: ViewModifier {
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @State private var up = false

    func body(content: Content) -> some View {
        content
            .offset(y: up ? -4 : 0)
            .rotationEffect(.degrees(up ? -5 : 0))
            .onAppear {
                guard !reduceMotion, !up else { return }
                Task { @MainActor in
                    withAnimation(.easeInOut(duration: 2.2).repeatForever(autoreverses: true)) { up = true }
                }
            }
    }
}

/// Le graphique se dessine de gauche à droite (à l'apparition et quand ses données changent).
private struct DrawIn<T: Equatable>: ViewModifier {
    let trigger: T
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @State private var progress: CGFloat = 0

    func body(content: Content) -> some View {
        content
            .mask(alignment: .leading) {
                GeometryReader { geo in
                    Rectangle().frame(width: geo.size.width * progress)
                }
            }
            .onAppear { run() }
            .onChange(of: trigger) { _, _ in run() }
    }

    private func run() {
        if reduceMotion { progress = 1; return }
        progress = 0
        Task { @MainActor in
            withAnimation(.timingCurve(0.2, 0.7, 0.1, 1, duration: 1.4)) { progress = 1 }
        }
    }
}

/// Secousse « non » (mauvaise réponse) : deux allers-retours pendant que la valeur passe de n à n + 1.
struct ShakeEffect: GeometryEffect {
    var travel: CGFloat = 7
    var animatableData: CGFloat

    func effectValue(size: CGSize) -> ProjectionTransform {
        ProjectionTransform(CGAffineTransform(translationX: travel * sin(animatableData * .pi * 4), y: 0))
    }
}

extension View {
    func glow(_ color: Color, active: Bool = true) -> some View { modifier(Glow(color: color, active: active)) }
    func spinIn() -> some View { modifier(SpinIn()) }
    func floating() -> some View { modifier(Floating()) }
    func drawIn() -> some View { modifier(DrawIn(trigger: 0)) }
    func drawIn<T: Equatable>(trigger: T) -> some View { modifier(DrawIn(trigger: trigger)) }
}

// MARK: - Barres

/// Barre de progression qui se remplit à son apparition.
struct FillBar: View {
    let value: Double
    var color: Color = .f1Red
    var height: CGFloat = 6
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @State private var shown = false

    var body: some View {
        GeometryReader { geo in
            ZStack(alignment: .leading) {
                Capsule().fill(Color.chip.opacity(0.6))
                Capsule().fill(color)
                    .frame(width: geo.size.width * CGFloat(max(0, min(1, value))) * (shown ? 1 : 0))
            }
        }
        .frame(height: height)
        .onAppear {
            guard !shown else { return }
            if reduceMotion { shown = true; return }
            withAnimation(.timingCurve(0.2, 0.7, 0.1, 1, duration: 1.1).delay(0.2)) { shown = true }
        }
        .accessibilityHidden(true)
    }
}

// MARK: - Direct

/// Point « connecté » qui émet des ondes.
struct LiveDot: View {
    let on: Bool
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @State private var ping = false

    var body: some View {
        Circle()
            .fill(on ? Color.green : Color.gray)
            .frame(width: 10, height: 10)
            .background {
                if on && !reduceMotion {
                    Circle()
                        .fill(Color.green)
                        .scaleEffect(ping ? 2.8 : 1)
                        .opacity(ping ? 0 : 0.7)
                        .onAppear {
                            Task { @MainActor in
                                withAnimation(.easeOut(duration: 1.6).repeatForever(autoreverses: false)) { ping = true }
                            }
                        }
                        .onDisappear { ping = false }
                }
            }
    }
}

// MARK: - Onde au toucher

/// Onde rouge là où l'on touche l'écran (comme sur le site), sans jamais gêner les gestes de l'app.
struct TouchRipples: UIViewRepresentable {
    func makeUIView(context: Context) -> RippleHost { RippleHost() }
    func updateUIView(_ uiView: RippleHost, context: Context) {}
}

final class RippleHost: UIView {
    private var spy: TouchSpy?

    override func didMoveToWindow() {
        super.didMoveToWindow()
        isUserInteractionEnabled = false
        guard let window, spy == nil else { return }
        let g = TouchSpy(target: nil, action: nil)
        g.cancelsTouchesInView = false
        g.delaysTouchesBegan = false
        g.delaysTouchesEnded = false
        g.delegate = g
        g.onTap = { [weak window] point in
            guard let window, !UIAccessibility.isReduceMotionEnabled else { return }
            RippleHost.ripple(at: point, in: window)
        }
        window.addGestureRecognizer(g)
        spy = g
    }

    static func ripple(at point: CGPoint, in window: UIWindow) {
        let size: CGFloat = 16
        let ring = CAShapeLayer()
        ring.frame = CGRect(x: point.x - size / 2, y: point.y - size / 2, width: size, height: size)
        ring.path = UIBezierPath(ovalIn: ring.bounds).cgPath
        ring.fillColor = UIColor.clear.cgColor
        ring.strokeColor = UIColor(red: 225 / 255, green: 6 / 255, blue: 0, alpha: 1).cgColor
        ring.lineWidth = 1.5
        ring.shadowColor = ring.strokeColor
        ring.shadowRadius = 6
        ring.shadowOpacity = 0.8
        ring.shadowOffset = .zero
        ring.zPosition = 1000
        // Valeur finale : l'onde reste invisible une fois l'animation finie, puis disparaît.
        ring.opacity = 0
        window.layer.addSublayer(ring)

        CATransaction.begin()
        CATransaction.setCompletionBlock { ring.removeFromSuperlayer() }
        let scale = CABasicAnimation(keyPath: "transform.scale")
        scale.fromValue = 0.3
        scale.toValue = 5
        let fade = CABasicAnimation(keyPath: "opacity")
        fade.fromValue = 1
        fade.toValue = 0
        let group = CAAnimationGroup()
        group.animations = [scale, fade]
        group.duration = 0.6
        group.timingFunction = CAMediaTimingFunction(controlPoints: 0.2, 0.7, 0.1, 1)
        ring.add(group, forKey: "onde")
        CATransaction.commit()
    }
}

/// Repère les touchers brefs (sans glisser) partout dans la fenêtre. Il ne se déclenche jamais :
/// les boutons, listes et défilements reçoivent leurs touchers exactement comme avant.
final class TouchSpy: UIGestureRecognizer, UIGestureRecognizerDelegate {
    var onTap: ((CGPoint) -> Void)?
    private var start: CGPoint?
    private var began = Date()

    override func touchesBegan(_ touches: Set<UITouch>, with event: UIEvent) {
        super.touchesBegan(touches, with: event)
        guard touches.count == 1, start == nil, let touch = touches.first, let view else {
            state = .failed
            return
        }
        start = touch.location(in: view)
        began = Date()
    }

    override func touchesMoved(_ touches: Set<UITouch>, with event: UIEvent) {
        super.touchesMoved(touches, with: event)
        guard let start, let touch = touches.first, let view else { return }
        let p = touch.location(in: view)
        if hypot(p.x - start.x, p.y - start.y) > 10 { state = .failed }
    }

    override func touchesEnded(_ touches: Set<UITouch>, with event: UIEvent) {
        super.touchesEnded(touches, with: event)
        if let start, Date().timeIntervalSince(began) < 0.6 { onTap?(start) }
        state = .failed
    }

    override func touchesCancelled(_ touches: Set<UITouch>, with event: UIEvent) {
        super.touchesCancelled(touches, with: event)
        state = .failed
    }

    override func reset() {
        super.reset()
        start = nil
    }

    func gestureRecognizer(_ gestureRecognizer: UIGestureRecognizer,
                           shouldRecognizeSimultaneouslyWith otherGestureRecognizer: UIGestureRecognizer) -> Bool {
        true
    }
}
