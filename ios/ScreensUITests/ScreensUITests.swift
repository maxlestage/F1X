import XCTest

/// Parcourt toute l'app (en français, thème sombre) et prend une capture de chaque page.
/// Les captures sont écrites dans le dossier `SHOTS_DIR` (CI) et jointes au résultat du test.
/// Aucune vérification ne fait échouer le parcours : on veut voir chaque page, même si une étape rate.
final class ScreensUITests: XCTestCase {
    private var app = XCUIApplication()
    private var index = 0

    override func setUp() {
        continueAfterFailure = true
        app.launchArguments += ["-AppleLanguages", "(fr)", "-AppleLocale", "fr_FR"]
        // Alertes système (notifications…) : on les ferme pour continuer.
        addUIInterruptionMonitor(withDescription: "Alertes") { alert in
            for label in ["Autoriser", "Allow", "OK", "Ne pas autoriser", "Don't Allow"] where alert.buttons[label].exists {
                alert.buttons[label].tap()
                return true
            }
            return false
        }
    }

    func testToutesLesPages() {
        app.launch()
        sleep(6) // ouverture animée + premières données
        shot("accueil")
        scrollShots("accueil", count: 3)

        // Prochain Grand Prix depuis l'accueil.
        scrollToTop()
        if tap(app.buttons["Voir le Grand Prix"]) {
            sleep(5)
            shot("gp-prochain")
            scrollShots("gp-prochain", count: 3)
            back()
        }

        // Direct : accueil du direct puis un replay (classement, carte, pneus, fil).
        tab("Direct")
        sleep(5)
        shot("direct")
        let session = app.buttons.matching(NSPredicate(format: "label CONTAINS ' · '")).firstMatch
        if tap(session) {
            sleep(10)
            shot("direct-replay-ordre")
            for (name, segment) in [("carte", "Carte"), ("pneus", "Pneus"), ("fil", "Fil")] {
                if tap(app.buttons[segment]) { sleep(3); shot("direct-replay-\(name)") }
            }
            if tap(app.buttons["Ordre"]) { sleep(1) }
            app.swipeDown()
            if tap(app.buttons["Quitter"]) { sleep(2) }
        }

        // Calendrier puis une course disputée.
        tab("Calendrier")
        sleep(5)
        shot("calendrier")
        scrollToTop()
        shot("calendrier-haut")
        let race = app.staticTexts.matching(NSPredicate(format: "label CONTAINS '🏆'")).firstMatch
        if tap(race) {
            sleep(6)
            shot("course")
            scrollShots("course", count: 5)
            back()
        }

        // Classements : un pilote, puis les écuries et une écurie.
        tab("Classements")
        sleep(5)
        shot("classements-pilotes")
        if tap(app.cells.element(boundBy: 3)) {
            sleep(6)
            shot("pilote")
            scrollShots("pilote", count: 3)
            back()
        }
        if tap(app.buttons["Écuries"]) {
            sleep(3)
            shot("classements-ecuries")
            if tap(app.cells.element(boundBy: 3)) {
                sleep(6)
                shot("ecurie")
                scrollShots("ecurie", count: 3)
                back()
            }
        }

        // Explorer : chaque rubrique.
        tab("Explorer")
        sleep(2)
        shot("explorer")
        let pages: [(String, String)] = [
            ("champions", "Champions du monde depuis 1950"),
            ("records", "Records"),
            ("saisons", "Toutes les saisons"),
            ("donnees", "Données OpenF1 (télémétrie, pneus, radios…)"),
            ("comparateur", "Comparateur de pilotes"),
            ("actus", "Actualités"),
            ("lexique", "Lexique"),
            ("pronostics", "Pronostics"),
            ("fantasy", "Fantasy F1"),
            ("quiz", "Quiz : devine le pilote"),
        ]
        for (name, label) in pages {
            tab("Explorer")
            let link = app.buttons[label].exists ? app.buttons[label] : app.staticTexts[label]
            guard tap(link) else { continue }
            sleep(6)
            shot(name)
            scrollShots(name, count: 2)
            if name == "quiz" {
                scrollToTop()
                let option = app.buttons.matching(NSPredicate(format: "label MATCHES '^[A-Z][a-zé]+ .+'")).element(boundBy: 0)
                if tap(option) { sleep(1); shot("quiz-reponse") }
            }
            back()
        }
    }

    // MARK: - Outils

    private func shot(_ name: String) {
        index += 1
        let file = String(format: "%02d-%@", index, name)
        let screenshot = XCUIScreen.main.screenshot()
        let attachment = XCTAttachment(screenshot: screenshot)
        attachment.name = file
        attachment.lifetime = .keepAlways
        add(attachment)
        if let dir = ProcessInfo.processInfo.environment["SHOTS_DIR"] {
            try? screenshot.pngRepresentation.write(to: URL(fileURLWithPath: dir).appendingPathComponent("\(file).png"))
        }
    }

    private func scrollShots(_ name: String, count: Int) {
        for i in 1...count {
            app.swipeUp(velocity: .slow)
            sleep(2)
            shot("\(name)-\(i)")
        }
    }

    private func scrollToTop() {
        for _ in 0..<6 { app.swipeDown(velocity: .fast) }
        sleep(1)
    }

    @discardableResult
    private func tap(_ element: XCUIElement) -> Bool {
        guard element.waitForExistence(timeout: 8) else { return false }
        element.tap()
        return true
    }

    private func tab(_ label: String) {
        let button = app.tabBars.buttons[label]
        if button.waitForExistence(timeout: 5) {
            button.tap()
            button.tap() // second toucher : revient à la racine de l'onglet
        } else {
            app.buttons[label].firstMatch.tap()
        }
        sleep(1)
    }

    private func back() {
        let bar = app.navigationBars.firstMatch
        if bar.buttons.element(boundBy: 0).exists {
            bar.buttons.element(boundBy: 0).tap()
        } else {
            app.swipeRight()
        }
        sleep(2)
    }
}
