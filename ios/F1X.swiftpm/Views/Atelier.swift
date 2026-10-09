import CoreText
import SwiftUI
import UIKit

// Habillage « atelier » (comme le site) : police Archivo à largeur variable, grain de film,
// lueur et ligne de vitesse, point qui pulse, apparitions au défilement. Rouge F1 conservé.

enum AtelierFont {
    /// Enregistre Archivo (police libre OFL, embarquée) au lancement.
    static func register() {
        guard let url = Bundle.main.url(forResource: "Archivo", withExtension: "ttf") else { return }
        CTFontManagerRegisterFontsForURL(url as CFURL, .process, nil)
    }

    /// Titres de barre de navigation en Archivo condensé.
    static func styleNavigationBars() {
        let bar = UINavigationBar.appearance()
        bar.largeTitleTextAttributes = [.font: UIFont.archivo(34, weight: 800, width: 86)]
        bar.titleTextAttributes = [.font: UIFont.archivo(17, weight: 700, width: 92)]
    }
}

extension UIFont {
    /// Archivo avec épaisseur (100–900) et largeur (62 étroit – 125 large) ; police système si absente.
    static func archivo(_ size: CGFloat, weight: CGFloat = 700, width: CGFloat = 100) -> UIFont {
        let axes: [NSNumber: CGFloat] = [
            NSNumber(value: 0x7767_6874): weight, // 'wght'
            NSNumber(value: 0x7764_7468): width,  // 'wdth'
        ]
        let descriptor = UIFontDescriptor(fontAttributes: [
            .name: "Archivo-SemiBold",
            UIFontDescriptor.AttributeName(rawValue: kCTFontVariationAttribute as String): axes,
        ])
        let font = UIFont(descriptor: descriptor, size: size)
        return font.familyName.hasPrefix("Archivo") ? font : .systemFont(ofSize: size, weight: weight >= 700 ? .heavy : weight <= 350 ? .light : .semibold)
    }
}

extension Font {
    static func archivo(_ size: CGFloat, weight: CGFloat = 700, width: CGFloat = 100) -> Font {
        Font(UIFont.archivo(size, weight: weight, width: width))
    }
}

// MARK: - Grain de film

/// Grain animé très léger posé sur toute l'app (ne capte aucun toucher).
struct GrainOverlay: View {
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    private static let tile: UIImage = {
        let size = 128
        let renderer = UIGraphicsImageRenderer(size: CGSize(width: size, height: size))
        return renderer.image { ctx in
            var generator = SystemRandomNumberGenerator()
            for y in 0..<size {
                for x in 0..<size {
                    let v = CGFloat(UInt8.random(in: 0...255, using: &generator)) / 255
                    ctx.cgContext.setFillColor(UIColor(white: v, alpha: 1).cgColor)
                    ctx.cgContext.fill(CGRect(x: x, y: y, width: 1, height: 1))
                }
            }
        }
    }()

    private static let jitter: [CGSize] = [
        .init(width: 0, height: 0), .init(width: -9, height: 7), .init(width: 6, height: -11),
        .init(width: -4, height: 13), .init(width: 11, height: 4), .init(width: -12, height: -6),
    ]

    var body: some View {
        TimelineView(.periodic(from: .now, by: reduceMotion ? 3600 : 0.15)) { context in
            let i = Int(context.date.timeIntervalSinceReferenceDate / 0.15) % Self.jitter.count
            Rectangle()
                .fill(ImagePaint(image: Image(uiImage: Self.tile)))
                .padding(-40)
                .offset(reduceMotion ? .zero : Self.jitter[i])
        }
        .opacity(0.045)
        .allowsHitTesting(false)
        .ignoresSafeArea()
        .accessibilityHidden(true)
    }
}

// MARK: - Lueur et ligne de vitesse

/// Lueur rouge qui dérive lentement et ligne de vitesse qui balaie la carte.
struct HeroGlow: View {
    var color: Color = .f1Red
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @State private var drift = false

    var body: some View {
        GeometryReader { geo in
            let w = geo.size.width, h = geo.size.height
            ZStack(alignment: .topLeading) {
                Circle()
                    .fill(RadialGradient(colors: [color.opacity(0.42), .clear], center: .center, startRadius: 0, endRadius: w * 0.45))
                    .frame(width: w * 0.9, height: w * 0.9)
                    .blur(radius: 30)
                    .offset(x: drift ? w * 0.35 : -w * 0.35, y: drift ? h * 0.35 : -w * 0.4)
                if !reduceMotion {
                    TimelineView(.animation(minimumInterval: 1 / 30)) { context in
                        let t = context.date.timeIntervalSinceReferenceDate.truncatingRemainder(dividingBy: 5.5) / 5.5
                        let k = min(t / 0.3, 1)
                        Capsule()
                            .fill(LinearGradient(colors: [.clear, color, .clear], startPoint: .leading, endPoint: .trailing))
                            .frame(width: w * 0.4, height: 2)
                            .offset(x: -w * 0.4 + k * w * 1.6, y: h * 0.32)
                            .opacity(k < 1 ? 0.9 : 0)
                    }
                }
            }
        }
        .allowsHitTesting(false)
        .onAppear {
            guard !reduceMotion else { return }
            withAnimation(.easeInOut(duration: 12).repeatForever(autoreverses: true)) { drift = true }
        }
    }
}

/// Point rouge lumineux qui pulse (devant les petits titres).
struct PulsingDot: View {
    var color: Color = .f1Red
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @State private var pulse = false

    var body: some View {
        Circle()
            .fill(color)
            .frame(width: 7, height: 7)
            .shadow(color: color, radius: pulse ? 2 : 7)
            .scaleEffect(pulse ? 0.7 : 1)
            .onAppear {
                guard !reduceMotion else { return }
                withAnimation(.easeInOut(duration: 1.2).repeatForever(autoreverses: true)) { pulse = true }
            }
    }
}

// MARK: - Apparitions

/// Apparition à l'arrivée à l'écran : monte, se précise.
private struct Rise: ViewModifier {
    let delay: Double
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @State private var shown = false

    func body(content: Content) -> some View {
        content
            .opacity(shown ? 1 : 0)
            .offset(y: shown ? 0 : 18)
            .blur(radius: shown ? 0 : 5)
            .onAppear {
                if reduceMotion { shown = true; return }
                withAnimation(.spring(response: 0.7, dampingFraction: 0.86).delay(delay)) { shown = true }
            }
    }
}

extension View {
    /// Monte et apparaît (avec un petit retard pour enchaîner plusieurs éléments).
    func rise(delay: Double = 0) -> some View { modifier(Rise(delay: delay)) }
}

/// Bouton qui s'enfonce sous le doigt.
struct PressableStyle: ButtonStyle {
    func makeBody(configuration: Configuration) -> some View {
        configuration.label
            .scaleEffect(configuration.isPressed ? 0.96 : 1)
            .brightness(configuration.isPressed ? -0.06 : 0)
            .animation(.spring(response: 0.3, dampingFraction: 0.6), value: configuration.isPressed)
    }
}
