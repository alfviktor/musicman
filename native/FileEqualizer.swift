import AVFAudio

struct EqualizerGains: Codable, Equatable {
    let low: Float
    let mid: Float
    let high: Float
    var flat: Bool { low == 0 && mid == 0 && high == 0 }

    func apply(to unit: AVAudioUnitEQ) {
        let bands: [(AVAudioUnitEQFilterType, Float, Float)] = [
            (.lowShelf, 140, low), (.parametric, 500, mid), (.highShelf, 1000, high)
        ]
        for (band, (type, frequency, gain)) in zip(unit.bands, bands) {
            band.filterType = type
            band.frequency = frequency
            band.bandwidth = 1
            band.gain = gain
            band.bypass = gain == 0
        }
        unit.globalGain = -(max(0, low) + max(0, mid) + max(0, high))
    }
}

@MainActor final class EqualizedFile {
    private let file: AVAudioFile
    private let graph = AVAudioEngine()
    private let node = AVAudioPlayerNode()
    let equalizer = AVAudioUnitEQ(numberOfBands: 3)
    private var offset: AVAudioFramePosition = 0
    private var serial = 0
    var onEnd: (() -> Void)?
    private var rate: Double { file.processingFormat.sampleRate }
    var duration: Double { Double(file.length) / rate }
    var position: Double {
        let time = node.lastRenderTime.flatMap { node.playerTime(forNodeTime: $0) }
        return min(duration, Double(offset) / rate + (time.map { Double($0.sampleTime) / $0.sampleRate } ?? 0))
    }
    init(url: URL, gains: EqualizerGains) throws {
        file = try AVAudioFile(forReading: url)
        guard file.length > 0 else { throw PlayerError("The audio file is empty.") }
        graph.attach(node)
        graph.attach(equalizer)
        graph.connect(node, to: equalizer, format: file.processingFormat)
        graph.connect(equalizer, to: graph.mainMixerNode, format: file.processingFormat)
        gains.apply(to: equalizer)
    }
    func seek(_ seconds: Double) {
        serial += 1
        let token = serial
        node.stop()
        offset = AVAudioFramePosition(max(0,min(duration,seconds)) * rate)
        offset = min(offset, file.length - 1)
        var frame = offset
        while frame < file.length {
            let count = AVAudioFrameCount(min(Int64(UInt32.max), file.length - frame))
            let last = frame + Int64(count) == file.length
            node.scheduleSegment(file, startingFrame: frame, frameCount: count, at: nil, completionCallbackType: .dataPlayedBack) { [weak self] _ in
                guard last else { return }
                Task { @MainActor in
                    guard let self, self.serial == token else { return }
                    self.onEnd?()
                }
            }
            frame += Int64(count)
        }
    }
    func resume() throws {
        if !graph.isRunning { try graph.start() }
        node.play()
    }
    func pause() { node.pause() }
    func stop() { serial += 1; node.stop(); graph.stop() }
}
