import SwiftUI

/// Résumé Wikipédia (biographie d'un pilote, histoire d'une écurie ou d'un circuit), en
/// français quand l'article existe, via le serveur F1X.
struct WikiSummaryView: View {
    /// Lien de l'article anglais (fourni par Jolpica).
    let url: String?
    @State private var summary: Summary?
    @State private var expanded = false

    private struct Summary: Decodable {
        let extract: String
        let url: String
        let lang: String
    }

    private var title: String? {
        guard let s = url, let u = URL(string: s) else { return nil }
        return u.lastPathComponent.removingPercentEncoding ?? u.lastPathComponent
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            if let s = summary {
                Text(s.extract)
                    .font(.subheadline)
                    .lineLimit(expanded ? nil : 5)
                    .fixedSize(horizontal: false, vertical: true)
                HStack {
                    Button(expanded ? L("Réduire", "Less") : L("Lire la suite", "Read more")) {
                        withAnimation { expanded.toggle() }
                    }
                    .font(.footnote.bold())
                    .buttonStyle(.borderless)
                    Spacer()
                    if let link = URL(string: s.url) {
                        Link(L("Wikipédia ↗", "Wikipedia ↗"), destination: link).font(.footnote)
                    }
                }
            } else if let s = url, let link = URL(string: s) {
                Link(L("Wikipédia ↗", "Wikipedia ↗"), destination: link)
            }
        }
        .task(id: url) {
            guard let title else { return }
            let lang = isFrench ? "fr" : "en"
            let path = "api/wiki/\(lang)/\(title.addingPercentEncoding(withAllowedCharacters: .urlPathAllowed) ?? title)"
            summary = try? await ServerAPI.shared.get(path, as: Summary.self, ttl: 86_400)
        }
    }
}
