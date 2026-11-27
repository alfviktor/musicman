# Musicman

A keyboard-driven macOS music player built with Rust and GPUI, the UI framework behind Zed. The home screen arranges your work music into shelves with artwork from your library. A narrow Swift adapter connects to MusicKit, iTunesLibrary, AVFAudio and Core Audio in the same process.

## Build

Use macOS 14 or later with Rust, Python 3, Xcode and its Metal compiler installed. If Xcode reports a missing Metal toolchain, run `xcodebuild -downloadComponent MetalToolchain`.

```sh
./build.sh
open build/rust/Musicman.app
```

The build uses ad hoc signing by default. For personal library access, supply a local Apple Development identity through `SIGN_IDENTITY`. Allow media-library access when macOS asks. Apple Music subscription tracks also require a signed-in subscription with access to the recording. Compatibility has been exercised on an Apple silicon Mac.

## Playback

Select a collection or album to see its tracks. Double-click a track to play it. Select one or more tracks for **Play next** or **Add to queue**. The repeat control cycles through off, one song and the entire queue. **Shuffle + loop** shuffles the visible tracks and repeats the queue. **Manage library** also exposes hidden tracks and playlists. Curation edits belong to Musicman and can be reversed.

Use ⌘K to search or enter a command. Examples include `night drive playlist on shuffle loop`, `play next Time by Hans Zimmer`, `repeat one`, and `show queue`. Commands are matched locally. Use ⌘O to open audio files and ⌘I to inspect the signal.

Missing files and unavailable recordings are skipped automatically. A looping queue stops quietly when every remaining recording has been tried. Selecting a recording again retries it. Closing the last window or choosing Quit Musicman stops the player.

## Now Playing and equalizer

Click the cover, song title or **Now playing** in the sidebar to see the record sleeve, signal details and upcoming songs. ⌘J toggles this view. Click the timeline to seek. The title bar links back to the collection and album.

**EQ** (⌘E) has three bands and starts off. **Focus** uses a −3 dB high shelf centered at 1 kHz. Editing a band saves a Custom preset. Use ⌘K and enter `save eq late night` to keep a named preset or `eq late night` to recall it. Presets and the last selection persist across launches. Positive gains reserve headroom automatically. The EQ processes local audio files. The MusicKit subscription player offers no public insert for this filter and its stream bypass is shown in the panel.

## Audio quality

Local files play through AVFAudio. Musicman reads the source codec, bit depth when available, and sample rate from the file. Subscription tracks play through MusicKit’s application player with Music.app closed. **Signal** shows the downloaded rendition when identifiable, the active variant when MusicKit reports one, and the hardware output rate. MusicKit can omit the active variant. In that case the stream’s current fidelity is unknown. A download badge describes the stored copy.

Rate matching is opt-in per launch and applies on the next track with a known rate. It can change a supported wired output’s nominal rate, which is shared by other apps. The hardware may mute briefly. Core Audio and the output device determine the final signal path.

## Library data

Musicman reads the local Music library and its Replay playlists. It caches artwork and its searchable index in the user's Library/Caches directory. Curated shelves, hidden items and queue state live in Application Support. Existing installations retain the original Pocketwheel storage paths and bundle identifier for continuity. Personal library records and artwork stay outside the source tree.

## Development

`src/` contains the GPUI interface, command handling, curation and queue. `native/` contains the Apple framework adapter. Run `cargo fmt --check` and `cargo test --release --locked` to check the Rust code. The bundled executable accepts `--check --refresh` to re-index the library and check artwork without playback. Add `--prepare "Song title"` to prepare a matching subscription track silently.

Musicman is licensed under Apache-2.0. See LICENSE-APACHE and NOTICE. Its native interface borrows the brushed keys and inset displays of vintage stereo components and the compact playlist layout of classic desktop players.
