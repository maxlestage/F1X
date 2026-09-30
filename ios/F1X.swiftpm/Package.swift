// swift-tools-version: 5.9

// Package au format « App Swift Playgrounds » : s'ouvre dans Xcode (Mac)
// et dans Swift Playgrounds (iPad) pour lancer / publier l'app sans Mac.

import PackageDescription
import AppleProductTypes

let package = Package(
    name: "F1X",
    platforms: [
        .iOS("17.0")
    ],
    products: [
        .iOSApplication(
            name: "F1X",
            targets: ["AppModule"],
            bundleIdentifier: "com.maxlestage.f1x",
            teamIdentifier: "",
            displayVersion: "1.0",
            bundleVersion: "1",
            appIcon: .placeholder(icon: .car),
            accentColor: .presetColor(.red),
            supportedDeviceFamilies: [
                .pad,
                .phone
            ],
            supportedInterfaceOrientations: [
                .portrait
            ],
            appCategory: .sports
        )
    ],
    targets: [
        .executableTarget(
            name: "AppModule",
            path: "."
        )
    ]
)
