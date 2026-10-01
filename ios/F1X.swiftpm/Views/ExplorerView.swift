import SwiftUI

struct ExplorerView: View {
    var body: some View {
        List {
            Section(L("Histoire", "History")) {
                NavigationLink { ChampionsView() } label: { Label(L("Champions du monde depuis 1950", "World champions since 1950"), systemImage: "trophy.fill") }
                NavigationLink { RecordsView() } label: { Label("Records", systemImage: "chart.bar.fill") }
                NavigationLink { CalendarView() } label: { Label(L("Toutes les saisons", "Every season"), systemImage: "calendar") }
            }
            Section(L("Outils", "Tools")) {
                NavigationLink { DataYearView(year: Calendar.current.component(.year, from: Date())) } label: { Label(L("Données OpenF1 (télémétrie, pneus, radios…)", "OpenF1 data (telemetry, tyres, radio…)"), systemImage: "chart.xyaxis.line") }
                NavigationLink { CompareView() } label: { Label(L("Comparateur de pilotes", "Driver comparison"), systemImage: "arrow.left.arrow.right") }
                NavigationLink { NewsView() } label: { Label(L("Actualités", "News"), systemImage: "newspaper.fill") }
                NavigationLink { GlossaryView() } label: { Label(L("Lexique", "Glossary"), systemImage: "book.fill") }
            }
            Section(L("Jeux", "Games")) {
                NavigationLink { PredictView() } label: { Label(L("Pronostics", "Predictions"), systemImage: "sparkles") }
                NavigationLink { FantasyView() } label: { Label("Fantasy F1", systemImage: "person.3.fill") }
                NavigationLink { QuizView() } label: { Label(L("Quiz : devine le pilote", "Quiz: guess the driver"), systemImage: "questionmark.circle.fill") }
            }
            Section("F1X") {
                Link(destination: URL(string: "presentation", relativeTo: Server.base)!) { Label(L("Site de F1X", "F1X website"), systemImage: "safari") }
                Link(destination: URL(string: "mentions-legales", relativeTo: Server.base)!) { Label(L("Mentions légales", "Legal notice"), systemImage: "doc.text") }
                Link(destination: URL(string: "confidentialite", relativeTo: Server.base)!) { Label(L("Confidentialité", "Privacy"), systemImage: "hand.raised") }
                Link(destination: URL(string: "credits", relativeTo: Server.base)!) { Label(L("Crédits et sources", "Credits & sources"), systemImage: "info.circle") }
            }
            Section {
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
                            Text("×\(e.1)").bold()
                        }
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
                            Text("×\(e.1)").bold()
                        }
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
            if articles.isEmpty && !failed { ProgressView().frame(maxWidth: .infinity) }
            if failed { Text(L("Actualités indisponibles.", "News unavailable.")).foregroundStyle(.secondary) }
            ForEach(articles) { a in
                if let url = URL(string: a.link) {
                    Link(destination: url) {
                        VStack(alignment: .leading, spacing: 6) {
                            if let img = a.image.flatMap(URL.init(string:)) {
                                AsyncImage(url: img) { $0.resizable().scaledToFill() } placeholder: { Color.clear }
                                    .frame(height: 150).frame(maxWidth: .infinity).clipped()
                                    .clipShape(RoundedRectangle(cornerRadius: 10))
                            }
                            Text(a.title).font(.headline).foregroundStyle(.primary)
                            if !a.excerpt.isEmpty { Text(a.excerpt).font(.footnote).foregroundStyle(.secondary).lineLimit(3) }
                            Text("\(a.source) ↗").font(.caption.bold()).foregroundStyle(Color.f1Red)
                        }
                    }
                }
            }
        }
        .navigationTitle(L("Actualités", "News"))
        .task {
            do { articles = try await ServerAPI.shared.news(lang: isFrench ? "fr" : "en") } catch { failed = true }
        }
        .refreshable { articles = (try? await ServerAPI.shared.news(lang: isFrench ? "fr" : "en")) ?? articles }
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
                let common = Dictionary(uniqueKeysWithValues: cb.map { ($0.id, $0) })
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
    @State private var squad = FantasyTeam.load()

    private let budget = 100.0

    /// Prix (M€) d'après les points : du leader (~30) au dernier (~5).
    private func price(_ points: String, _ list: [Double]) -> Double {
        let p = Double(points) ?? 0, hi = list.max() ?? 1, lo = list.min() ?? 0
        return (5 + 25 * (p - lo) / max(hi - lo, 1)).rounded()
    }

    var body: some View {
        let dp = drivers.map { Double($0.points) ?? 0 }, tp = teams.map { Double($0.points) ?? 0 }
        let spent = drivers.filter { squad.drivers.contains($0.driver.driverId) }.map { price($0.points, dp) }.reduce(0, +)
            + (teams.first { $0.constructor.constructorId == squad.team }.map { price($0.points, tp) } ?? 0)
        let score = drivers.filter { squad.drivers.contains($0.driver.driverId) }.map { Double($0.points) ?? 0 }.reduce(0, +)
            + (teams.first { $0.constructor.constructorId == squad.team }.flatMap { Double($0.points) } ?? 0)
        List {
            Section {
                StatGrid(items: [
                    (L("Budget restant", "Budget left"), String(format: "%.0f M€", budget - spent)),
                    (L("Pilotes", "Drivers"), "\(squad.drivers.count)/5"),
                    (L("Points", "Points"), String(format: "%.0f", score)),
                ])
                Text(L("Choisis 5 pilotes et 1 écurie sans dépasser 100 M€. Ton score = les points marqués cette saison.",
                       "Pick 5 drivers and 1 team within €100M. Your score = points scored this season."))
                    .font(.caption).foregroundStyle(.secondary)
            }
            Section(L("Pilotes", "Drivers")) {
                ForEach(drivers) { d in
                    let on = squad.drivers.contains(d.driver.driverId)
                    let cost = price(d.points, dp)
                    Button {
                        if on { squad.drivers.removeAll { $0 == d.driver.driverId } }
                        else if squad.drivers.count < 5 && spent + cost <= budget { squad.drivers.append(d.driver.driverId) }
                        squad.save()
                    } label: {
                        HStack {
                            Image(systemName: on ? "checkmark.circle.fill" : "circle").foregroundStyle(on ? Color.f1Red : .secondary)
                            Text(d.driver.fullName).foregroundStyle(.primary)
                            Spacer()
                            Text(String(format: "%.0f M€", cost)).monospacedDigit().foregroundStyle(.secondary)
                        }
                    }
                }
            }
            Section(L("Écurie", "Team")) {
                ForEach(teams) { t in
                    let on = squad.team == t.constructor.constructorId
                    let cost = price(t.points, tp)
                    Button {
                        squad.team = on ? nil : t.constructor.constructorId
                        squad.save()
                    } label: {
                        HStack {
                            Image(systemName: on ? "checkmark.circle.fill" : "circle").foregroundStyle(on ? Color.f1Red : .secondary)
                            Text(t.constructor.name).foregroundStyle(.primary)
                            Spacer()
                            Text(String(format: "%.0f M€", cost)).monospacedDigit().foregroundStyle(.secondary)
                        }
                    }
                }
            }
        }
        .navigationTitle("Fantasy F1")
        .task {
            async let d = try? F1API.shared.driverStandings()
            async let t = try? F1API.shared.constructorStandings()
            drivers = await d ?? []
            teams = await t ?? []
        }
    }
}

// MARK: - Quiz

struct QuizView: View {
    @State private var champions: [Champion] = []
    @State private var answer: Champion?
    @State private var options: [Champion] = []
    @State private var picked: String?
    @State private var score = 0
    @AppStorage("f1x-quiz-best") private var best = 0

    var body: some View {
        List {
            Section {
                HStack {
                    Text(L("Score : \(score)", "Score: \(score)")).bold()
                    Spacer()
                    Text(L("Record : \(best)", "Best: \(best)")).foregroundStyle(.secondary)
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
                                if od.driverId == d.driverId {
                                    score += 1
                                    best = max(best, score)
                                } else {
                                    score = 0
                                }
                            } label: {
                                HStack {
                                    Text(od.fullName).foregroundStyle(.primary)
                                    Spacer()
                                    if picked != nil && od.driverId == d.driverId { Image(systemName: "checkmark.circle.fill").foregroundStyle(.green) }
                                    else if picked == od.driverId { Image(systemName: "xmark.circle.fill").foregroundStyle(.red) }
                                }
                            }
                        }
                    }
                    if picked != nil {
                        Button(L("Question suivante", "Next question")) { newQuestion() }.bold()
                    }
                }
            } else {
                ProgressView()
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
