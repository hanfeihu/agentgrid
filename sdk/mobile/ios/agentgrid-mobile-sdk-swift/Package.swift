// swift-tools-version: 5.7

import PackageDescription

let package = Package(
    name: "AgentGridMobileSDK",
    platforms: [
        .iOS(.v15),
        .macOS(.v12)
    ],
    products: [
        .library(name: "AgentGridMobileSDK", targets: ["AgentGridMobileSDK"])
    ],
    targets: [
        .target(name: "AgentGridMobileSDK")
    ]
)

