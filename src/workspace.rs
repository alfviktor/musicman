use crate::{audio, input, library::*};
use gpui::{prelude::*, *};
use rand::seq::SliceRandom;
use serde_json::{Value, json};
use std::collections::{BTreeSet, HashSet};

#[derive(Clone)]
pub enum Action {
    Play,
    Toggle,
    Next,
    Previous,
    Shuffle,
    Repeat,
    Enqueue(bool),
    Remove,
    Keep,
    Hide,
    Restore,
    Exclude,
    Pin,
    Open,
    Refresh,
    Match,
    Manage,
    Menu,
    Inspect,
    Back,
    WorkTracks,
    Equalizer,
    Preset(String),
    Band(usize, i8),
    NowPlaying,
}
pub enum Choice {
    Playlist(usize),
    Track(usize, bool),
}
pub struct Workspace {
    pub lib: Library,
    pub page: Page,
    pub rows: Vec<usize>,
    pub selection: BTreeSet<usize>,
    pub anchor: usize,
    pub search: Entity<input::TextInput>,
    pub query: String,
    pub focus: FocusHandle,
    pub scroll: UniformListScrollHandle,
    pub pending: Option<(Queue, usize)>,
    unavailable: HashSet<String>,
    pub current: Option<usize>,
    pub playing: bool,
    pub ended: bool,
    pub position: f64,
    pub duration: f64,
    pub output: String,
    pub source: String,
    pub active: String,
    pub matching: bool,
    pub message: String,
    pub loading: bool,
    pub manage: bool,
    pub menu: bool,
    pub inspector: bool,
    pub show_eq: bool,
    pub now_view: bool,
    pub eq: crate::eq::Presets,
    pub eq_route: String,
    pub seek_bounds: std::rc::Rc<std::cell::Cell<Bounds<Pixels>>>,
    pub deck: usize,
    pub choices: Vec<(String, Choice)>,
    pub choice_row: usize,
    pub choice_scroll: UniformListScrollHandle,
    pub back: Vec<Page>,
    requested: HashSet<String>,
    pub ready: HashSet<String>,
}
impl Workspace {
    pub fn new(cx: &mut Context<Self>, receiver: async_channel::Receiver<Value>) -> Self {
        let search = cx.new(input::new);
        cx.observe(&search, |this, e, cx| {
            let text = e.read(cx).content.to_string();
            if text != this.query {
                this.query = text;
                this.choices.clear();
                this.rebuild();
                cx.notify()
            }
        })
        .detach();
        cx.spawn(async move |this, cx| {
            while let Ok(v) = receiver.recv().await {
                if this.update(cx, |s, cx| s.event(v, cx)).is_err() {
                    break;
                }
            }
        })
        .detach();
        let presets = crate::eq::Presets::load();
        let preset_error = presets.as_ref().err().map(|e| e.to_string());
        cx.on_app_quit(|s, _| {
            s.checkpoint();
            async {}
        })
        .detach();
        cx.on_release(|s, _| s.checkpoint()).detach();
        let mut s = Self {
            lib: Library::default(),
            page: Page::Work,
            rows: vec![],
            selection: BTreeSet::new(),
            anchor: 0,
            search,
            query: String::new(),
            focus: cx.focus_handle(),
            scroll: UniformListScrollHandle::new(),
            pending: None,
            unavailable: HashSet::new(),
            current: None,
            playing: false,
            ended: false,
            position: 0.,
            duration: 0.,
            output: String::new(),
            source: String::new(),
            active: String::new(),
            matching: false,
            message: preset_error.unwrap_or_default(),
            loading: true,
            manage: false,
            menu: false,
            inspector: false,
            show_eq: false,
            now_view: false,
            eq: presets.unwrap_or_default(),
            eq_route: String::new(),
            seek_bounds: Default::default(),
            deck: 0,
            choices: vec![],
            choice_row: 0,
            choice_scroll: UniformListScrollHandle::new(),
            back: vec![],
            requested: HashSet::new(),
            ready: HashSet::new(),
        };
        audio::send("eq", json!(s.eq.gains()));
        s.load(cx);
        audio::send("info", Value::Null);
        s
    }
    fn load(&mut self, cx: &mut Context<Self>) {
        self.loading = true;
        if !cache().join("library-v4.json").exists() {
            audio::send("refresh", Value::Null);
            return;
        }
        let load = cx.background_executor().spawn(async { Library::load() });
        cx.spawn(async move |this, cx| {
            let result = load.await;
            let _ = this.update(cx, |s, cx| {
                s.loading = false;
                match result {
                    Ok(mut l) => {
                        let old = s
                            .current
                            .and_then(|i| s.lib.data.tracks.get(i))
                            .map(|t| t.id.clone());
                        for &i in &s.lib.files {
                            l.add_file(s.lib.data.tracks[i].clone());
                        }
                        s.current = old.and_then(|id| l.ids.get(&id).copied());
                        s.lib = l;
                        s.page = Page::Work;
                        s.back.clear();
                        s.rebuild();
                        s.request_covers();
                    }
                    Err(e) => s.message = format!("Library could not load: {e}"),
                };
                cx.notify()
            });
        })
        .detach()
    }
    fn event(&mut self, v: Value, cx: &mut Context<Self>) {
        let string = |key: &str| v[key].as_str().unwrap_or_default().to_owned();
        match v["kind"].as_str().unwrap_or("") {
            "library" => self.load(cx),
            "cover" => {
                self.ready.insert(string("id"));
            }
            "files" => {
                if let Ok(files) = serde_json::from_value::<Vec<Track>>(v["tracks"].clone()) {
                    for t in files {
                        self.lib.add_file(t);
                    }
                    let files: Vec<_> = self
                        .lib
                        .files
                        .iter()
                        .map(|&i| &self.lib.data.tracks[i])
                        .collect();
                    let saved = save("opened-files.json", &files);
                    self.lib.rebuild();
                    self.go(Page::Files, cx);
                    if let Err(e) = saved {
                        self.message = e.to_string();
                    }
                }
            }
            "unavailable" => {
                let id = string("id");
                let attempt = if let Some((q, i)) = &self.pending {
                    if q.ids.get(*i) != Some(&id) {
                        return;
                    }
                    self.pending.take()
                } else {
                    self.lib
                        .queue
                        .position
                        .filter(|&i| {
                            self.current.is_some() && self.lib.queue.ids.get(i) == Some(&id)
                        })
                        .map(|i| (self.lib.queue.clone(), i))
                };
                if let Some((mut q, i)) = attempt {
                    self.unavailable.insert(id);
                    self.current = None;
                    self.playing = false;
                    self.ended = true;
                    self.position = 0.;
                    self.duration = 0.;
                    self.source.clear();
                    self.active.clear();
                    // A failed repeat-one item falls through to the following recording.
                    q.position = None;
                    self.start(q, i);
                }
            }
            "error" => {
                if ["play", "toggle", "prepare"].contains(&v["action"].as_str().unwrap_or("")) {
                    self.pending = None
                }
                if v["action"] == "play" {
                    self.current = None;
                    self.playing = false;
                    self.source.clear();
                    self.active.clear();
                }
                if v["action"] == "refresh" {
                    self.loading = false
                }
                self.message = string("message");
            }
            "state" | "started" | "ended" => {
                let was_playing = self.playing;
                self.playing = v["playing"].as_bool().unwrap_or(false);
                self.matching = v["matching"].as_bool().unwrap_or(false);
                self.position = v["position"].as_f64().unwrap_or(0.);
                self.duration = v["duration"].as_f64().unwrap_or(0.);
                self.source = string("source");
                self.active = string("active");
                self.output = string("output");
                self.eq_route = string("eqRoute");
                if v["kind"] == "started" {
                    if let Some((mut q, i)) = self.pending.take() {
                        q.position = Some(i);
                        q.bookmark = None;
                        self.lib.queue = q;
                        self.persist_queue();
                    }
                    self.current = self.lib.ids.get(&string("id")).copied();
                    self.ended = false;
                    if self.page == Page::Queue {
                        self.rebuild()
                    }
                    self.request_covers();
                }
                if v["kind"] == "state" && was_playing && !self.playing {
                    self.checkpoint();
                }
                if v["kind"] == "ended" {
                    self.ended = true;
                    self.checkpoint();
                    self.advance(false)
                }
            }
            _ => (),
        }
        cx.notify()
    }
    pub fn rebuild(&mut self) {
        self.rows = self.lib.rows(
            &self.page,
            if self.page == Page::Queue {
                ""
            } else {
                &self.query
            },
        );
        self.selection.clear();
        self.anchor = 0;
        self.scroll.scroll_to_item(0, ScrollStrategy::Top);
    }
    pub fn selected(&self) -> Option<usize> {
        self.selection
            .first()
            .and_then(|r| self.rows.get(*r))
            .copied()
    }
    pub fn select(&mut self, row: usize, range: bool, extend: bool) {
        if row >= self.rows.len() {
            return;
        }
        if range {
            self.selection = (self.anchor.min(row)..=self.anchor.max(row)).collect()
        } else if extend {
            if !self.selection.remove(&row) {
                self.selection.insert(row);
            }
            self.anchor = row
        } else {
            self.selection = BTreeSet::from([row]);
            self.anchor = row
        }
        self.request_covers()
    }
    pub fn go(&mut self, page: Page, cx: &mut Context<Self>) {
        self.now_view = false;
        if self.page != page {
            self.back.push(self.page.clone());
            self.page = page
        }
        self.search.update(cx, |i, cx| {
            i.reset();
            cx.notify()
        });
        self.query.clear();
        self.message.clear();
        self.choices.clear();
        self.deck = 0;
        self.menu = false;
        self.rebuild();
        self.request_covers();
        cx.notify()
    }
    fn checkpoint(&mut self) {
        let Some(track) = self.current.and_then(|i| self.lib.data.tracks.get(i)) else {
            return;
        };
        if self.pending.is_some()
            || self
                .lib
                .queue
                .position
                .and_then(|i| self.lib.queue.ids.get(i))
                != Some(&track.id)
        {
            return;
        }
        let duration = if self.duration > 0. {
            self.duration
        } else {
            track.seconds
        };
        let point =
            (!self.ended && self.position > 0. && (duration <= 0. || self.position < duration))
                .then(|| Bookmark {
                    id: track.id.clone(),
                    seconds: self.position,
                    duration,
                });
        if self.lib.queue.bookmark != point {
            self.lib.queue.bookmark = point;
            self.persist_queue();
        }
    }
    fn persist_queue(&mut self) {
        if let Err(e) = save("queue.json", &self.lib.queue) {
            self.message = e.to_string()
        }
    }
    fn persist_curation(&mut self) {
        if let Err(e) = save("curation.json", &self.lib.c) {
            self.message = e.to_string()
        }
        self.lib.rebuild();
        self.rebuild()
    }
    pub fn request_covers(&mut self) {
        let cards = self.lib.album_cards(&self.page);
        let mut counts = std::collections::HashMap::new();
        let mut ids: Vec<String> = cards
            .iter()
            .enumerate()
            .filter(|(i, pick)| {
                if self.page == Page::Work {
                    let count = counts.entry(&pick.group).or_insert(0);
                    *count += 1;
                    *count <= 3
                } else {
                    (*i >= self.deck) && (*i < self.deck + 5)
                }
            })
            .map(|(_, pick)| pick)
            .filter_map(|p| self.lib.cover_id(&p.artist, &p.album).map(str::to_owned))
            .collect();
        if let Page::Album(a, b) = &self.page
            && let Some(id) = self.lib.cover_id(a, b)
        {
            ids.push(id.into())
        }
        let queued = self
            .lib
            .queue
            .position
            .and_then(|p| self.lib.queue.ids.get(p))
            .and_then(|id| self.lib.ids.get(id))
            .copied();
        for i in [self.current, self.selected(), queued]
            .into_iter()
            .flatten()
        {
            let t = &self.lib.data.tracks[i];
            ids.push(
                self.lib
                    .cover_id(&t.album_artist, &t.album)
                    .unwrap_or(&t.id)
                    .into(),
            )
        }
        let mut missing = vec![];
        for id in ids {
            if self.requested.insert(id.clone()) {
                if cache()
                    .join("covers-v1")
                    .join(format!("{id}.jpg"))
                    .is_file()
                {
                    self.ready.insert(id);
                } else {
                    missing.push(id)
                }
            }
        }
        if !missing.is_empty() {
            audio::send("covers", json!(missing))
        }
    }
    fn start(&mut self, q: Queue, i: usize) {
        self.start_at(q, i, 0.);
    }
    fn start_at(&mut self, q: Queue, requested: usize, offset: f64) {
        if self.pending.is_some() {
            return;
        }
        let Some(i) = q.playable_from(requested, |id| {
            self.lib.ids.contains_key(id)
                && !self.lib.c.hidden.contains(id)
                && !self.unavailable.contains(id)
        }) else {
            if self.ended {
                self.lib.queue = q;
                self.persist_queue();
                if self.page == Page::Queue {
                    self.rebuild();
                }
            }
            return;
        };
        let idx = self.lib.ids[&q.ids[i]];
        self.message.clear();
        let mut request = json!(self.lib.data.tracks[idx]);
        if i == requested && offset > 0. {
            request["startAt"] = json!(offset);
        }
        audio::send("play", request);
        self.pending = Some((q, i));
    }
    pub fn play_row(&mut self, r: usize) {
        if self.page == Page::Hidden || r >= self.rows.len() {
            return;
        }
        self.unavailable.clear();
        let q = if self.page == Page::Queue {
            self.lib.queue.clone()
        } else {
            Queue {
                ids: self
                    .rows
                    .iter()
                    .map(|&i| self.lib.data.tracks[i].id.clone())
                    .collect(),
                mode: self.lib.queue.mode.clone(),
                ..Default::default()
            }
        };
        self.start(q, r)
    }
    fn advance(&mut self, manual: bool) {
        let mut q = self.lib.queue.clone();
        if let Some(i) = q.next(manual) {
            self.start(q, i)
        }
    }
    fn add(&mut self, tracks: Vec<usize>, next: bool) {
        if self.pending.is_some() {
            return;
        }
        let ids: Vec<_> = tracks
            .iter()
            .map(|&i| &self.lib.data.tracks[i].id)
            .filter(|s| !self.lib.c.hidden.contains(*s))
            .cloned()
            .collect();
        if ids.is_empty() {
            return;
        }
        let n = ids.len();
        self.lib.queue.insert(ids, next);
        self.persist_queue();
        self.message = format!(
            "{} · {n} selected",
            if next { "Play next" } else { "Added to queue" }
        );
        if self.page == Page::Queue {
            self.rebuild()
        }
    }
    pub fn action(&mut self, a: Action, cx: &mut Context<Self>) {
        use self::Action::*;
        let seq: Vec<_> = self
            .selection
            .iter()
            .filter_map(|r| self.rows.get(*r).copied())
            .collect();
        if self.pending.is_some()
            && matches!(
                a,
                Play | Toggle
                    | Next
                    | Previous
                    | Shuffle
                    | Repeat
                    | Enqueue(_)
                    | Remove
                    | Keep
                    | Hide
                    | Restore
                    | Exclude
                    | Refresh
            )
        {
            return;
        }
        if !matches!(a, Menu) {
            self.menu = false
        }
        match a {
            Play => self.play_row(self.selection.first().copied().unwrap_or(0)),
            Toggle => {
                if self.current.is_some() && !self.ended {
                    audio::send("toggle", Value::Null)
                } else if !self.lib.queue.ids.is_empty() {
                    self.unavailable.clear();
                    let offset = self.lib.queue.resume_point().map_or(0., |b| b.seconds);
                    self.start_at(
                        self.lib.queue.clone(),
                        self.lib.queue.position.unwrap_or(0),
                        offset,
                    )
                } else if !seq.is_empty() {
                    self.action(Play, cx)
                }
            }
            Shuffle => {
                if !self.rows.is_empty() && self.page != Page::Hidden {
                    self.unavailable.clear();
                    let mut ids: Vec<_> = self
                        .rows
                        .iter()
                        .map(|&i| self.lib.data.tracks[i].id.clone())
                        .collect();
                    ids.shuffle(&mut rand::rng());
                    self.start(
                        Queue {
                            ids,
                            shuffled: true,
                            mode: "All".into(),
                            position: None,
                            bookmark: None,
                        },
                        0,
                    )
                }
            }
            Next => self.advance(true),
            Previous => {
                if let Some(i) = self.lib.queue.position {
                    self.start(self.lib.queue.clone(), i.saturating_sub(1))
                }
            }
            Repeat => {
                self.lib.queue.mode = match self.lib.queue.mode.as_str() {
                    "Off" | "" => "One song",
                    "One song" => "All",
                    _ => "Off",
                }
                .into();
                self.persist_queue()
            }
            Enqueue(next) => self.add(seq, next),
            Remove => {
                if self.page == Page::Queue {
                    for i in self.selection.iter().rev().copied() {
                        self.lib.queue.remove(i)
                    }
                    self.persist_queue();
                    self.rebuild()
                }
            }
            Keep | Hide | Restore | Exclude => {
                for i in seq {
                    let id = self.lib.data.tracks[i].id.clone();
                    let c = &mut self.lib.c;
                    match a {
                        Keep => {
                            c.excluded.remove(&id);
                            c.included.insert(id);
                        }
                        Hide => {
                            c.hidden.insert(id.clone());
                            for n in (0..self.lib.queue.ids.len()).rev() {
                                if self.lib.queue.ids[n] == id {
                                    self.lib.queue.remove(n)
                                }
                            }
                        }
                        Restore => {
                            c.hidden.remove(&id);
                        }
                        _ => {
                            c.included.remove(&id);
                            c.excluded.insert(id);
                        }
                    }
                }
                self.persist_curation();
                if matches!(a, Hide) {
                    self.persist_queue()
                }
            }
            Pin => {
                if let Page::Playlist(i) = self.page {
                    let key = self.lib.data.playlists[i].key().to_owned();
                    if !self.lib.c.playlists.remove(&key) {
                        self.lib.c.playlists.insert(key);
                    }
                    self.persist_curation()
                }
            }
            Open => audio::send("open", Value::Null),
            Refresh => {
                if !self.loading {
                    self.loading = true;
                    audio::send("refresh", Value::Null)
                }
            }
            Equalizer => self.show_eq = !self.show_eq,
            NowPlaying => {
                self.now_view = !self.now_view;
                self.request_covers();
            }
            Preset(name) => {
                self.eq.selected = name;
                self.apply_eq();
            }
            Band(band, delta) => {
                self.eq.edit(band, delta);
                self.apply_eq();
            }
            Match => audio::send("rate", json!(!self.matching)),
            Manage => {
                self.manage = !self.manage;
                self.go(if self.manage { Page::All } else { Page::Work }, cx)
            }
            Menu => self.menu = !self.menu,
            Inspect => {
                self.inspector = !self.inspector;
                self.request_covers()
            }
            Back => {
                if let Some(p) = self.back.pop() {
                    let hist = std::mem::take(&mut self.back);
                    self.go(p, cx);
                    self.back = hist
                }
            }
            WorkTracks => self.go(Page::WorkTracks, cx),
        }
        cx.notify()
    }
    pub fn choose(&mut self, i: usize, cx: &mut Context<Self>) {
        if i >= self.choices.len() {
            return;
        }
        let (_, choice) = self.choices.remove(i);
        self.choices.clear();
        match choice {
            Choice::Playlist(p) => {
                self.go(Page::Playlist(p), cx);
                self.action(Action::Shuffle, cx)
            }
            Choice::Track(t, n) => {
                self.go(self.page.clone(), cx);
                self.add(vec![t], n);
            }
        }
        cx.notify()
    }
    fn apply_eq(&mut self) {
        if let Err(e) = self.eq.save() {
            self.message = e.to_string();
        }
        audio::send("eq", json!(self.eq.gains()));
    }
    pub fn run(&mut self, cx: &mut Context<Self>) {
        let s = fold(self.query.trim());
        self.choice_row = 0;
        self.choice_scroll.scroll_to_item(0, ScrollStrategy::Top);
        self.choices.clear();
        if let Some(name) = s.strip_prefix("save eq ") {
            if self.eq.save_as(name.trim()) {
                self.go(self.page.clone(), cx);
                self.show_eq = true;
                self.apply_eq();
            } else {
                self.message =
                    "Choose a preset name of 1–32 characters. Off, Focus and Custom are reserved."
                        .into();
            }
            cx.notify();
            return;
        } else if let Some(name) = s.strip_prefix("eq ") {
            if self.eq.names().any(|n| n == name) {
                self.go(self.page.clone(), cx);
                self.show_eq = true;
                self.eq.selected = name.into();
                self.apply_eq();
            } else {
                self.message = "Unknown equalizer preset.".into();
            }
            cx.notify();
            return;
        }
        if let Some(name) = s
            .strip_suffix(" playlist on shuffle loop")
            .or_else(|| s.strip_suffix(" on shuffle loop"))
        {
            let name = name.trim_start_matches("play ").trim();
            let matches: Vec<_> = self
                .lib
                .data
                .playlists
                .iter()
                .enumerate()
                .filter(|(_, m)| fold(&m.name).contains(name))
                .collect();
            let exact = matches.iter().any(|(_, m)| fold(&m.name) == name);
            self.choices = matches
                .into_iter()
                .filter(|(_, m)| !exact || fold(&m.name) == name)
                .map(|(i, m)| (m.name.clone(), Choice::Playlist(i)))
                .collect();
        } else if let Some((term, next)) =
            s.strip_prefix("play next ").map(|x| (x, true)).or_else(|| {
                s.strip_prefix("add ")
                    .and_then(|x| x.strip_suffix(" to queue"))
                    .map(|x| (x, false))
            })
        {
            let (title, artist) = term.split_once(" by ").unwrap_or((term, ""));
            let hits = self.lib.rows(&Page::All, &format!("{title} {artist}"));
            let exact = hits
                .iter()
                .any(|&i| fold(&self.lib.data.tracks[i].title) == title);
            self.choices = hits
                .into_iter()
                .filter(|&i| !exact || fold(&self.lib.data.tracks[i].title) == title)
                .map(|i| {
                    let t = &self.lib.data.tracks[i];
                    (
                        format!("{} · {} · {}", t.title, t.artist, t.album),
                        Choice::Track(i, next),
                    )
                })
                .collect();
        } else if [
            "repeat this song",
            "repeat one",
            "loop queue",
            "repeat all",
            "repeat off",
        ]
        .contains(&s.as_str())
        {
            self.lib.queue.mode = match s.as_str() {
                "loop queue" | "repeat all" => "All",
                "repeat off" => "Off",
                _ => "One song",
            }
            .into();
            self.persist_queue();
            self.go(self.page.clone(), cx);
            return;
        } else if s == "show queue" {
            self.go(Page::Queue, cx);
            return;
        } else {
            return;
        }
        if self.choices.len() == 1 {
            self.choose(0, cx)
        } else {
            self.message = if self.choices.is_empty() {
                "No match in this library."
            } else {
                "Choose a match"
            }
            .into()
        }
        cx.notify()
    }
}
