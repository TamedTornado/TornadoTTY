#!/usr/bin/env swift

import AppKit
import CoreGraphics
import Foundation

func fail(_ message: String, code: Int32 = 2) -> Never {
    FileHandle.standardError.write(Data("error: \(message)\n".utf8))
    exit(code)
}

func screenDisplayID(_ screen: NSScreen) -> CGDirectDisplayID? {
    let key = NSDeviceDescriptionKey("NSScreenNumber")
    return (screen.deviceDescription[key] as? NSNumber)?.uint32Value
}

func isDisplayFamilyName(_ name: String, canonicalName: String) -> Bool {
    if name == canonicalName {
        return true
    }

    let plainPrefix = canonicalName + " ("
    if name.hasPrefix(plainPrefix), name.hasSuffix(")") {
        let start = name.index(name.startIndex, offsetBy: plainPrefix.count)
        let suffix = name[start..<name.index(before: name.endIndex)]
        return Int(suffix) != nil
    }

    for encodedPrefix in [canonicalName + "+%28", canonicalName + "%20%28"] {
        if name.hasPrefix(encodedPrefix), name.hasSuffix("%29") {
            let start = name.index(name.startIndex, offsetBy: encodedPrefix.count)
            let end = name.index(name.endIndex, offsetBy: -3)
            return Int(name[start..<end]) != nil
        }
    }

    return false
}

func matchedScreen(named requestedName: String) -> NSScreen? {
    // Never let a stale "ZenttyTests (N)" duplicate win over the canonical screen.
    if let exact = NSScreen.screens.first(where: { $0.localizedName == requestedName }) {
        return exact
    }

    return NSScreen.screens.first {
        isDisplayFamilyName($0.localizedName, canonicalName: requestedName)
    }
}

func preferredMainDisplayID(testDisplayName: String?) -> CGDirectDisplayID {
    let currentMain = CGMainDisplayID()
    guard let testDisplayName,
          let currentMainScreen = NSScreen.screens.first(where: { screenDisplayID($0) == currentMain }),
          isDisplayFamilyName(currentMainScreen.localizedName, canonicalName: testDisplayName) else {
        return currentMain
    }

    let nonTestScreens = NSScreen.screens.filter {
        !isDisplayFamilyName($0.localizedName, canonicalName: testDisplayName)
    }
    let preferredScreen = nonTestScreens.first {
        guard let displayID = screenDisplayID($0) else { return false }
        return CGDisplayIsBuiltin(displayID) != 0
    } ?? nonTestScreens.first

    return preferredScreen.flatMap(screenDisplayID) ?? currentMain
}

func displayUUID(_ displayID: CGDirectDisplayID) -> String? {
    guard let uuid = CGDisplayCreateUUIDFromDisplayID(displayID) else {
        return nil
    }
    return CFUUIDCreateString(nil, uuid.takeRetainedValue()) as String
}

func topologyStatus(screenName: String, expectedMainDisplayID: CGDirectDisplayID) -> String {
    guard let screen = NSScreen.screens.first(where: { $0.localizedName == screenName }),
          let testDisplay = screenDisplayID(screen) else {
        return "missing"
    }

    var displayCount: UInt32 = 0
    guard CGGetActiveDisplayList(0, nil, &displayCount) == .success else {
        return "invalid"
    }

    var activeDisplays = [CGDirectDisplayID](repeating: 0, count: Int(displayCount))
    guard CGGetActiveDisplayList(displayCount, &activeDisplays, &displayCount) == .success else {
        return "invalid"
    }

    let mirrorsAnotherDisplay = CGDisplayMirrorsDisplay(testDisplay) != kCGNullDirectDisplay
    let isMirrorSource = activeDisplays.contains {
        $0 != testDisplay && CGDisplayMirrorsDisplay($0) == testDisplay
    }

    if mirrorsAnotherDisplay || isMirrorSource {
        return "mirrored"
    }
    if CGMainDisplayID() != expectedMainDisplayID {
        return "main-changed"
    }
    return "ok"
}

let arguments = CommandLine.arguments
guard arguments.count >= 2 else {
    fail("missing virtual-display-state command")
}

switch arguments[1] {
case "raw-main-display-id":
    print(CGMainDisplayID())

case "main-display-identity":
    let testDisplayName = arguments.count == 3 ? arguments[2] : nil
    let displayID = preferredMainDisplayID(testDisplayName: testDisplayName)
    guard let uuid = displayUUID(displayID) else {
        exit(2)
    }
    print("\(uuid)\t\(displayID)")

case "display-id-for-uuid":
    guard arguments.count == 3,
          let uuid = CFUUIDCreateFromString(nil, arguments[2] as CFString) else {
        exit(2)
    }
    let displayID = CGDisplayGetDisplayIDFromUUID(uuid)
    guard displayID != kCGNullDirectDisplay else {
        exit(1)
    }
    print(displayID)

case "matched-screen-name":
    guard arguments.count == 3 else {
        fail("matched-screen-name requires a display name")
    }
    guard let screen = matchedScreen(named: arguments[2]) else {
        exit(1)
    }
    print(screen.localizedName)

case "topology-status":
    guard arguments.count == 4,
          let expectedMainDisplayID = UInt32(arguments[3]) else {
        print("invalid")
        exit(0)
    }
    print(topologyStatus(screenName: arguments[2], expectedMainDisplayID: expectedMainDisplayID))

case "registered-screen-plan":
    guard arguments.count == 3 else {
        fail("registered-screen-plan requires a display name")
    }

    let identifiers = ProcessInfo.processInfo.environment["ZENTTY_REGISTERED_SCREEN_IDENTIFIERS"] ?? ""
    guard let data = "[\(identifiers)]".data(using: .utf8),
          let entries = try? JSONSerialization.jsonObject(with: data) as? [[String: Any]] else {
        exit(2)
    }

    let canonicalName = arguments[2]
    let matches = entries.compactMap { entry -> (tagID: String, displayID: String, isCanonical: Bool)? in
        let name = entry["name"] as? String
        let originalName = entry["originalName"] as? String
        let names = [name, originalName].compactMap { $0 }
        guard names.contains(where: {
            isDisplayFamilyName($0, canonicalName: canonicalName)
        }), let tagID = entry["tagID"] as? String else {
            return nil
        }
        let isCanonical = names.contains(canonicalName)
        return (tagID, entry["displayID"] as? String ?? "0", isCanonical)
    }

    let canonicalMatches = matches.filter(\.isCanonical)
    guard canonicalMatches.count <= 1 else {
        exit(3)
    }
    guard !matches.isEmpty else {
        exit(1)
    }
    if let canonical = canonicalMatches.first {
        print("canonical:\(canonical.tagID):\(canonical.displayID)")
    }
    for legacy in matches.filter({ !$0.isCanonical }) {
        print("legacy:\(legacy.tagID):\(legacy.displayID)")
    }

default:
    fail("unknown virtual-display-state command '\(arguments[1])'")
}
