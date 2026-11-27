import AppKit
import AVFAudio
import AVFoundation
import AudioToolbox
import MediaPlayer
import MusicKit
import ImageIO
import iTunesLibrary
import UniformTypeIdentifiers

struct PlayerError: LocalizedError {
    let message: String
    init(_ message: String) { self.message = message }
    var errorDescription: String? { message }
}

// Only failures belonging to one recording can advance the queue. Route,
// authorization and connection failures must be handled by the listener.
struct UnavailableTrack: LocalizedError {
    let reason: String
    init(_ reason: String) { self.reason = reason }
    var errorDescription: String? { reason }

    static func matches(_ error: Error) -> Bool {
        if error is UnavailableTrack { return true }
        var cause: NSError? = error as NSError
        for _ in 0..<6 {
            guard let e = cause else { break }
            if e.domain == MPErrorDomain {
                if e.code == MPError.Code.notFound.rawValue {return true}
                if [MPError.Code.permissionDenied, .cloudServiceCapabilityMissing, .networkConnectionFailed, .requestTimedOut,
                    .cancelled].contains(where: { $0.rawValue == e.code }) {return false}
            }
            if e.domain == NSOSStatusErrorDomain && [kAudioFileInvalidFileError,kAudioFileFileNotFoundError,
                kAudioFileUnsupportedFileTypeError,kAudioFileUnsupportedDataFormatError].contains(where: {Int($0) == e.code}) {return true}
            if e.domain == NSCocoaErrorDomain && [NSFileReadNoSuchFileError, NSFileNoSuchFileError].contains(e.code) { return true }
            if e.domain == AVFoundationErrorDomain && [
                AVError.Code.fileFormatNotRecognized, .fileFailedToParse, .decoderNotFound, .incompatibleAsset, .invalidSourceMedia, .noLongerPlayable
            ].contains(where: { $0.rawValue == e.code }) { return true }
            // Some MusicKit builds wrap catalog failures in an opaque player error.
            let detail = [e.localizedDescription, e.localizedFailureReason ?? "",
                          e.userInfo[NSDebugDescriptionErrorKey] as? String ?? ""].joined(separator: " ").lowercased()
            if ["no playable items", "no playable content", "item is not playable", "track is unavailable",
                "not available in your country or region"].contains(where: detail.contains) { return true }
            cause = e.userInfo[NSUnderlyingErrorKey] as? NSError
        }
        return false
    }
}

typealias EventSink = @convention(c) (UnsafePointer<CChar>) -> Void
private func emit(_ sink: EventSink?, _ kind: String, _ data: [String: Any] = [:]) {
    var event = data
    event["kind"] = kind
    if let bytes = try? JSONSerialization.data(withJSONObject: event) {
        String(decoding:bytes,as:UTF8.self).withCString { sink?($0) }
    }
}

private actor Artwork {
    static let shared = Artwork()
    static let directory = Library.cache.deletingLastPathComponent().appendingPathComponent("covers-v1")
    func load(ids: [String], sink: EventSink?) throws {
        let fm = FileManager.default
        try fm.createDirectory(at:Self.directory,withIntermediateDirectories:true)
        var pending = Set(ids.filter { $0.count == 16 && $0.allSatisfy(\.isHexDigit) && !fm.fileExists(atPath:Self.directory.appendingPathComponent($0+".jpg").path) })
        guard !pending.isEmpty else { return }
        let library = try ITLibrary(apiVersion:"1.0")
        for item in library.allMediaItems {
            let id = String(format:"%016llX",item.persistentID.uint64Value)
            guard pending.remove(id) != nil else { continue }
            autoreleasepool {
                guard let data = item.artwork?.imageData, let source = CGImageSourceCreateWithData(data as CFData,nil),
                      let thumb = CGImageSourceCreateThumbnailAtIndex(source,0,[kCGImageSourceCreateThumbnailFromImageAlways:true,kCGImageSourceThumbnailMaxPixelSize:520,kCGImageSourceCreateThumbnailWithTransform:true] as CFDictionary) else { return }
                let output = NSMutableData()
                guard let dest = CGImageDestinationCreateWithData(output,UTType.jpeg.identifier as CFString,1,nil) else { return }
                CGImageDestinationAddImage(dest,thumb,[kCGImageDestinationLossyCompressionQuality:0.88] as CFDictionary)
                if CGImageDestinationFinalize(dest), (try? (output as Data).write(to:Self.directory.appendingPathComponent(id+".jpg"),options:.atomic)) != nil { emit(sink,"cover",["id":id]) }
            }
            if pending.isEmpty { break }
        }
    }
}

@MainActor private final class Engine: NSObject, AVAudioPlayerDelegate {
    static let shared = Engine()
    var sink: EventSink?
    var local: AVAudioPlayer?
    var filtered: EqualizedFile?
    var fileURL: URL?
    var gains = EqualizerGains(low: 0, mid: 0, high: 0)
    var kitActive = false
    var player: ApplicationMusicPlayer { .shared }
    var id = "", source = "", playing = false
    var sourceRate: Double?
    var timer: Timer?
    var generation = 0
    var transitioning = false
    var matchedDevice: String?
    let matcher = RateMatcher()
    var songs: [String: Song] = [:]
    func info(_ kind: String = "state") {
        let device = OutputDevice.current()
        let variant = kitActive ? player.state.audioVariant?.description : nil
        emit(sink,kind,["id":id,"playing":playing,"source":source,"active":variant ?? (kitActive ? "Unreported by MusicKit" : ""),
                        "output":device?.summary ?? "Output unavailable","matching":matchedDevice != nil,
                        "eqRoute":kitActive ? "MusicKit bypass" : filtered != nil ? "Active on local file" : "Bypassed",
                        "position":filtered?.position ?? local?.currentTime ?? (kitActive ? player.playbackTime : 0),
                        "duration":filtered?.duration ?? local?.duration ?? 0])
    }
    func stop() {
        timer?.invalidate(); timer = nil
        filtered?.stop(); filtered = nil; fileURL = nil
        local?.stop(); local = nil
        if kitActive { player.stop(); kitActive = false }
        playing = false
    }
    func play(_ track: Track, prepareOnly: Bool) async throws {
        guard !transitioning else { throw PlayerError("Playback is already loading.") }
        transitioning = true; defer { transitioning = false }
        generation += 1
        stop(); id = track.id; source = ""; sourceRate = nil
        if let file = track.file {
            do { _ = try file.resourceValues(forKeys: [.isRegularFileKey]) }
            catch { if UnavailableTrack.matches(error) { throw UnavailableTrack("Audio file is missing.") }; throw error }
            let q = AudioQuality.file(file); source = q.label; sourceRate = q.hz
            if !prepareOnly { try await match() }
            try openFile(file, gains: gains, position: 0, audible: !prepareOnly)

        } else {
            let authorization = prepareOnly ? MusicAuthorization.currentStatus : await MusicAuthorization.request()
            guard authorization == .authorized else { throw PlayerError("Allow Music access in System Settings → Privacy & Security → Media & Apple Music.") }
            let song: Song
            if let cached = songs[track.id] { song = cached } else {
                song = try await SongResolver.resolve(track)
                if songs.count >= 64 {songs.removeAll(keepingCapacity:true)}
                songs[track.id] = song
            }
            if let url = track.package, let q = AudioQuality.download(url) { source = "Download · " + q.label; sourceRate = q.hz }
            else { source = "Apple Music stream" }
            player.queue = [song]; kitActive = true
            if prepareOnly && ProcessInfo.processInfo.operatingSystemVersion.majorVersion >= 26 {
                if #available(macOS 26.4, *) { player.queue.affectsListeningHistory = false }
            }
            if prepareOnly {try await player.prepareToPlay()} else {try await match(); try await player.play()}
        }
        if prepareOnly { info("prepared"); stop(); return }
        playing = true; info("started")
        timer = Timer.scheduledTimer(withTimeInterval: 1, repeats:true) { [weak self] _ in
            MainActor.assumeIsolated {
                guard let self, !self.transitioning else { return }
                if self.kitActive {
                    let status = self.player.state.playbackStatus
                    if status == .stopped { self.finish(); return }
                    self.playing = status == .playing
                }
                self.info()
            }
        }
    }
    func openFile(_ url: URL, gains: EqualizerGains, position: Double, audible: Bool) throws {
        // Build the replacement first so a decoder or graph error preserves the existing player.
        let previousLocal = local, previousEQ = filtered
        if gains.flat {
            let next = try AVAudioPlayer(contentsOf: url)
            guard next.prepareToPlay() else { throw UnavailableTrack("The file could not be prepared.") }
            next.currentTime = position
            previousEQ?.pause()
            if audible && !next.play() {
                if let previousEQ { try previousEQ.resume() }
                throw PlayerError("Audio output could not start.")
            }
            next.delegate = self
            local = next; filtered = nil
        } else {
            let next = try EqualizedFile(url: url, gains: gains)
            next.seek(position)
            next.onEnd = { [weak self, weak next] in
                guard let self, let next, next === self.filtered else { return }
                self.finish()
            }
            previousLocal?.pause()
            do { if audible { try next.resume() } }
            catch { if audible { _ = previousLocal?.play() }; throw error }
            filtered = next; local = nil
        }
        previousEQ?.stop(); previousLocal?.stop()
        fileURL = url
    }
    func setEQ(_ next: EqualizerGains) throws {
        guard [next.low,next.mid,next.high].allSatisfy({ $0.isFinite && abs($0) <= 6 }) else {
            throw PlayerError("Equalizer bands must be between −6 and +6 dB.")
        }
        if let url = fileURL, next != gains {
            if let filtered, !next.flat { next.apply(to: filtered.equalizer) }
            else { try openFile(url, gains: next, position: filtered?.position ?? local?.currentTime ?? 0, audible: playing) }
        }
        gains = next
        info()
    }
    func match() async throws {
        guard let uid = matchedDevice else {return}
        guard let device = OutputDevice.current(), device.uid == uid else { matchedDevice = nil; throw PlayerError("The selected headphone output changed. Playback was stopped.") }
        if let hz = sourceRate, abs(device.rate-hz)>1 {try await matcher.apply(device,hz:hz)}
    }
    func finish() {timer?.invalidate(); timer = nil; playing = false; info("ended")}
    nonisolated func audioPlayerDidFinishPlaying(_ player:AVAudioPlayer,successfully flag:Bool) {
        Task { @MainActor in guard player === self.local else {return}; if flag {self.finish()} else {self.decodeFailed(player)} }
    }
    private func decodeFailed(_ player: AVAudioPlayer) {
        guard player === local else { return }
        stop()
        emit(sink, "unavailable", ["id":id])
    }
    nonisolated func audioPlayerDecodeErrorDidOccur(_ player: AVAudioPlayer, error: Error?) {
        Task { @MainActor in self.decodeFailed(player) }
    }
    func command(_ method: String, _ bytes: Data) async throws {
        switch method {
        case "play","prepare": do {try await play(JSONDecoder().decode(Track.self,from:bytes),prepareOnly:method=="prepare")} catch {stop(); info(); throw error}
        case "toggle":
            guard !transitioning else {return}
            if let filtered {
                if playing { filtered.pause() } else { try filtered.resume() }
                playing.toggle()
            } else if let local {if local.isPlaying {local.pause()} else {guard local.play() else {throw PlayerError("Could not resume file.")}}; playing = local.isPlaying}
            else if kitActive { if playing {player.pause(); playing=false} else {try await player.play(); playing=true} }
            info()
        case "seek":
            guard !transitioning else { return }
            let t = try JSONDecoder().decode(Double.self,from:bytes)
            if let filtered { filtered.seek(t); if playing {try filtered.resume()} }
            else if let local { local.currentTime = max(0,min(t,local.duration)) }
            else if kitActive { player.playbackTime = max(0,t) }
            info()
        case "eq":
            try setEQ(JSONDecoder().decode(EqualizerGains.self, from: bytes))
        case "rate": if try JSONDecoder().decode(Bool.self,from:bytes) {
            guard let dev=OutputDevice.current(),dev.canMatch else {throw PlayerError("Select wired headphones or a DAC first.")}
            matchedDevice=dev.uid
        } else {matchedDevice=nil; matcher.restore(wait:false)}; info()
        case "refresh": let dest=try await Task.detached {let lib=try Library.read(); try lib.save(); return Library.cache.path}.value; emit(sink,"library",["path":dest])
        case "covers": try await Artwork.shared.load(ids:JSONDecoder().decode([String].self,from:bytes),sink:sink)
        case "open":
            let p=NSOpenPanel(); p.allowedContentTypes=[.audio]; p.allowsMultipleSelection=true
            if p.runModal() == .OK { let json=try JSONEncoder().encode(p.urls.map(Track.loose)); emit(sink,"files",["tracks":try JSONSerialization.jsonObject(with:json)]) }
        case "info": info()
        default: break
        }
    }
}
@_cdecl("pw_connect") public func pwConnect(_ callback: @convention(c) (UnsafePointer<CChar>)->Void) { MainActor.assumeIsolated { Engine.shared.sink = callback } }
@_cdecl("pw_send") public func pwSend(_ method: UnsafePointer<CChar>,_ text: UnsafePointer<CChar>) {
    let name=String(cString:method), bytes=Data(String(cString:text).utf8)
    Task { @MainActor in do {try await Engine.shared.command(name,bytes)} catch {
        let engine = Engine.shared
        if (name == "play" || name == "toggle") && UnavailableTrack.matches(error) {
            engine.stop();
            emit(engine.sink,"unavailable",["id":engine.id])
        } else { emit(engine.sink,"error",["action":name,"message":error.localizedDescription]) }
    } }
}
@_cdecl("pw_pump") public func pwPump() { RunLoop.current.run(until:Date(timeIntervalSinceNow:0.02)) }
@_cdecl("pw_shutdown") public func pwShutdown() { MainActor.assumeIsolated { let e=Engine.shared; e.stop(); e.matcher.restore(wait:true) } }

@_cdecl("pw_activate_existing") public func activateExisting() {
    guard let identifier = Bundle.main.bundleIdentifier else {return}
    let me = ProcessInfo.processInfo.processIdentifier
    let other = NSRunningApplication.runningApplications(withBundleIdentifier: identifier)
        .first { $0.processIdentifier != me && !$0.isTerminated }
    _ = other?.activate(options: [])
}
