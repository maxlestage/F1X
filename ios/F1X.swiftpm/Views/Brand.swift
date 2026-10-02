import SwiftUI

// Identité animée de F1X : écran de démarrage (feux de départ + logo) et logo propre à
// chaque rubrique, animé à chaque changement de page. Désactivé si « Réduire les
// animations » est activé.

/// Rubriques (onglets) de l'app, chacune avec sa couleur et son pictogramme.
enum AppSection: Int, CaseIterable, Hashable {
    case home, live, calendar, standings, explore

    var label: String {
        switch self {
        case .home: return L("Accueil", "Home")
        case .live: return L("Direct", "Live")
        case .calendar: return L("Calendrier", "Calendar")
        case .standings: return L("Classements", "Standings")
        case .explore: return L("Explorer", "Explore")
        }
    }

    var symbol: String {
        switch self {
        case .home: return "house.fill"
        case .live: return "dot.radiowaves.left.and.right"
        case .calendar: return "calendar"
        case .standings: return "trophy.fill"
        case .explore: return "safari.fill"
        }
    }

    /// Couleur sombre et claire du dégradé.
    var colors: (Color, Color) {
        switch self {
        case .home: return (Color(hex: 0xE10600), Color(hex: 0xFF5A3C))
        case .live: return (Color(hex: 0x16A34A), Color(hex: 0x4ADE80))
        case .calendar: return (Color(hex: 0x2563EB), Color(hex: 0x60A5FA))
        case .standings: return (Color(hex: 0xC99400), Color(hex: 0xFCD34D))
        case .explore: return (Color(hex: 0x7C3AED), Color(hex: 0xC084FC))
        }
    }
}

/// Parallélogramme arrondi (écusson incliné « vitesse »).
struct Slanted: Shape {
    var slant: CGFloat = 0.25
    var radius: CGFloat = 6

    func path(in rect: CGRect) -> Path {
        let dx = rect.height * slant
        var p = Path()
        p.move(to: CGPoint(x: rect.minX + dx, y: rect.minY))
        p.addLine(to: CGPoint(x: rect.maxX, y: rect.minY))
        p.addLine(to: CGPoint(x: rect.maxX - dx, y: rect.maxY))
        p.addLine(to: CGPoint(x: rect.minX, y: rect.maxY))
        p.closeSubpath()
        return p
    }
}

/// Logo de rubrique : écusson incliné dégradé + pictogramme, traînée de vitesse et reflet.
struct SectionBadge: View {
    let section: AppSection
    var size: CGFloat = 26

    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @State private var shown = false
    @State private var shine = false

    var body: some View {
        let (dark, light) = section.colors
        HStack(spacing: size * 0.12) {
            // Traînée de vitesse.
            VStack(alignment: .trailing, spacing: size * 0.16) {
                Capsule().fill(light.opacity(0.55)).frame(width: size * 0.5, height: size * 0.08)
                Capsule().fill(LinearGradient(colors: [.clear, light], startPoint: .leading, endPoint: .trailing))
                    .frame(width: size * 0.85, height: size * 0.12)
                Capsule().fill(light.opacity(0.35)).frame(width: size * 0.4, height: size * 0.08)
            }
            .opacity(shown ? 0 : 1)
            .offset(x: shown ? -size * 0.6 : 0)
            ZStack {
                Slanted(radius: size * 0.2)
                    .fill(LinearGradient(colors: [light, dark], startPoint: .topLeading, endPoint: .bottomTrailing))
                    .shadow(color: dark.opacity(0.5), radius: size * 0.3, y: size * 0.08)
                Image(systemName: section.symbol)
                    .font(.system(size: size * 0.52, weight: .bold))
                    .foregroundStyle(.white)
                    .scaleEffect(shown ? 1 : 0.2)
                    .rotationEffect(.degrees(shown ? 0 : -30))
                // Reflet qui traverse l'écusson.
                LinearGradient(colors: [.clear, .white.opacity(0.75), .clear], startPoint: .leading, endPoint: .trailing)
                    .frame(width: size * 0.5)
                    .offset(x: shine ? size * 1.2 : -size * 1.2)
                    .mask(Slanted(radius: size * 0.2))
            }
            .frame(width: size * 1.35, height: size)
            .offset(x: shown ? 0 : -size)
            .opacity(shown ? 1 : 0)
        }
        .accessibilityHidden(true)
        .onAppear { animate() }
        .onChange(of: section) { _, _ in animate() }
    }

    private func animate() {
        guard !reduceMotion else {
            shown = true
            return
        }
        shown = false
        shine = false
        withAnimation(.spring(response: 0.45, dampingFraction: 0.62)) { shown = true }
        withAnimation(.easeOut(duration: 0.6).delay(0.35)) { shine = true }
    }
}

/// Grand logo au centre de l'écran lors d'un changement de rubrique (bref, sans bloquer).
struct SectionIntro: View {
    let section: AppSection
    @State private var label = false

    var body: some View {
        let (dark, _) = section.colors
        ZStack {
            Color.black.opacity(0.55)
            RadialGradient(colors: [dark.opacity(0.35), .clear], center: .center, startRadius: 0, endRadius: 260)
            VStack(spacing: 16) {
                SectionBadge(section: section, size: 76)
                Text(section.label.uppercased())
                    .font(.system(size: 24, weight: .black).italic())
                    .tracking(label ? 2 : 10)
                    .foregroundStyle(.white)
                    .opacity(label ? 1 : 0)
                    .offset(y: label ? 0 : 10)
            }
        }
        .ignoresSafeArea()
        .allowsHitTesting(false)
        .accessibilityHidden(true)
        .onAppear { withAnimation(.easeOut(duration: 0.4).delay(0.15)) { label = true } }
    }
}

/// Écran de démarrage : cinq feux rouges s'allument puis s'éteignent, le logo arrive.
struct SplashView: View {
    let onFinish: () -> Void

    @State private var lights = 0
    @State private var out = false
    @State private var logo = false
    @State private var xSpin = false
    @State private var line = false

    var body: some View {
        ZStack {
            Color(hex: 0x07070B).ignoresSafeArea()
            VStack(spacing: 24) {
                HStack(spacing: 12) {
                    ForEach(0..<5, id: \.self) { i in
                        let on = i < lights && !out
                        Circle()
                            .fill(on ? Color(hex: 0xFF1E10) : Color(hex: 0x2A0A0A))
                            .frame(width: 24, height: 24)
                            .shadow(color: on ? Color(hex: 0xFF2A1A) : .clear, radius: 10)
                            .overlay(Circle().stroke(on ? Color(hex: 0xFF6A5A) : Color(hex: 0x1A1A22), lineWidth: 2))
                    }
                }
                HStack(spacing: 0) {
                    Text("F1").foregroundStyle(.white)
                    Text("X").foregroundStyle(Color.f1Red)
                        .rotationEffect(.degrees(xSpin ? 0 : -200))
                        .scaleEffect(xSpin ? 1 : 0.3)
                }
                .font(.system(size: 72, weight: .black).italic())
                .offset(x: logo ? 0 : -140)
                .opacity(logo ? 1 : 0)
                .blur(radius: logo ? 0 : 6)
                Capsule().fill(Color.f1Red).frame(width: line ? 160 : 0, height: 4)
            }
        }
        .contentShape(Rectangle())
        .onTapGesture { onFinish() }
        .task {
            for i in 1...5 {
                try? await Task.sleep(nanoseconds: 150_000_000)
                lights = i
            }
            try? await Task.sleep(nanoseconds: 250_000_000)
            out = true
            withAnimation(.spring(response: 0.5, dampingFraction: 0.7)) { logo = true }
            withAnimation(.spring(response: 0.6, dampingFraction: 0.55).delay(0.2)) { xSpin = true }
            withAnimation(.easeOut(duration: 0.45).delay(0.45)) { line = true }
            try? await Task.sleep(nanoseconds: 1_100_000_000)
            onFinish()
        }
    }
}

private struct SectionKey: EnvironmentKey {
    static let defaultValue: AppSection = .home
}

extension EnvironmentValues {
    /// Rubrique de la pile de navigation courante.
    var appSection: AppSection {
        get { self[SectionKey.self] }
        set { self[SectionKey.self] = newValue }
    }
}

/// Logo de la rubrique en haut à gauche de la page, animé à chaque affichage de page.
struct SectionLogoToolbar: ViewModifier {
    @Environment(\.appSection) private var section

    func body(content: Content) -> some View {
        content.toolbar {
            ToolbarItem(placement: .topBarLeading) {
                SectionBadge(section: section, size: 22)
            }
        }
    }
}

extension View {
    func sectionLogo() -> some View { modifier(SectionLogoToolbar()) }
}
