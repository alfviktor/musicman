mod audio;
mod eq;
mod input;
mod library;
mod theme;
mod view;
mod workspace;
use gpui::*;
use serde_json::{Value, json};
use std::time::{Duration, Instant};

fn check(rx: async_channel::Receiver<Value>) -> Result<(), String> {
    fn wait(rx: &async_channel::Receiver<Value>, kind: &str) -> Result<Value, String> {
        let start = Instant::now();
        while start.elapsed() < Duration::from_secs(50) {
            audio::pump();
            while let Ok(v) = rx.try_recv() {
                if v["kind"] == "error" {
                    return Err(v["message"].to_string());
                }
                if v["kind"] == kind {
                    return Ok(v);
                }
            }
        }
        Err(format!("Timed out waiting for {kind}"))
    }
    if std::env::args().any(|s| s == "--refresh")
        || !library::cache().join("library-v4.json").exists()
    {
        audio::send("refresh", Value::Null);
        wait(&rx, "library")?;
    }
    let s = library::Library::load().map_err(|e| e.to_string())?;
    println!(
        "Library: {} songs. Work: {}. Curated releases: {}. Replay playlists: {}.",
        s.data.tracks.len(),
        s.rows(&library::Page::Work, "").len(),
        s.picks.len(),
        s.data
            .playlists
            .iter()
            .filter(|m| m.name.starts_with("Replay"))
            .count()
    );
    let ids: Vec<_> = s
        .picks
        .iter()
        .filter_map(|p| s.cover_id(&p.artist, &p.album))
        .collect();
    audio::send("covers", json!(ids));
    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(15)
        && ids.iter().any(|id| {
            !library::cache()
                .join("covers-v1")
                .join(format!("{id}.jpg"))
                .exists()
        })
    {
        audio::pump();
    }
    println!(
        "Artwork cached: {}/{}",
        ids.iter()
            .filter(|id| library::cache()
                .join("covers-v1")
                .join(format!("{id}.jpg"))
                .exists())
            .count(),
        ids.len()
    );
    if let Some(term) = std::env::args().skip_while(|s| s != "--prepare").nth(1) {
        let t = s
            .data
            .tracks
            .iter()
            .find(|t| t.title.eq_ignore_ascii_case(&term) && t.file.is_none())
            .ok_or("No matching Apple Music song")?;
        audio::send("prepare", json!(t));
        let v = wait(&rx, "prepared")?;
        println!("Prepared (silent): {} · {}", t.title, v["source"]);
    }
    for path in std::env::args().skip_while(|s| s != "--files").skip(1) {
        let absolute = std::fs::canonicalize(&path).map_err(|e| e.to_string())?;
        let url = url::Url::from_file_path(absolute).map_err(|_| "Invalid file path")?;
        audio::send("prepare", json!(library::Track::loose(url.as_str())));
        let event = wait(&rx, "prepared")?;
        println!("Prepared file (silent): {path} · {}", event["source"]);
    }
    Ok(())
}
fn main() {
    let rx = audio::connect();
    if std::env::args().any(|s| s == "--check") {
        let result = check(rx);
        audio::shutdown();
        if let Err(e) = result {
            eprintln!("{e}");
            std::process::exit(1)
        }
        return;
    }
    Application::new().run(move |cx| {
        input::bind(cx);
        cx.on_window_closed(|cx| {
            if cx.windows().is_empty() {
                cx.quit()
            }
        })
        .detach();
        cx.on_app_quit(|_| {
            audio::shutdown();
            async {}
        })
        .detach();
        let mut bounds = Bounds::centered(None, size(px(1040.), px(780.)), cx);
        // AppKit rounds fractional screen origins outwards when it creates a window.
        bounds.origin.x = bounds.origin.x.floor();
        bounds.origin.y = bounds.origin.y.floor();
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                window_min_size: Some(size(px(785.), px(580.))),
                titlebar: Some(TitlebarOptions {
                    title: Some("Musicman".into()),
                    appears_transparent: true,
                    traffic_light_position: Some(point(px(14.), px(12.))),
                }),
                ..Default::default()
            },
            |window, cx| {
                cx.new(|cx| {
                    let s = workspace::Workspace::new(cx, rx);
                    window.focus(&s.focus);
                    s
                })
            },
        )
        .expect("Open player window");
    })
}
