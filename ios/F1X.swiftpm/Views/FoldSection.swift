import SwiftUI

/// Section de liste repliable : on touche le titre pour la plier ou la déplier.
/// L'état est retenu d'une page à l'autre (on garde pliées les rubriques qui n'intéressent pas).
struct FoldSection<Content: View>: View {
    let title: String
    var footer: String?
    @ViewBuilder var content: Content

    @AppStorage("foldedSections") private var folded = ""

    init(_ title: String, footer: String? = nil, @ViewBuilder content: () -> Content) {
        self.title = title
        self.footer = footer
        self.content = content()
    }

    /// Identifiant stable : le titre sans les nombres (« Arrêts aux stands (42) » → « Arrêts aux stands »).
    private var key: String {
        title.filter { !$0.isNumber && $0 != "(" && $0 != ")" }.trimmingCharacters(in: .whitespaces)
    }

    private var isFolded: Bool { folded.components(separatedBy: "|").contains(key) }

    private func toggle() {
        var set = Set(folded.components(separatedBy: "|").filter { !$0.isEmpty })
        if set.contains(key) { set.remove(key) } else { set.insert(key) }
        folded = set.sorted().joined(separator: "|")
    }

    var body: some View {
        Section {
            if !isFolded { content }
        } header: {
            Button {
                withAnimation(.easeInOut(duration: 0.2)) { toggle() }
            } label: {
                HStack {
                    Text(title)
                    Spacer()
                    Image(systemName: "chevron.down")
                        .rotationEffect(.degrees(isFolded ? -90 : 0))
                        .font(.caption.bold())
                }
                .contentShape(Rectangle())
            }
            .buttonStyle(.plain)
            .accessibilityHint(isFolded ? L("Déplier", "Expand") : L("Replier", "Collapse"))
        } footer: {
            if let footer, !isFolded { Text(footer) }
        }
    }
}
