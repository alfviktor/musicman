import MusicKit

enum SongResolver {
    static func resolve(_ track: Track) async throws -> Song {
        var request = MusicLibraryRequest<Song>()
        request.filter(matching: \.title, equalTo: track.title)
        request.limit = 100
        var batch = try await request.response().items
        var match: Song?
        while true {
            for song in batch where song.title == track.title && (track.artist.isEmpty || song.artistName == track.artist)
                && (track.album.isEmpty || song.albumTitle == track.album)
                && (song.trackNumber == nil || track.number == 0 || song.trackNumber == track.number)
                && (song.discNumber == nil || track.disc == 0 || song.discNumber == track.disc)
                && (song.duration == nil || track.seconds == 0 || abs(song.duration! - track.seconds) < 2) {
                if let match, match.id != song.id {
                    throw PlayerError("Multiple recordings match. Choose a distinct recording from the library.")
                }
                match = song
            }
            guard batch.hasNextBatch, let more = try await batch.nextBatch(limit: 100) else { break }
            batch = more
        }
        guard let song = match, song.playParameters != nil else {
            throw PlayerError("MusicKit could not identify a playable copy. Check Apple Music access and subscription status.")
        }
        return song
    }
}
