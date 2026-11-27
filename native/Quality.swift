import AppKit
import CoreAudio
import AudioToolbox
import MusicKit

struct AudioQuality {
    var label: String
    var hz: Double?
    static func rate(_ hz: Double) -> String { String(format: "%g kHz", hz / 1000) }
    static func file(_ url: URL) -> Self {
        var id: AudioFileID?
        guard AudioFileOpenURL(url as CFURL, .readPermission, 0, &id) == noErr, let id else { return .init(label: "File format unavailable") }
        defer { AudioFileClose(id) }
        var format = AudioStreamBasicDescription(), size = UInt32(MemoryLayout<AudioStreamBasicDescription>.size)
        guard AudioFileGetProperty(id, kAudioFilePropertyDataFormat, &size, &format) == noErr else { return .init(label: "File format unavailable") }
        let codec: String
        switch format.mFormatID {
        case kAudioFormatAppleLossless: codec = "Lossless ALAC"
        case kAudioFormatFLAC: codec = "Lossless FLAC"
        case kAudioFormatLinearPCM: codec = "Linear PCM"
        case kAudioFormatMPEGLayer3: codec = "MP3"
        case kAudioFormatMPEG4AAC: codec = "AAC"
        default: codec = url.pathExtension.uppercased()
        }
        var bits: Int32 = 0
        size = UInt32(MemoryLayout<Int32>.size)
        if AudioFileGetProperty(id, kAudioFilePropertySourceBitDepth, &size, &bits) != noErr || bits == 0 {
            bits = Int32(format.mBitsPerChannel)
            if format.mFormatID == kAudioFormatAppleLossless { bits = [1:16, 2:20, 3:24, 4:32][format.mFormatFlags] ?? bits }
        }
        let depth = bits == 0 ? nil : "\(abs(bits))-bit" + (bits < 0 ? " float" : "")
        return .init(label: [codec, depth, Self.rate(format.mSampleRate)].compactMap { $0 }.joined(separator: " · "), hz: format.mSampleRate)
    }

}

struct OutputDevice {
    let id: AudioDeviceID
    let name: String
    let uid: String
    let rate: Double
    let transport: UInt32
    private static func address(_ selector: AudioObjectPropertySelector) -> AudioObjectPropertyAddress {
        .init(mSelector: selector, mScope: kAudioObjectPropertyScopeGlobal, mElement: kAudioObjectPropertyElementMain)
    }
    private static func read<T>(_ id: AudioObjectID, _ s: AudioObjectPropertySelector, _ zero: T) -> T? {
        var a = address(s), result = zero, size = UInt32(MemoryLayout<T>.size)
        let status = withUnsafeMutableBytes(of: &result) { AudioObjectGetPropertyData(id,&a,0,nil,&size,$0.baseAddress!) }
        return status == noErr ? result : nil
    }
    static func current() -> Self? {
        guard let id = read(AudioObjectID(kAudioObjectSystemObject), kAudioHardwarePropertyDefaultOutputDevice, UInt32(0)), id != 0,
              let rate = read(id,kAudioDevicePropertyNominalSampleRate,0.0), rate > 0 else { return nil }
        let name = read(id,kAudioObjectPropertyName,Optional<Unmanaged<CFString>>.none) ?? nil
        let deviceUID = read(id,kAudioDevicePropertyDeviceUID,Optional<Unmanaged<CFString>>.none) ?? nil
        return Self(id: id, name: name?.takeRetainedValue() as String? ?? "Default output", uid: deviceUID?.takeRetainedValue() as String? ?? "", rate: rate,
                    transport: read(id,kAudioDevicePropertyTransportType,UInt32(0)) ?? 0)
    }
    var summary: String { "\(name) · \(AudioQuality.rate(rate))" }
    func supports(_ hz: Double) -> Bool {
        var a = Self.address(kAudioDevicePropertyAvailableNominalSampleRates), size: UInt32 = 0
        guard AudioObjectGetPropertyDataSize(id,&a,0,nil,&size) == noErr, size >= MemoryLayout<AudioValueRange>.size else { return false }
        var rates = [AudioValueRange](repeating: .init(), count: Int(size)/MemoryLayout<AudioValueRange>.stride)
        guard AudioObjectGetPropertyData(id,&a,0,nil,&size,&rates) == noErr else { return false }
        return rates.contains { $0.mMinimum <= hz && hz <= $0.mMaximum }
    }
    var canMatch: Bool {
        uid == "BuiltInHeadphoneOutputDevice" || [kAudioDeviceTransportTypeUSB,kAudioDeviceTransportTypeThunderbolt].contains(transport)
    }
    static var systemSounds: AudioDeviceID? {
        read(AudioObjectID(kAudioObjectSystemObject),kAudioHardwarePropertyDefaultSystemOutputDevice,UInt32(0))
    }
    static func restoreSystemSounds(_ id: AudioDeviceID) throws {
        guard systemSounds != id else { return }
        var a = address(kAudioHardwarePropertyDefaultSystemOutputDevice), target = id
        guard AudioObjectSetPropertyData(AudioObjectID(kAudioObjectSystemObject),&a,0,nil,4,&target) == noErr else { throw PlayerError("Could not restore the system sound output.") }
    }
    func switchAndVerify(_ hz: Double) throws {
        guard let soundDevice = Self.systemSounds, Self.current()?.uid == uid else { throw PlayerError("Output changed before the rate switch.") }
        try setRate(hz)
        // Run this transaction through reconciliation even if the caller selects another track.
        for _ in 0..<20 {
            Thread.sleep(forTimeInterval:0.05)
            guard let device = Self.current(), device.uid == uid else { throw PlayerError("Default audio output changed during the rate switch.") }
            if abs(device.rate-hz)<1 { break }
        }
        Thread.sleep(forTimeInterval:0.5)
        guard let device = Self.current(), device.uid == uid else { throw PlayerError("The selected output disconnected.") }
        try Self.restoreSystemSounds(soundDevice)
        guard Self.systemSounds == soundDevice, abs(device.rate-hz)<1 else { throw PlayerError("Output route or rate could not be confirmed.") }
    }
    func setRate(_ hz: Double) throws {
        var a = Self.address(kAudioDevicePropertyNominalSampleRate), settable = DarwinBoolean(false), target = hz
        guard canMatch,
              supports(hz), AudioObjectIsPropertySettable(id,&a,&settable) == noErr, settable.boolValue,
              Self.current()?.id == id else { throw PlayerError("\(name) cannot switch to \(AudioQuality.rate(hz)).") }
        guard AudioObjectSetPropertyData(id,&a,0,nil,UInt32(MemoryLayout<Double>.size),&target) == noErr else { throw PlayerError("Core Audio declined the rate change.") }
    }
}

extension AudioQuality {
    static func download(_ url: URL) -> Self? {
        // Read only the downloaded variant's manifest metadata. Other variants may also be advertised.
        guard let xml = try? XMLDocument(contentsOf: url.appendingPathComponent("boot.xml"), options: [.nodeLoadExternalEntitiesNever]),
              let streams = try? xml.nodes(forXPath: "//*[local-name()='Stream']"), streams.count == 1,
              let stream = (streams.first as? XMLElement)?.attribute(forName: "NetworkURL")?.stringValue.flatMap(URL.init(string:)),
              let masterNode = try? xml.nodes(forXPath: "//*[local-name()='MasterPlaylist']/*[local-name()='NetworkURL']").first,
              let masterURL = masterNode.stringValue.flatMap(URL.init(string:)),
              let files = try? FileManager.default.contentsOfDirectory(at: url.appendingPathComponent("Data"), includingPropertiesForKeys: nil),
              let master = files.first(where: { $0.lastPathComponent.hasSuffix("-master.m3u8") }),
              let text = try? String(contentsOf: master, encoding: .utf8) else { return nil }
        let lines = text.trimmingCharacters(in: .controlCharacters).components(separatedBy: .newlines)
        func attr(_ key: String, _ line: String) -> String? {
            let fields = line.drop(while: { $0 != ":" }).dropFirst().split(separator: ",")
            return fields.first { $0.hasPrefix(key + "=") }.map { String($0.dropFirst(key.count+1)).trimmingCharacters(in: CharacterSet(charactersIn: "\"")) }
        }
        let variants = lines.indices.filter { i in
            guard lines[i].hasPrefix("#EXT-X-STREAM-INF:"), i+1 < lines.count,
                  let location = URL(string: lines[i+1], relativeTo: masterURL)?.absoluteURL else { return false }
            return location == stream || (location.host == stream.host && location.path == stream.path)
        }
        guard variants.count == 1, let i = variants.first, let group = attr("AUDIO",lines[i]),
              let codec = attr("CODECS",lines[i]),
              let media = lines.first(where: { $0.hasPrefix("#EXT-X-MEDIA:") && attr("GROUP-ID",$0) == group }),
              let hz = attr("SAMPLE-RATE",media).flatMap(Double.init), hz > 0 else { return nil }
        let bits = attr("BIT-DEPTH",media).map { $0 + "-bit" }
        let name = codec == "alac" ? "Lossless ALAC" : codec
        return .init(label: [name,bits,Self.rate(hz)].compactMap { $0 }.joined(separator: " · "), hz: hz)
    }
}

final class RateMatcher: @unchecked Sendable {
    private let worker = DispatchQueue(label:"inc.zeroone.Pocketwheel.audio-rate")
    private var ownership: (uid: String, original: Double, last: Double)?
    func apply(_ device: OutputDevice, hz: Double) async throws {
        try await withCheckedThrowingContinuation { (reply: CheckedContinuation<Void, Error>) in
            worker.async { [self] in
                do {
                    let baseline = ownership.flatMap { $0.uid == device.uid && abs($0.last-device.rate)<1 ? $0.original : nil } ?? device.rate
                    try device.switchAndVerify(hz)
                    ownership = (device.uid,baseline,hz)
                    reply.resume()
                } catch { reply.resume(throwing:error) }
            }
        }
    }
    func restore(wait: Bool) {
        let action: @Sendable () -> Void = { [self] in
            defer { ownership = nil }
            if let old = ownership, let d = OutputDevice.current(), d.uid == old.uid,
               abs(d.rate-old.last)<1, abs(d.rate-old.original)>0.5 { try? d.switchAndVerify(old.original) }
        }
        if wait { worker.sync(execute:action) } else { worker.async(execute:action) }
    }
}
