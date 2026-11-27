use rand::seq::SliceRandom;
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    fs, io,
    path::{Path, PathBuf},
};
use unicode_normalization::{UnicodeNormalization, char::is_combining_mark};

pub fn home() -> PathBuf {
    PathBuf::from(std::env::var_os("HOME").expect("macOS home directory"))
}
pub fn cache() -> PathBuf {
    home().join("Library/Caches/inc.zeroone.Pocketwheel")
}
pub fn settings() -> PathBuf {
    home().join("Library/Application Support/Pocketwheel")
}
pub fn fold(s: &str) -> String {
    s.nfd()
        .filter(|c| !is_combining_mark(*c))
        .flat_map(char::to_lowercase)
        .collect()
}
fn read<T: serde::de::DeserializeOwned>(path: &Path) -> io::Result<T> {
    serde_json::from_slice(&fs::read(path)?).map_err(io::Error::other)
}
pub fn save<T: Serialize>(name: &str, data: &T) -> io::Result<()> {
    let dir = settings();
    fs::create_dir_all(&dir)?;
    let tmp = dir.join(format!(".{name}.{}.tmp", std::process::id()));
    fs::write(&tmp, serde_json::to_vec(data)?)?;
    fs::rename(tmp, dir.join(name))
}
pub fn optional<T: serde::de::DeserializeOwned>(name: &str) -> io::Result<Option<T>> {
    match read(&settings().join(name)) {
        Ok(v) => Ok(Some(v)),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e),
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Track {
    pub id: String,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub album_artist: String,
    pub disc: i32,
    pub number: i32,
    pub seconds: f64,
    pub file: Option<String>,
    pub package: Option<String>,
    pub genre: Option<String>,
    pub year: Option<i32>,
}
impl Track {
    pub fn loose(url: &str) -> Self {
        let name = url::Url::parse(url)
            .ok()
            .and_then(|u| u.to_file_path().ok())
            .and_then(|p| {
                p.file_stem()
                    .map(|stem| stem.to_string_lossy().into_owned())
            })
            .unwrap_or_else(|| url.to_owned());
        Self {
            id: url.into(),
            title: name,
            artist: String::new(),
            album: String::new(),
            album_artist: String::new(),
            disc: 0,
            number: 0,
            seconds: 0.,
            file: Some(url.into()),
            package: None,
            genre: None,
            year: None,
        }
    }
}
#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Mix {
    pub name: String,
    pub ids: Vec<String>,
    pub persistent_key: Option<String>,
}
impl Mix {
    pub fn key(&self) -> &str {
        self.persistent_key.as_deref().unwrap_or(&self.name)
    }
}
#[derive(Deserialize, Default)]
pub struct Index {
    pub tracks: Vec<Track>,
    pub playlists: Vec<Mix>,
}
#[derive(Deserialize, Serialize, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct Curation {
    pub playlists: HashSet<String>,
    pub artists: HashSet<String>,
    pub soundtrack_genre: bool,
    pub included: HashSet<String>,
    pub excluded: HashSet<String>,
    pub hidden: HashSet<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Pick {
    pub group: String,
    pub artist: String,
    pub album: String,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Bookmark {
    pub id: String,
    pub seconds: f64,
    pub duration: f64,
}
#[derive(Default, Serialize, Deserialize, Clone)]
#[serde(default)]
pub struct Queue {
    pub ids: Vec<String>,
    pub position: Option<usize>,
    pub shuffled: bool,
    pub mode: String,
    pub bookmark: Option<Bookmark>,
}
impl Queue {
    pub fn resume_point(&self) -> Option<&Bookmark> {
        let id = self.ids.get(self.position?)?;
        self.bookmark.as_ref().filter(|b| {
            &b.id == id
                && b.seconds.is_finite()
                && b.seconds > 0.
                && b.duration.is_finite()
                && (b.duration == 0. || b.seconds < b.duration)
        })
    }
    /// Walk the current order once, including the prefix only when the whole queue loops.
    pub fn playable_from(
        &self,
        start: usize,
        mut playable: impl FnMut(&str) -> bool,
    ) -> Option<usize> {
        if start >= self.ids.len() {
            return None;
        }
        let wrap = if self.mode == "All" { start } else { 0 };
        (start..self.ids.len())
            .chain(0..wrap)
            .find(|&i| playable(&self.ids[i]))
    }
    pub fn next(&mut self, manual: bool) -> Option<usize> {
        if self.ids.is_empty() {
            return None;
        }
        let Some(i) = self.position else {
            return Some(0);
        };
        if self.mode == "One song" && !manual {
            return Some(i);
        }
        if i + 1 < self.ids.len() {
            return Some(i + 1);
        }
        if self.mode != "All" {
            return None;
        }
        if self.shuffled {
            let last = self.ids[i].clone();
            self.ids.shuffle(&mut rand::rng());
            if self.ids[0] == last
                && let Some(j) = self.ids.iter().position(|s| s != &last)
            {
                self.ids.swap(0, j)
            }
        }
        Some(0)
    }
    pub fn insert(&mut self, ids: Vec<String>, next: bool) {
        let i = if next {
            self.position.map_or(0, |i| i + 1)
        } else {
            self.ids.len()
        };
        self.ids.splice(i..i, ids);
    }
    pub fn remove(&mut self, i: usize) {
        if i < self.ids.len() && Some(i) != self.position {
            self.ids.remove(i);
            if let Some(p) = self.position.as_mut()
                && i < *p
            {
                *p -= 1
            }
        }
    }
}
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Page {
    Work,
    WorkTracks,
    Group(String),
    Album(String, String),
    Playlist(usize),
    All,
    Hidden,
    Queue,
    Files,
}
#[derive(Default)]
pub struct Library {
    pub data: Index,
    pub ids: HashMap<String, usize>,
    pub albums: HashMap<(String, String), Vec<usize>>,
    pub picks: Vec<Pick>,
    pub c: Curation,
    pub replay: HashMap<(String, String), Vec<String>>,
    pub queue: Queue,
    terms: Vec<String>,
    work: HashSet<usize>,
    pub files: Vec<usize>,
}
impl Library {
    pub fn load() -> io::Result<Self> {
        let data: Index = read(&cache().join("library-v4.json"))?;
        let mut s = Self {
            data,
            ..Self::default()
        };
        for (i, t) in s.data.tracks.iter().enumerate() {
            s.ids.insert(t.id.clone(), i);
            s.albums
                .entry((t.album_artist.clone(), t.album.clone()))
                .or_default()
                .push(i);
            s.terms
                .push(fold(&format!("{} {} {}", t.title, t.artist, t.album)));
        }
        for v in s.albums.values_mut() {
            v.sort_by_key(|&i| {
                let t = &s.data.tracks[i];
                (t.disc, t.number)
            })
        }
        for m in &s.data.playlists {
            if !fold(&m.name).contains("replay") {
                continue;
            }
            for id in &m.ids {
                if let Some(&i) = s.ids.get(id) {
                    let t = &s.data.tracks[i];
                    s.replay
                        .entry((t.album_artist.clone(), t.album.clone()))
                        .or_default()
                        .push(m.name.clone());
                }
            }
        }
        s.picks = optional("album-shelves.json")?.unwrap_or_else(|| {
            let mut scored: Vec<_> = s
                .replay
                .iter()
                .filter(|(a, _)| {
                    s.albums.get(*a).is_some_and(|v| {
                        v.iter().any(|&i| {
                            let genre = s.data.tracks[i]
                                .genre
                                .as_deref()
                                .unwrap_or("")
                                .to_lowercase();
                            ["ambient", "soundtrack", "electronic", "instrumental"]
                                .iter()
                                .any(|x| genre.contains(x))
                        })
                    })
                })
                .collect();
            scored.sort_by_key(|(k, v)| (std::cmp::Reverse(v.len()), (*k).clone()));
            scored
                .into_iter()
                .take(16)
                .map(|((artist, album), _)| Pick {
                    artist: artist.clone(),
                    album: album.clone(),
                    group: "Replay selections".into(),
                })
                .collect()
        });
        s.c = optional("curation.json")?.unwrap_or_else(|| {
            let mut c = Curation::default();
            for m in &s.data.playlists {
                let name = fold(&m.name);
                if ["work music", "soundtrack", "brown noise", "plastikman"]
                    .iter()
                    .any(|x| name.contains(x))
                {
                    c.playlists.insert(m.key().to_owned());
                }
            }
            c
        });
        for t in optional::<Vec<Track>>("opened-files.json")?.unwrap_or_default() {
            if t.id.starts_with("file://") {
                s.add_file(t);
            }
        }
        let kept_files: Vec<_> =
            s.c.included
                .iter()
                .filter(|id| id.starts_with("file://") && !s.ids.contains_key(*id))
                .cloned()
                .collect();
        for id in kept_files {
            s.add_file(Track::loose(&id));
        }
        s.queue = optional("queue.json")?.unwrap_or_default();
        let old = std::mem::take(&mut s.queue.ids);
        let pos = s.queue.position.take();
        for (i, id) in old.into_iter().enumerate() {
            if !s.ids.contains_key(&id) && id.starts_with("file://") {
                s.add_file(Track::loose(&id));
            }
            if s.ids.contains_key(&id) && !s.c.hidden.contains(&id) {
                if Some(i) == pos {
                    s.queue.position = Some(s.queue.ids.len())
                };
                s.queue.ids.push(id)
            }
        }
        s.rebuild();
        Ok(s)
    }
    pub fn add_file(&mut self, t: Track) -> usize {
        if let Some(&i) = self.ids.get(&t.id) {
            if !self.files.contains(&i) {
                self.files.push(i)
            };
            return i;
        }
        let i = self.data.tracks.len();
        self.terms.push(fold(&t.title));
        self.ids.insert(t.id.clone(), i);
        self.data.tracks.push(t);
        self.files.push(i);
        i
    }
    pub fn rebuild(&mut self) {
        self.work.clear();
        for p in &self.picks {
            if let Some(rows) = self.albums.get(&(p.artist.clone(), p.album.clone())) {
                self.work.extend(rows)
            }
        }
        for m in &self.data.playlists {
            if self.c.playlists.contains(m.key()) {
                self.work
                    .extend(m.ids.iter().filter_map(|id| self.ids.get(id)))
            }
        }
        for (i, t) in self.data.tracks.iter().enumerate() {
            if self.c.included.contains(&t.id)
                || self.c.artists.contains(&t.artist)
                || (self.c.soundtrack_genre
                    && t.genre
                        .as_ref()
                        .is_some_and(|g| g.to_lowercase().contains("soundtrack")))
            {
                self.work.insert(i);
            }
            if self.c.hidden.contains(&t.id) || self.c.excluded.contains(&t.id) {
                self.work.remove(&i);
            }
        }
    }
    pub fn rows(&self, p: &Page, q: &str) -> Vec<usize> {
        let mut rows: Vec<usize> = match p {
            Page::All | Page::Hidden | Page::Work | Page::WorkTracks => {
                (0..self.data.tracks.len()).collect()
            }
            Page::Playlist(i) => self
                .data
                .playlists
                .get(*i)
                .into_iter()
                .flat_map(|m| m.ids.iter().filter_map(|id| self.ids.get(id)).copied())
                .collect(),
            Page::Queue => self
                .queue
                .ids
                .iter()
                .filter_map(|id| self.ids.get(id))
                .copied()
                .collect(),
            Page::Files => self.files.clone(),
            Page::Album(a, b) => self
                .albums
                .get(&(a.clone(), b.clone()))
                .cloned()
                .unwrap_or_default(),
            Page::Group(g) => self
                .picks
                .iter()
                .filter(|p| &p.group == g)
                .filter_map(|p| self.albums.get(&(p.artist.clone(), p.album.clone())))
                .flatten()
                .copied()
                .collect(),
        };
        let query = fold(q);
        let words: Vec<_> = query.split_whitespace().collect();
        rows.retain(|&i| {
            let id = &self.data.tracks[i].id;
            (if *p == Page::Hidden {
                self.c.hidden.contains(id)
            } else if *p == Page::Queue {
                true
            } else if matches!(p, Page::Work | Page::WorkTracks | Page::Group(_)) {
                self.work.contains(&i)
            } else {
                !self.c.hidden.contains(id)
            }) && words.iter().all(|w| self.terms[i].contains(w))
        });
        rows
    }
    pub fn cover_id(&self, a: &str, b: &str) -> Option<&str> {
        self.albums
            .get(&(a.into(), b.into()))
            .and_then(|v| v.first())
            .map(|&i| self.data.tracks[i].id.as_str())
    }
    pub fn album_cards(&self, p: &Page) -> Vec<Pick> {
        self.picks
            .iter()
            .filter(|x| {
                (p == &Page::Work || matches!(p,Page::Group(g) if *g==x.group))
                    && self
                        .albums
                        .get(&(x.artist.clone(), x.album.clone()))
                        .is_some_and(|xs| xs.iter().any(|i| self.work.contains(i)))
            })
            .cloned()
            .collect()
    }
    pub fn name(&self, p: &Page) -> String {
        match p {
            Page::Work => "Work music".into(),
            Page::WorkTracks => "Work tracks".into(),
            Page::Group(g) => g.clone(),
            Page::Album(_, b) => b.clone(),
            Page::Playlist(i) => self.data.playlists[*i].name.clone(),
            Page::All => "All songs".into(),
            Page::Hidden => "Hidden songs".into(),
            Page::Queue => "Queue".into(),
            Page::Files => "Opened files".into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn queue() -> Queue {
        Queue {
            ids: ["a", "b", "c"].map(String::from).to_vec(),
            position: Some(1),
            ..Default::default()
        }
    }

    #[test]
    fn resume_belongs_to_the_current_recording_and_survives_reordering() {
        let mut q: Queue = serde_json::from_str(r#"{"ids":["a","b"],"position":1}"#).unwrap();
        assert!(q.resume_point().is_none());
        q.bookmark = Some(Bookmark {
            id: "b".into(),
            seconds: 72.0,
            duration: 180.0,
        });
        q.remove(0);
        let mut restored: Queue = serde_json::from_slice(&serde_json::to_vec(&q).unwrap()).unwrap();
        assert_eq!(restored.resume_point().unwrap().seconds, 72.);
        restored.ids[0] = "c".into();
        assert!(restored.resume_point().is_none());
        restored.ids[0] = "b".into();
        restored.bookmark.as_mut().unwrap().seconds = 180.;
        assert!(restored.resume_point().is_none());
    }
    #[test]
    fn unavailable_recordings_skip_duplicates_in_the_current_order() {
        let mut q = Queue {
            ids: ["gone", "gone", "live"].map(String::from).to_vec(),
            ..Default::default()
        };
        for mode in ["Off", "One song", "All"] {
            q.mode = mode.into();
            assert_eq!(q.playable_from(0, |id| id != "gone"), Some(2));
            assert_eq!(q.playable_from(0, |_| false), None);
        }
        q.ids.reverse();
        assert_eq!(q.playable_from(1, |id| id == "live"), Some(0));
        q.mode = "Off".into();
        assert_eq!(q.playable_from(1, |id| id == "live"), None);
    }
    #[test]
    fn repeat_one_applies_to_natural_end_and_all_wraps() {
        let mut q = queue();
        q.mode = "One song".into();
        assert_eq!(q.next(false), Some(1));
        assert_eq!(q.next(true), Some(2));
        q.position = Some(2);
        assert_eq!(q.next(true), None);
        q.mode = "All".into();
        assert_eq!(q.next(false), Some(0));
        q.shuffled = true;
        assert_eq!(q.next(false), Some(0));
        assert_ne!(q.ids[0], "c");
    }

    #[test]
    fn play_next_preserves_selection_and_duplicate_positions() {
        let mut q = queue();
        q.insert(vec!["c".into(), "a".into()], true);
        assert_eq!(q.ids, ["a", "b", "c", "a", "c"]);
        q.remove(1);
        q.remove(0);
        assert_eq!(q.position, Some(0));
        assert_eq!(q.ids, ["b", "c", "a", "c"]);
    }

    #[test]
    fn hidden_playing_row_preserves_queue_indices() {
        let mut lib = Library::default();
        for url in ["file:///a.flac", "file:///b.flac"] {
            lib.add_file(Track::loose(url));
            lib.queue.ids.push(url.to_owned());
        }
        lib.c.hidden.insert(lib.queue.ids[0].clone());
        assert_eq!(lib.rows(&Page::Queue, ""), [0, 1]);
        assert_eq!(lib.rows(&Page::All, ""), [1]);
    }
}
