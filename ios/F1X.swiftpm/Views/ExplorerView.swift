import SwiftUI

struct ExplorerView: View {
    static let version: String = {
        let info = Bundle.main.infoDictionary
        let v = info?["CFBundleShortVersionString"] as? String ?? "1.0"
        let b = info?["CFBundleVersion"] as? String ?? "?"
        return "\(v) (\(b))"
    }()

    @AppStorage("theme") private var theme = "auto"
    @AppStorage(AppLanguage.key) private var language = "auto"

    var body: some View {
        List {
            Section(L("Apparence", "Appearance")) {
                Picker(L("Thème", "Theme"), selection: $theme) {
                    Text(L("Automatique", "Automatic")).tag("auto")
                    Text(L("Clair", "Light")).tag("light")
                    Text(L("Sombre", "Dark")).tag("dark")
                }
                .pickerStyle(.segmented)
                Picker(L("Langue", "Language"), selection: $language) {
                    Text(L("Automatique", "Automatic")).tag("auto")
                    Text("Français").tag("fr")
                    Text("English").tag("en")
                }
                .pickerStyle(.segmented)
            }
            Section(L("Histoire", "History")) {
                NavigationLink { ChampionsView() } label: { Label(L("Champions du monde depuis 1950", "World champions since 1950"), systemImage: "trophy.fill").cascadeIn() }
                NavigationLink { RecordsView() } label: { Label("Records", systemImage: "chart.bar.fill").cascadeIn() }
                NavigationLink { CalendarView() } label: { Label(L("Toutes les saisons", "Every season"), systemImage: "calendar").cascadeIn() }
            }
            Section(L("Outils", "Tools")) {
                NavigationLink { DataYearView(year: Calendar.current.component(.year, from: Date())) } label: { Label(L("Données OpenF1 (télémétrie, pneus, radios…)", "OpenF1 data (telemetry, tyres, radio…)"), systemImage: "chart.xyaxis.line").cascadeIn() }
                NavigationLink { CompareView() } label: { Label(L("Comparateur de pilotes", "Driver comparison"), systemImage: "arrow.left.arrow.right").cascadeIn() }
                NavigationLink { NewsView() } label: { Label(L("Actualités", "News"), systemImage: "newspaper.fill").cascadeIn() }
                NavigationLink { GlossaryView() } label: { Label(L("Lexique", "Glossary"), systemImage: "book.fill").cascadeIn() }
            }
            Section(L("Jeux", "Games")) {
                NavigationLink { PredictView() } label: { Label(L("Pronostics", "Predictions"), systemImage: "sparkles").cascadeIn() }
                NavigationLink { FantasyView() } label: { Label("Fantasy F1", systemImage: "person.3.fill").cascadeIn() }
                NavigationLink { QuizView() } label: { Label(L("Quiz : devine le pilote", "Quiz: guess the driver"), systemImage: "questionmark.circle.fill").cascadeIn() }
            }
            Section("F1X") {
                Link(destination: URL(string: "presentation", relativeTo: Server.base)!) { Label(L("Site de F1X", "F1X website"), systemImage: "safari").cascadeIn() }
                Link(destination: URL(string: "mentions-legales", relativeTo: Server.base)!) { Label(L("Mentions légales", "Legal notice"), systemImage: "doc.text").cascadeIn() }
                Link(destination: URL(string: "confidentialite", relativeTo: Server.base)!) { Label(L("Confidentialité", "Privacy"), systemImage: "hand.raised").cascadeIn() }
                Link(destination: URL(string: "credits", relativeTo: Server.base)!) { Label(L("Crédits et sources", "Credits & sources"), systemImage: "info.circle").cascadeIn() }
            }
            Section {
                VStack(alignment: .leading, spacing: 4) {
                    Text(L("Conçu et développé par", "Designed and built by")).font(.caption).foregroundStyle(.secondary)
                    Text("Maxime Nathan Lestage").font(.headline)
                }
                // Numéro de build installé (le même que dans TestFlight).
                Text(L("Version \(Self.version)", "Version \(Self.version)"))
                    .font(.caption.monospacedDigit()).foregroundStyle(.secondary)
                Text(L("F1X est une application indépendante et non officielle, sans lien avec Formula One Group, la FIA ou les écuries. F1 et Formula 1 sont des marques de Formula One Licensing B.V.",
                       "F1X is an independent, unofficial app, not affiliated with Formula One Group, the FIA or the teams. F1 and Formula 1 are trademarks of Formula One Licensing B.V."))
                    .font(.caption).foregroundStyle(.secondary)
            }
        }
        .listStyle(.insetGrouped)
        .navigationTitle(L("Explorer", "Explore"))
        .f1Destinations()
    }
}

// MARK: - Champions et records

struct ChampionsView: View {
    @State private var champions: [Champion] = []

    var body: some View {
        List(champions.reversed()) { c in
            VStack(alignment: .leading, spacing: 4) {
                HStack {
                    Text(c.season).font(.headline.monospacedDigit())
                    if c.in_progress == true { Text(L("en cours", "in progress")).font(.caption).foregroundStyle(.secondary) }
                }
                if let d = c.driver {
                    NavigationLink(value: d) {
                        HStack {
                            Avatar(name: d.fullName, wikipedia: d.url, color: Team.color(c.driver_team?.constructorId), size: 32)
                            VStack(alignment: .leading) {
                                Text("\(Flag.nationality(d.nationality)) \(d.fullName)").bold()
                                Text([c.driver_team?.name, c.driver_points.map { "\($0) pts" }].compactMap { $0 }.joined(separator: " · "))
                                    .font(.caption).foregroundStyle(.secondary)
                            }
                        }
                    }
                }
                if let k = c.constructor {
                    Text(L("Constructeurs : \(k.name)", "Constructors: \(k.name)")).font(.footnote).foregroundStyle(.secondary)
                }
            }
            .cascadeIn()
        }
        .navigationTitle(L("Champions", "Champions"))
        .task { champions = (try? await ServerAPI.shared.champions()) ?? [] }
    }
}

struct RecordsView: View {
    @State private var champions: [Champion] = []

    var body: some View {
        let done = champions.filter { $0.in_progress != true }
        let drivers = Dictionary(grouping: done.compactMap(\.driver), by: \.driverId)
            .map { ($0.value[0], $0.value.count) }.sorted { $0.1 > $1.1 }
        let teams = Dictionary(grouping: done.compactMap(\.constructor), by: \.constructorId)
            .map { ($0.value[0], $0.value.count) }.sorted { $0.1 > $1.1 }
        List {
            Section(L("Titres pilotes", "Drivers' titles")) {
                ForEach(Array(drivers.prefix(10).enumerated()), id: \.offset) { i, e in
                    NavigationLink(value: e.0) {
                        HStack {
                            Text("\(i + 1)").bold().frame(width: 24)
                            Avatar(name: e.0.fullName, wikipedia: e.0.url, size: 30)
                            Text(e.0.fullName)
                            Spacer()
                            CountingText(text: "×\(e.1)").bold()
                        }
                        .cascadeIn()
                    }
                }
            }
            Section(L("Titres constructeurs (depuis 1958)", "Constructors' titles (since 1958)")) {
                ForEach(Array(teams.prefix(10).enumerated()), id: \.offset) { i, e in
                    NavigationLink(value: e.0) {
                        HStack {
                            Text("\(i + 1)").bold().frame(width: 24)
                            RoundedRectangle(cornerRadius: 2).fill(Team.color(e.0.constructorId)).frame(width: 4, height: 22)
                            Text(e.0.name)
                            Spacer()
                            CountingText(text: "×\(e.1)").bold()
                        }
                        .cascadeIn()
                    }
                }
            }
            Section(L("Plus de victoires sur une saison", "Most wins in a season")) {
                ForEach(Array(done.sorted { (Int($0.driver_wins ?? "0") ?? 0) > (Int($1.driver_wins ?? "0") ?? 0) }.prefix(8)), id: \.season) { c in
                    if let d = c.driver {
                        HStack {
                            Text(d.fullName)
                            Spacer()
                            Text(L("\(c.driver_wins ?? "0") victoires en \(c.season)", "\(c.driver_wins ?? "0") wins in \(c.season)")).foregroundStyle(.secondary)
                        }
                    }
                }
            }
        }
        .navigationTitle("Records")
        .task { champions = (try? await ServerAPI.shared.champions()) ?? [] }
    }
}

// MARK: - Actualités et lexique

struct NewsView: View {
    @State private var articles: [Article] = []
    @State private var failed = false

    var body: some View {
        List {
            if articles.isEmpty && !failed { StartLightsLoader() }
            if failed { Text(L("Actualités indisponibles.", "News unavailable.")).foregroundStyle(.secondary) }
            ForEach(articles) { a in
                if let url = URL(string: a.link) {
                    Link(destination: url) {
                        VStack(alignment: .leading, spacing: 6) {
                            if let img = a.image.flatMap(URL.init(string:)) {
                                AsyncImage(url: img, transaction: Transaction(animation: .easeOut(duration: 0.6))) { phase in
                                    if let image = phase.image {
                                        image.resizable().scaledToFill()
                                            .transition(.opacity.combined(with: .scale(scale: 1.12)))
                                    } else {
                                        Color.clear
                                    }
                                }
                                    .frame(height: 150).frame(maxWidth: .infinity).clipped()
                                    .clipShape(RoundedRectangle(cornerRadius: 10))
                            }
                            Text(a.title).font(.headline).foregroundStyle(Color.primary)
                            if !a.excerpt.isEmpty { Text(a.excerpt).font(.footnote).foregroundStyle(Color.secondary).lineLimit(3) }
                            Text("\(a.source) ↗").font(.caption.bold()).foregroundStyle(Color.f1Red)
                        }
                        .cascadeUp()
                    }
                }
            }
        }
        .navigationTitle(L("Actualités", "News"))
        .task {
            do { articles = try await ServerAPI.shared.news(lang: isFrench ? "fr" : "en") } catch { failed = true }
        }
        .refreshable { articles = (try? await ServerAPI.shared.news(lang: isFrench ? "fr" : "en")) ?? articles }
        .autoRefresh(every: 600) { articles = (try? await ServerAPI.shared.news(lang: isFrench ? "fr" : "en")) ?? articles }
    }
}

struct GlossaryView: View {
    @State private var query = ""

    var body: some View {
        let q = query.folding(options: [.diacriticInsensitive, .caseInsensitive], locale: nil)
        let list = glossary.filter {
            q.isEmpty || ($0.term + " " + $0.definition).folding(options: [.diacriticInsensitive, .caseInsensitive], locale: nil).contains(q)
        }
        List(list) { e in
            VStack(alignment: .leading, spacing: 4) {
                Text(e.term).font(.headline)
                Text(e.definition).font(.subheadline).foregroundStyle(.secondary).fixedSize(horizontal: false, vertical: true)
            }
            .cascadeIn()
        }
        .searchable(text: $query, prompt: L("Rechercher un terme", "Search a term"))
        .navigationTitle(L("Lexique", "Glossary"))
    }
}

// MARK: - Comparateur

struct CompareView: View {
    @State private var standings: [DriverStanding] = []
    @State private var a: String = ""
    @State private var b: String = ""
    @State private var ca: [Race] = []
    @State private var cb: [Race] = []

    var body: some View {
        List {
            Section {
                Picker(L("Pilote A", "Driver A"), selection: $a) {
                    ForEach(standings) { Text($0.driver.fullName).tag($0.driver.driverId) }
                }
                Picker(L("Pilote B", "Driver B"), selection: $b) {
                    ForEach(standings) { Text($0.driver.fullName).tag($0.driver.driverId) }
                }
            }
            if !ca.isEmpty && !cb.isEmpty {
                let sa = CareerStats(ca), sb = CareerStats(cb)
                Section(L("Carrière", "Career")) {
                    row(L("Départs", "Starts"), sa.starts, sb.starts)
                    row(L("Victoires", "Wins"), sa.wins, sb.wins)
                    row("Podiums", sa.podiums, sb.podiums)
                    row("Poles", sa.poles, sb.poles)
                    row(L("Meilleurs tours", "Fastest laps"), sa.fastest, sb.fastest)
                    row(L("Saisons", "Seasons"), sa.seasons.count, sb.seasons.count)
                }
                let common = Dictionary(cb.map { ($0.id, $0) }, uniquingKeysWith: { a, _ in a })
                let duels = ca.compactMap { r -> (Int, Int)? in
                    guard let o = common[r.id], let pa = r.results?.first.flatMap({ Int($0.position) }),
                          let pb = o.results?.first.flatMap({ Int($0.position) }) else { return nil }
                    return (pa, pb)
                }
                if !duels.isEmpty {
                    Section(L("Face-à-face en course", "Head-to-head in races")) {
                        row(L("Devant à l'arrivée", "Finished ahead"), duels.filter { $0.0 < $0.1 }.count, duels.filter { $0.1 < $0.0 }.count)
                        Text(L("\(duels.count) courses disputées ensemble", "\(duels.count) races together")).font(.footnote).foregroundStyle(.secondary)
                    }
                }
            }
        }
        .navigationTitle(L("Comparateur", "Compare"))
        .task {
            standings = (try? await F1API.shared.driverStandings()) ?? []
            if a.isEmpty, standings.count > 1 {
                a = standings[0].driver.driverId
                b = standings[1].driver.driverId
            }
        }
        .task(id: a + b) {
            guard !a.isEmpty, !b.isEmpty else { return }
            async let x = try? F1API.shared.career(driverId: a)
            async let y = try? F1API.shared.career(driverId: b)
            ca = await x ?? []
            cb = await y ?? []
        }
    }

    private func name(_ id: String) -> String { standings.first { $0.driver.driverId == id }?.driver.familyName ?? id }

    private func row(_ label: String, _ x: Int, _ y: Int) -> some View {
        VStack(spacing: 4) {
            Text(label).font(.caption).foregroundStyle(.secondary)
            HStack {
                Text("\(x)").font(.title3.bold().monospacedDigit()).foregroundStyle(x >= y ? Color(hex: 0xE5483F) : .primary)
                Text(name(a)).font(.caption)
                Spacer()
                Text(name(b)).font(.caption)
                Text("\(y)").font(.title3.bold().monospacedDigit()).foregroundStyle(y >= x ? Color(hex: 0x3B9FD8) : .primary)
            }
            GeometryReader { geo in
                let total = max(x + y, 1)
                HStack(spacing: 2) {
                    Rectangle().fill(Color(hex: 0xE5483F)).frame(width: geo.size.width * CGFloat(x) / CGFloat(total))
                    Rectangle().fill(Color(hex: 0x3B9FD8))
                }
            }
            .frame(height: 6)
            .clipShape(Capsule())
        }
    }
}

// MARK: - Pronostics

struct PredictView: View {
    @State private var next: Race?
    @State private var drivers: [DriverStanding] = []
    @State private var pick = Prediction(season: "", round: "", raceName: "", p1: "", p2: "", p3: "")
    @State private var history: [(Prediction, Int?)] = []
    @State private var saved = false

    var body: some View {
        List {
            if let race = next {
                Section(L("Ton podium pour : \(race.raceName)", "Your podium for: \(race.raceName)")) {
                    ForEach([("1", \Prediction.p1), ("2", \Prediction.p2), ("3", \Prediction.p3)], id: \.0) { pos, key in
                        Picker("P\(pos)", selection: Binding(get: { pick[keyPath: key] }, set: { pick[keyPath: key] = $0; saved = false })) {
                            Text("–").tag("")
                            ForEach(drivers) { Text($0.driver.fullName).tag($0.driver.driverId) }
                        }
                    }
                    Button(saved ? L("Pronostic enregistré ✓", "Prediction saved ✓") : L("Enregistrer", "Save")) {
                        Predictions.save(pick)
                        saved = true
                    }
                    .disabled(pick.p1.isEmpty || pick.p2.isEmpty || pick.p3.isEmpty)
                }
            }
            Section(L("Mes pronostics", "My predictions")) {
                if history.isEmpty { Text(L("Aucun pour l'instant.", "None yet.")).foregroundStyle(.secondary) }
                ForEach(history, id: \.0) { p, score in
                    HStack {
                        Text("\(p.season) · \(p.raceName)")
                        Spacer()
                        Text(score.map { "\($0) pts" } ?? L("en attente", "pending")).bold()
                    }
                }
                Text(L("10 points par pilote à la bonne place, 3 s'il est sur le podium à une autre place.", "10 points per driver in the right place, 3 if on the podium elsewhere."))
                    .font(.caption).foregroundStyle(.secondary)
            }
        }
        .navigationTitle(L("Pronostics", "Predictions"))
        .task {
            async let s = try? F1API.shared.schedule()
            async let d = try? F1API.shared.driverStandings()
            let races = await s ?? []
            drivers = await d ?? []
            next = races.first { !$0.isOver() }
            if let r = next {
                pick = Predictions.all().first { $0.season == r.season && $0.round == r.round }
                    ?? Prediction(season: r.season, round: r.round, raceName: r.raceName, p1: "", p2: "", p3: "")
            }
            var out: [(Prediction, Int?)] = []
            for p in Predictions.all().reversed() {
                let results = (try? await F1API.shared.results(season: p.season, round: Int(p.round) ?? 0)) ?? []
                out.append((p, results.isEmpty ? nil : Predictions.score(p, results: results)))
            }
            history = out
        }
    }
}

// MARK: - Fantasy

struct FantasyView: View {
    @State private var drivers: [DriverStanding] = []
    @State private var teams: [ConstructorStanding] = []
    /// Équipe validée (enregistrée) et brouillon en cours de composition.
    @State private var saved = FantasyTeam.load()
    @State private var draft = FantasyTeam.load()
    /// Pourquoi le dernier choix a été refusé (5 pilotes déjà, budget dépassé).
    @State private var notice: String?

    private let budget = 100.0

    /// Prix (M€) d'après les points : du leader (~30) au dernier (~5).
    private func price(_ points: String, _ list: [Double]) -> Double {
        let p = Double(points) ?? 0, hi = list.max() ?? 1, lo = list.min() ?? 0
        return (5 + 25 * (p - lo) / max(hi - lo, 1)).rounded()
    }

    var body: some View {
        let dp = drivers.map { Double($0.points) ?? 0 }, tp = teams.map { Double($0.points) ?? 0 }
        let teamCost = teams.first { $0.constructor.constructorId == draft.team }.map { price($0.points, tp) } ?? 0
        let spent = drivers.filter { draft.drivers.contains($0.driver.driverId) }.map { price($0.points, dp) }.reduce(0, +) + teamCost
        let score = drivers.filter { saved.drivers.contains($0.driver.driverId) }.map { Double($0.points) ?? 0 }.reduce(0, +)
            + (teams.first { $0.constructor.constructorId == saved.team }.flatMap { Double($0.points) } ?? 0)
        List {
            Section {
                StatGrid(items: [
                    (L("Budget restant", "Budget left"), String(format: "%.0f M€", budget - spent)),
                    (L("Pilotes", "Drivers"), "\(draft.drivers.count)/5"),
                    (L("Points", "Points"), String(format: "%.0f", score)),
                ])
                Text(L("Choisis 5 pilotes et 1 écurie sans dépasser 100 M€, puis valide ton équipe. Ton score = les points marqués cette saison par l'équipe validée.",
                       "Pick 5 drivers and 1 team within €100M, then confirm your team. Your score = points scored this season by the confirmed team."))
                    .font(.caption).foregroundStyle(.secondary)
                if let notice {
                    Text(notice).font(.footnote.bold()).foregroundStyle(Color.f1Red)
                        .transition(.opacity.combined(with: .move(edge: .top)))
                }
            }
            Section(L("Pilotes", "Drivers")) {
                ForEach(drivers) { d in
                    let cost = price(d.points, dp)
                    Button { toggleDriver(d.driver.driverId, cost: cost, spent: spent) } label: {
                        pickRow(d.driver.fullName, cost: cost, on: draft.drivers.contains(d.driver.driverId))
                    }
                    .buttonStyle(.plain)
                }
            }
            Section(L("Écurie", "Team")) {
                ForEach(teams) { t in
                    let cost = price(t.points, tp)
                    Button { chooseTeam(t.constructor.constructorId, cost: cost, spent: spent - teamCost) } label: {
                        pickRow(t.constructor.name, cost: cost, on: draft.team == t.constructor.constructorId)
                    }
                    .buttonStyle(.plain)
                }
            }
        }
        .navigationTitle("Fantasy F1")
        // Bouton « Valider mon équipe » toujours visible au-dessus de la barre d'onglets.
        .safeAreaInset(edge: .bottom) { validateBar(spent: spent) }
        .task {
            async let d = try? F1API.shared.driverStandings()
            async let t = try? F1API.shared.constructorStandings()
            drivers = await d ?? []
            teams = await t ?? []
        }
    }

    private func pickRow(_ name: String, cost: Double, on: Bool) -> some View {
        HStack {
            Image(systemName: on ? "checkmark.circle.fill" : "circle")
                .foregroundStyle(on ? Color.f1Red : Color.secondary)
                .contentTransition(.symbolEffect(.replace))
            Text(name).foregroundStyle(Color.primary)
            Spacer()
            Text(String(format: "%.0f M€", cost)).monospacedDigit().foregroundStyle(Color.secondary)
        }
        .contentShape(Rectangle())
    }

    private func toggleDriver(_ id: String, cost: Double, spent: Double) {
        withAnimation(.snappy) {
            if let i = draft.drivers.firstIndex(of: id) {
                draft.drivers.remove(at: i)
                notice = nil
            } else if draft.drivers.count >= 5 {
                notice = L("5 pilotes maximum : retire d'abord un pilote.", "5 drivers max: remove a driver first.")
            } else if spent + cost > budget {
                notice = L(String(format: "Budget dépassé : il manque %.0f M€ pour ce pilote.", spent + cost - budget),
                           String(format: "Over budget: €%.0fM short for this driver.", spent + cost - budget))
            } else {
                draft.drivers.append(id)
                notice = nil
            }
        }
    }

    /// `spent` : dépenses hors écurie (on peut toujours remplacer l'écurie choisie).
    private func chooseTeam(_ id: String, cost: Double, spent: Double) {
        withAnimation(.snappy) {
            if draft.team == id {
                draft.team = nil
                notice = nil
            } else if spent + cost > budget {
                notice = L(String(format: "Budget dépassé : il manque %.0f M€ pour cette écurie.", spent + cost - budget),
                           String(format: "Over budget: €%.0fM short for this team.", spent + cost - budget))
            } else {
                draft.team = id
                notice = nil
            }
        }
    }

    private func validateBar(spent: Double) -> some View {
        let missing = 5 - draft.drivers.count
        let ready = missing == 0 && draft.team != nil && spent <= budget
        let changed = draft != saved
        let done = ready && !changed
        let status: String = {
            if spent > budget { return L(String(format: "Budget dépassé de %.0f M€.", spent - budget), String(format: "€%.0fM over budget.", spent - budget)) }
            if missing > 0 && draft.team == nil { return L("Encore \(missing) pilote(s) et une écurie à choisir.", "\(missing) more driver(s) and a team to pick.") }
            if missing > 0 { return L("Encore \(missing) pilote(s) à choisir.", "\(missing) more driver(s) to pick.") }
            if draft.team == nil { return L("Choisis une écurie.", "Pick a team.") }
            if changed { return L("Modifications pas encore validées.", "Changes not confirmed yet.") }
            return L("Équipe validée : elle marque des points toute la saison.", "Team confirmed: it scores all season.")
        }()
        return VStack(spacing: 6) {
            Button {
                draft.save()
                withAnimation(.spring(response: 0.4, dampingFraction: 0.65)) {
                    saved = draft
                    notice = nil
                }
            } label: {
                Label(done ? L("Équipe validée", "Team confirmed") : L("Valider mon équipe", "Confirm my team"),
                      systemImage: done ? "checkmark.seal.fill" : "checkmark.circle")
                    .font(.headline)
                    .contentTransition(.symbolEffect(.replace))
                    .symbolEffect(.bounce, value: saved)
                    .frame(maxWidth: .infinity)
                    .padding(.vertical, 14)
                    .foregroundStyle(done ? Color.green : ready ? Color.white : Color.secondary)
                    .background(done ? Color.green.opacity(0.18) : ready ? Color.f1Red : Color.gray.opacity(0.25), in: Capsule())
                    .overlay { if ready && changed { SweepShine().clipShape(Capsule()) } }
            }
            .buttonStyle(PressableStyle())
            .disabled(!(ready && changed))
            .sensoryFeedback(.success, trigger: saved)
            Text(status)
                .font(.caption)
                .foregroundStyle(spent > budget ? Color.f1Red : Color.secondary)
                .contentTransition(.opacity)
        }
        .padding(.horizontal, 16)
        .padding(.top, 10)
        .padding(.bottom, 8)
        .background(.bar)
    }
}

// MARK: - Quiz

struct QuizView: View {
    @State private var champions: [Champion] = []
    @State private var answer: Champion?
    @State private var options: [Champion] = []
    @State private var picked: String?
    @State private var score = 0
    /// Secousse de la mauvaise réponse (0 → 1 en une demi-seconde).
    @State private var shake: CGFloat = 0
    @AppStorage("f1x-quiz-best") private var best = 0

    var body: some View {
        List {
            Section {
                HStack {
                    Text(L("Score : \(score)", "Score: \(score)")).bold()
                        .contentTransition(.numericText(value: Double(score)))
                    Spacer()
                    Text(L("Record : \(best)", "Best: \(best)")).foregroundStyle(.secondary)
                        .contentTransition(.numericText(value: Double(best)))
                }
            }
            if let a = answer, let d = a.driver {
                Section(L("Qui est ce champion du monde ?", "Who is this world champion?")) {
                    Text(L("Champion en \(a.season)", "Champion in \(a.season)")).font(.headline)
                    Text(L("Écurie : \(a.driver_team?.name ?? "?") · \(a.driver_wins ?? "?") victoires · \(a.driver_points ?? "?") points · nationalité \(d.nationality ?? "?")",
                           "Team: \(a.driver_team?.name ?? "?") · \(a.driver_wins ?? "?") wins · \(a.driver_points ?? "?") points · \(d.nationality ?? "?") nationality"))
                        .foregroundStyle(.secondary)
                    ForEach(options, id: \.season) { o in
                        if let od = o.driver {
                            Button {
                                guard picked == nil else { return }
                                picked = od.driverId
                                withAnimation(.snappy) {
                                    if od.driverId == d.driverId {
                                        score += 1
                                        best = max(best, score)
                                    } else {
                                        score = 0
                                    }
                                }
                                if od.driverId != d.driverId {
                                    withAnimation(.linear(duration: 0.45)) { shake = 1 }
                                }
                            } label: {
                                HStack {
                                    Text(od.fullName).foregroundStyle(Color.primary)
                                    Spacer()
                                    // Bonne réponse : la coche rebondit ; mauvaise : la ligne secoue la tête.
                                    if picked != nil && od.driverId == d.driverId {
                                        Image(systemName: "checkmark.circle.fill").foregroundStyle(.green)
                                            .symbolEffect(.bounce, value: picked)
                                            .transition(.scale.combined(with: .opacity))
                                    } else if picked == od.driverId {
                                        Image(systemName: "xmark.circle.fill").foregroundStyle(.red)
                                            .transition(.scale.combined(with: .opacity))
                                    }
                                }
                                .modifier(ShakeEffect(animatableData: picked == od.driverId && od.driverId != d.driverId ? shake : 0))
                                .cascadeIn()
                            }
                        }
                    }
                    if picked != nil {
                        Button(L("Question suivante", "Next question")) { newQuestion() }.bold()
                    }
                }
            } else {
                StartLightsLoader()
            }
        }
        .navigationTitle("Quiz")
        .task {
            champions = ((try? await ServerAPI.shared.champions()) ?? []).filter { $0.driver != nil && $0.in_progress != true }
            newQuestion()
        }
    }

    private func newQuestion() {
        picked = nil
        shake = 0
        guard let a = champions.randomElement(), let ad = a.driver else { return }
        var seen: Set<String> = [ad.driverId]
        var opts = [a]
        for c in champions.shuffled() {
            guard opts.count < 4, let d = c.driver, !seen.contains(d.driverId) else { continue }
            seen.insert(d.driverId)
            opts.append(c)
        }
        answer = a
        options = opts.shuffled()
    }
}
