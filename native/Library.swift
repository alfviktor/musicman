import AVFAudio
import Foundation
import iTunesLibrary

struct Track: Codable {
    let id: String
    let title: String
    let artist: String
    let album: String
    let albumArtist: String
    let disc: Int
    let number: Int
    let seconds: Double
    let file: URL?
    var package: URL? = nil
    var genre: String? = nil
    var year: Int? = nil

    static let extensions = Set(["mp3", "m4a", "aac", "wav", "aif", "aiff", "aifc", "caf", "flac", "alac"])

    static func loose(_ url: URL) -> Track {
        Track(id: url.standardizedFileURL.absoluteString, title: url.deletingPathExtension().lastPathComponent,
              artist: "", album: "", albumArtist: "", disc: 0, number: 0, seconds: 0, file: url)
    }
}

struct Mix: Codable {
    let name: String
    let ids: [String]
    var persistentKey: String? = nil
    var key: String { persistentKey ?? name }
}

struct Library: Codable {
    let tracks: [Track]
    let playlists: [Mix]

    static func read() throws -> Self {
        let library = try ITLibrary(apiVersion: "1.0")
        let tracks = library.allMediaItems.compactMap { item -> Track? in
            guard item.mediaKind == .kindSong else { return nil }
            let url = item.location
            let direct = !item.isDRMProtected && url?.isFileURL == true && Track.extensions.contains(url!.pathExtension.lowercased())
            return Track(id: String(format: "%016llX", item.persistentID.uint64Value), title: item.title,
                         artist: item.artist?.name ?? "", album: item.album.title ?? "",
                         albumArtist: item.album.albumArtist ?? item.artist?.name ?? "",
                         disc: item.album.discNumber, number: item.trackNumber,
                         seconds: Double(item.totalTime) / 1000, file: direct ? url : nil, package: url?.pathExtension == "movpkg" ? url : nil, genre: item.genre, year: item.year > 0 ? Int(item.year) : nil)
        }.sorted { $0.title.localizedCaseInsensitiveCompare($1.title) == .orderedAscending }
        let playlists = library.allPlaylists.filter { $0.isVisible && $0.distinguishedKind.rawValue == 0 && $0.kind != .folder }
            .map { Mix(name: $0.name, ids: $0.items.map { String(format: "%016llX", $0.persistentID.uint64Value) }, persistentKey: String(format:"%016llX",$0.persistentID.uint64Value)) }
            .sorted { $0.name.localizedCaseInsensitiveCompare($1.name) == .orderedAscending }
        return Self(tracks: tracks, playlists: playlists)
    }

    static var cache: URL {
        FileManager.default.urls(for: .cachesDirectory, in: .userDomainMask)[0]
            .appendingPathComponent("inc.zeroone.Pocketwheel/library-v4.json")
    }
    static func cached() throws -> Self { try JSONDecoder().decode(Self.self, from: Data(contentsOf: cache)) }
    func save() throws {
        try FileManager.default.createDirectory(at: Self.cache.deletingLastPathComponent(), withIntermediateDirectories: true)
        try JSONEncoder().encode(self).write(to: Self.cache, options: .atomic)
    }
}
