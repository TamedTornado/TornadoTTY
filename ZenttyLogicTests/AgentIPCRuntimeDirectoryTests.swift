import Darwin
import XCTest
@testable import Zentty

/// Covers the lifecycle of the IPC runtime directory: the socket path staying
/// stable across a rebind, and stale directories being reaped by ownership
/// rather than by pid.
///
/// Regression origin: a cache cleaner deleted `~/Library/Caches/Zentty/ipc-*/`
/// out from under a running instance. The listening descriptor stayed bound to
/// the now-unlinked inode, so the app looked healthy while every pane's
/// `connect()` failed with `ENOENT` — silently, because the shell integration
/// discards its own errors. cwd and git-branch reporting died app-wide, and new
/// panes inherited the same dead path, so only an app restart recovered.
final class AgentIPCRuntimeDirectoryTests: XCTestCase {

    private var baseDirectory: URL!

    override func setUpWithError() throws {
        try super.setUpWithError()
        // Deliberately short and *not* NSTemporaryDirectory(): a `sockaddr_un`
        // path is capped at 104 bytes, and `/var/folders/…/T/` plus a UUID
        // already overruns it before the socket name is appended.
        baseDirectory = URL(fileURLWithPath: "/tmp", isDirectory: true)
            .appendingPathComponent("zt-\(UUID().uuidString.prefix(8))", isDirectory: true)
        try FileManager.default.createDirectory(at: baseDirectory, withIntermediateDirectories: true)
    }

    override func tearDownWithError() throws {
        try? FileManager.default.removeItem(at: baseDirectory)
        baseDirectory = nil
        try super.tearDownWithError()
    }

    private func makeServer(instanceID: String = "svrinstance") -> AgentIPCServer {
        let server = AgentIPCServer(instanceID: instanceID, baseRuntimeDirectory: baseDirectory)
        addTeardownBlock { server.stop() }
        return server
    }

    private var currentPID: Int32 { ProcessInfo.processInfo.processIdentifier }

    /// Create a runtime directory as another instance would leave it behind.
    @discardableResult
    private func makeForeignRuntimeDirectory(named name: String, withLockFile: Bool) throws -> URL {
        let directory = baseDirectory.appendingPathComponent(name, isDirectory: true)
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        if withLockFile {
            // Built from the production constant so a rename cannot silently
            // decouple these fixtures from what the reaper actually looks for.
            let lockPath = directory
                .appendingPathComponent(AgentIPCServer.lockFileName, isDirectory: false)
                .path
            let descriptor = open(lockPath, O_CREAT | O_RDWR, 0o600)
            XCTAssertGreaterThanOrEqual(descriptor, 0, "failed to create lock fixture")
            close(descriptor)
        }
        return directory
    }

    private func exists(_ url: URL) -> Bool {
        FileManager.default.fileExists(atPath: url.path)
    }

    /// Whether a client can actually reach the listener, as the `zentty` CLI
    /// does from a pane. Distinct from the socket file merely existing — the
    /// original bug left a descriptor bound to an unlinked inode, which looks
    /// perfectly healthy from inside the app and is unreachable from outside.
    private func canConnect(to path: String) -> Bool {
        let descriptor = socket(AF_UNIX, SOCK_STREAM, 0)
        guard descriptor >= 0 else {
            return false
        }
        defer { close(descriptor) }

        var address = sockaddr_un()
        address.sun_family = sa_family_t(AF_UNIX)
        let utf8Path = path.utf8CString
        guard utf8Path.count <= MemoryLayout.size(ofValue: address.sun_path) else {
            return false
        }
        _ = withUnsafeMutablePointer(to: &address.sun_path.0) { pointer in
            utf8Path.withUnsafeBufferPointer { buffer in
                memcpy(pointer, buffer.baseAddress, buffer.count)
            }
        }

        let result = withUnsafePointer(to: &address) {
            $0.withMemoryRebound(to: sockaddr.self, capacity: 1) {
                Darwin.connect(descriptor, $0, socklen_t(MemoryLayout<sockaddr_un>.size))
            }
        }
        return result == 0
    }

    // MARK: - Rebinding after external deletion

    func test_socket_rebinds_at_the_same_path_after_the_runtime_directory_is_deleted() throws {
        let server = makeServer()

        let originalPath = try XCTUnwrap(server.startIfNeeded(), "server failed to start")
        XCTAssertTrue(FileManager.default.fileExists(atPath: originalPath), "socket should exist after start")
        let runtimeDirectory = try XCTUnwrap(server.currentRuntimeDirectoryURL())

        // Simulate CleanMyMac / an OS purge / a stray `rm -rf`.
        try FileManager.default.removeItem(at: runtimeDirectory)
        XCTAssertFalse(FileManager.default.fileExists(atPath: originalPath))

        let rebound = server.startIfNeeded()

        XCTAssertEqual(
            rebound,
            originalPath,
            "the socket must come back at the SAME path: a pane shell captured ZENTTY_INSTANCE_SOCKET when it "
                + "spawned and its environment cannot be rewritten, so a fresh path would strand every running pane"
        )
        XCTAssertTrue(
            FileManager.default.fileExists(atPath: originalPath),
            "rebinding must recreate the socket on disk, not just report a path"
        )
    }

    func test_clients_can_reach_the_socket_again_after_a_rebind() throws {
        let server = makeServer()
        let path = try XCTUnwrap(server.startIfNeeded())
        XCTAssertTrue(canConnect(to: path), "a freshly started server must be reachable")

        try FileManager.default.removeItem(at: try XCTUnwrap(server.currentRuntimeDirectoryURL()))
        XCTAssertFalse(
            canConnect(to: path),
            "with the path unlinked every connect() fails — this is precisely what users hit, while the app "
                + "still holds a bound descriptor and believes it is serving"
        )

        _ = server.startIfNeeded()

        XCTAssertTrue(
            canConnect(to: path),
            "after rebinding, the path a running pane captured at spawn must be reachable again without a restart"
        )
    }

    func test_socket_path_is_derived_from_instance_id_so_it_survives_a_rebind() throws {
        let server = makeServer(instanceID: "stableinstanceid")

        let originalPath = try XCTUnwrap(server.startIfNeeded())
        let runtimeDirectory = try XCTUnwrap(server.currentRuntimeDirectoryURL())
        XCTAssertEqual(runtimeDirectory.lastPathComponent, "ipc-\(currentPID)-stablein")

        try FileManager.default.removeItem(at: runtimeDirectory)
        _ = server.startIfNeeded()

        XCTAssertEqual(
            server.currentRuntimeDirectoryURL()?.lastPathComponent,
            "ipc-\(currentPID)-stablein",
            "the directory name must not be re-randomised on rebind"
        )
        XCTAssertEqual(server.startIfNeeded(), originalPath)
    }

    func test_repeated_start_calls_are_idempotent_while_the_socket_is_healthy() throws {
        let server = makeServer()

        let first = try XCTUnwrap(server.startIfNeeded())
        let second = try XCTUnwrap(server.startIfNeeded())

        XCTAssertEqual(first, second)
    }

    func test_stop_removes_the_runtime_directory() throws {
        let server = makeServer()
        _ = server.startIfNeeded()
        let runtimeDirectory = try XCTUnwrap(server.currentRuntimeDirectoryURL())

        server.stop()

        XCTAssertFalse(exists(runtimeDirectory), "a clean shutdown must not leave its runtime directory behind")
    }

    func test_a_regular_file_left_at_the_socket_path_is_treated_as_missing() throws {
        let server = makeServer()
        let path = try XCTUnwrap(server.startIfNeeded())
        let runtimeDirectory = try XCTUnwrap(server.currentRuntimeDirectoryURL())

        // Something replaced the socket with an ordinary file — a restored
        // backup, a botched sync. An existence check would call this healthy.
        try FileManager.default.removeItem(at: runtimeDirectory)
        try FileManager.default.createDirectory(at: runtimeDirectory, withIntermediateDirectories: true)
        try Data().write(to: URL(fileURLWithPath: path))

        XCTAssertEqual(server.startIfNeeded(), path)
        XCTAssertTrue(
            canConnect(to: path),
            "a non-socket at the bound path must trigger a rebind, not be mistaken for a working listener"
        )
    }

    func test_start_recovers_after_an_earlier_failure() throws {
        // A regular file where the runtime root belongs, so creating the
        // directory fails the way a full disk or a permissions problem would.
        let blockedRoot = baseDirectory.appendingPathComponent("blocked", isDirectory: true)
        try Data().write(to: blockedRoot)

        let server = AgentIPCServer(instanceID: "recoverable", baseRuntimeDirectory: blockedRoot)
        addTeardownBlock { server.stop() }

        XCTAssertNil(server.startIfNeeded(), "a start that cannot create its directory must fail")

        try FileManager.default.removeItem(at: blockedRoot)

        XCTAssertNotNil(
            server.startIfNeeded(),
            "a failed start must not poison later attempts — otherwise a transient problem mutes the instance "
                + "until the app is restarted, which is the failure mode this whole change exists to remove"
        )
    }

    // MARK: - Reaping by ownership, not by pid

    func test_reaper_keeps_a_directory_whose_lock_is_held() throws {
        // The pid in the name is deliberately dead, so only the lock can save
        // this directory. With a live pid the test would still pass if the lock
        // check regressed and fell through to the pid fallback.
        let directory = try makeForeignRuntimeDirectory(named: "ipc-999999-liveowner", withLockFile: true)
        let lockPath = directory
            .appendingPathComponent(AgentIPCServer.lockFileName, isDirectory: false)
            .path

        // A separate open file description, so this conflicts with the reaper's
        // attempt exactly as another process's lock would.
        let descriptor = open(lockPath, O_RDWR)
        XCTAssertGreaterThanOrEqual(descriptor, 0)
        XCTAssertEqual(flock(descriptor, LOCK_EX | LOCK_NB), 0, "test failed to take the lock it means to hold")
        defer { close(descriptor) }

        _ = makeServer().startIfNeeded()

        XCTAssertTrue(exists(directory), "a directory with a held lock belongs to a live instance and must survive")
    }

    func test_reaper_removes_a_directory_whose_lock_is_not_held_even_when_its_pid_is_alive() throws {
        // The pid-reuse case the old `kill(pid, 0)` check got wrong: the pid in
        // the name is live (it is this very test process) but no one owns the
        // directory, so it leaked forever.
        let directory = try makeForeignRuntimeDirectory(named: "ipc-\(currentPID)-abandoned", withLockFile: true)

        _ = makeServer().startIfNeeded()

        XCTAssertFalse(
            exists(directory),
            "an unlocked directory is abandoned regardless of whether its pid has been recycled"
        )
    }

    func test_reaper_never_removes_its_own_directory() throws {
        let server = makeServer()
        _ = server.startIfNeeded()
        let ownDirectory = try XCTUnwrap(server.currentRuntimeDirectoryURL())

        // A second start re-runs the sweep while this instance is the owner.
        _ = server.startIfNeeded()

        XCTAssertTrue(exists(ownDirectory))
    }

    // MARK: - Legacy directories (no lock file)

    func test_reaper_falls_back_to_the_pid_check_when_there_is_no_lock_file() throws {
        let liveOwner = try makeForeignRuntimeDirectory(named: "ipc-\(currentPID)-legacy", withLockFile: false)
        let deadOwner = try makeForeignRuntimeDirectory(named: "ipc-999999-legacy", withLockFile: false)

        _ = makeServer().startIfNeeded()

        XCTAssertTrue(
            exists(liveOwner),
            "a directory from a build that predates locking is still protected by its pid while that pid is alive"
        )
        XCTAssertFalse(exists(deadOwner), "and reaped once the pid is gone")
    }

    // MARK: - Path length

    func test_start_fails_cleanly_when_the_socket_path_exceeds_the_unix_domain_limit() throws {
        let longComponent = String(repeating: "d", count: 120)
        let longBase = baseDirectory.appendingPathComponent(longComponent, isDirectory: true)
        let server = AgentIPCServer(instanceID: "toolongpath", baseRuntimeDirectory: longBase)
        addTeardownBlock { server.stop() }

        XCTAssertNil(
            server.startIfNeeded(),
            "a socket path over the 104-byte sockaddr_un limit must fail rather than bind a truncated path"
        )
    }

    // MARK: - Runtime overlay path recognition

    func test_runtime_overlay_paths_are_recognised_under_both_the_current_and_legacy_roots() {
        let home = NSHomeDirectory()

        XCTAssertTrue(
            ZenttyRuntimePaths.isRuntimeOverlayPath(
                "\(home)/.config/zentty/run/ipc-123-abcdefgh/launch/wl_x/pn_y/kimi/home"
            ),
            "overlays under the current root must be recognised"
        )
        XCTAssertTrue(
            ZenttyRuntimePaths.isRuntimeOverlayPath(
                "\(home)/Library/Caches/Zentty/ipc-123-abcdefgh/launch/wl_x/pn_y/kimi/home"
            ),
            "overlays under the legacy root must stay recognised — an agent started before the move still has the "
                + "old path in KIMI_CODE_HOME, and treating it as a real user home corrupts the user's config"
        )
        XCTAssertFalse(
            ZenttyRuntimePaths.isRuntimeOverlayPath("\(home)/.kimi-code"),
            "a real user home is not an overlay"
        )
        XCTAssertFalse(
            ZenttyRuntimePaths.isRuntimeOverlayPath("\(home)/.config/zentty"),
            "the config directory itself is not a runtime overlay"
        )
    }

    func test_the_runtime_root_that_is_created_is_the_same_one_overlay_matching_looks_for() {
        let home = URL(fileURLWithPath: "/Users/example", isDirectory: true)

        let currentRoot = ZenttyRuntimePaths.currentRootURL(homeDirectory: home)
        XCTAssertEqual(currentRoot.path, "/Users/example/.config/zentty/run")
        XCTAssertTrue(
            ZenttyRuntimePaths.isRuntimeOverlayPath(
                currentRoot.appendingPathComponent("ipc-1-abcdefgh/launch/wl/pn/kimi/home").path
            ),
            "a path under the root the server actually creates must be recognised as an overlay — if these two "
                + "drifted apart nothing would fail to build, and stale overlays would quietly stop being detected"
        )

        let legacyRoot = ZenttyRuntimePaths.legacyRootURL(homeDirectory: home)
        XCTAssertEqual(legacyRoot.path, "/Users/example/Library/Caches/Zentty")
        XCTAssertTrue(
            ZenttyRuntimePaths.isRuntimeOverlayPath(
                legacyRoot.appendingPathComponent("ipc-1-abcdefgh/launch/wl/pn/kimi/home").path
            )
        )
    }
}
