mod rack;
use crate::theme::*;
use crate::{
    library::{Page, Pick, cache},
    workspace::{Action, Workspace},
};
use gpui::{prelude::*, *};
fn line() -> Div {
    div().h(px(1.)).w_full().bg(rgb(EDGE)).flex_shrink_0()
}
fn caption(s: impl Into<SharedString>) -> Div {
    div()
        .text_size(px(11.))
        .font_family(MONO)
        .text_color(rgb(DIM))
        .child(s.into())
}
fn label(s: impl Into<SharedString>) -> Div {
    div().truncate().child(s.into())
}
fn clock(t: f64) -> String {
    let n = t.max(0.) as usize;
    format!("{}:{:02}", n / 60, n % 60)
}
impl Workspace {
    fn button(
        &self,
        id: &'static str,
        text: impl Into<SharedString>,
        act: Action,
        accent: bool,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        div()
            .id(id)
            .flex_none()
            .h(px(27.))
            .px(px(9.))
            .flex()
            .items_center()
            .justify_center()
            .text_size(px(12.))
            .font_weight(FontWeight::SEMIBOLD)
            .border_1()
            .border_color(rgb(RAIL))
            .text_color(rgb(BG))
            .bg(keycap(accent))
            .cursor_pointer()
            .hover(move |s| s.bg(rgb(if accent { ACCENT } else { KEY_HIGHLIGHT })))
            .active(|s| s.bg(rgb(KEY_SHADOW)))
            .child(text.into())
            .on_click(cx.listener(move |s, _, _, cx| s.action(act.clone(), cx)))
    }
    fn nav(
        &self,
        key: usize,
        icon: &str,
        title: String,
        p: Page,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let active = p == self.page && !self.now_view;
        div()
            .id(("nav", key))
            .flex()
            .items_center()
            .gap(px(9.))
            .h(px(29.))
            .px(px(11.))
            .bg(rgb(if active { SELECTED } else { SIDE }))
            .text_color(rgb(if active { TEXT } else { TEXT }))
            .cursor_pointer()
            .hover(|s| s.bg(rgb(HOVER)))
            .child(
                div()
                    .w(px(16.))
                    .text_color(rgb(if active { ACCENT } else { DIM }))
                    .child(icon.to_owned()),
            )
            .child(label(title))
            .on_click(cx.listener(move |s, _, w, cx| {
                s.go(p.clone(), cx);
                w.focus(&s.focus)
            }))
    }
    fn side(&self, cx: &mut Context<Self>) -> Div {
        let mut col = div()
            .w(px(206.))
            .flex_none()
            .bg(rgb(SIDE))
            .border_r_1()
            .border_color(rgb(EDGE))
            .flex()
            .flex_col();
        let mut links = div()
            .id("navigation")
            .overflow_y_scroll()
            .flex_1()
            .min_h_0()
            .p(px(10.))
            .flex()
            .flex_col()
            .gap(px(2.))
            .child(rack::faceplate("LIBRARY"))
            .child(self.nav(0, "▦", "Work music".into(), Page::Work, cx))
            .child(self.nav(
                1,
                "≡",
                format!("Queue   {}", self.lib.queue.ids.len()),
                Page::Queue,
                cx,
            ));
        if self.display_track().is_some() {
            links = links.child(
                div()
                    .id("now-nav")
                    .h(px(29.))
                    .px(px(11.))
                    .flex()
                    .items_center()
                    .gap(px(9.))
                    .bg(rgb(if self.now_view { SELECTED } else { SIDE }))
                    .cursor_pointer()
                    .hover(|s| s.bg(rgb(HOVER)))
                    .child(div().w(px(16.)).text_color(rgb(ACCENT)).child("▷"))
                    .child("Now playing")
                    .on_click(cx.listener(|s, _, w, cx| {
                        s.now_view = true;
                        s.request_covers();
                        w.focus(&s.focus);
                        cx.notify();
                    })),
            );
        }
        if self.manage {
            links = links
                .child(self.nav(2, "♫", "All songs".into(), Page::All, cx))
                .child(self.nav(3, "−", "Hidden songs".into(), Page::Hidden, cx));
        }
        if self.manage || !self.lib.files.is_empty() {
            links = links.child(self.nav(4, "↗", "Opened files".into(), Page::Files, cx))
        }
        let mut groups = vec![];
        for p in &self.lib.picks {
            if !groups.contains(&&p.group) {
                groups.push(&p.group)
            }
        }
        if !groups.is_empty() {
            links = links.child(div().pt(px(15.)).child(rack::faceplate("COLLECTIONS")))
        }
        for (i, g) in groups.into_iter().enumerate() {
            links = links.child(self.nav(i + 10, "◫", g.clone(), Page::Group(g.clone()), cx))
        }
        links = links.child(div().pt(px(15.)).child(rack::faceplate("PLAYLISTS")));
        for (i, m) in self
            .lib
            .data
            .playlists
            .iter()
            .enumerate()
            .filter(|(_, m)| self.manage || self.lib.c.playlists.contains(m.key()))
        {
            links = links.child(self.nav(i + 100, "≋", m.name.clone(), Page::Playlist(i), cx));
        }
        col = col
            .child(links)
            .child(line())
            .child(div().p(px(10.)).child(self.button(
                "manage",
                if self.manage {
                    "✓  Manage library"
                } else {
                    "Manage library"
                },
                Action::Manage,
                false,
                cx,
            )));
        col
    }
    fn art(&self, id: Option<&str>, size: f32) -> Div {
        let box_ = div()
            .size(px(size))
            .flex_shrink_0()
            .overflow_hidden()
            .bg(rgb(PANEL))
            .flex()
            .justify_center()
            .items_center();
        if let Some(id) = id.filter(|s| self.ready.contains(*s)) {
            box_.child(
                img(cache().join("covers-v1").join(format!("{id}.jpg")))
                    .size_full()
                    .object_fit(ObjectFit::Cover),
            )
        } else {
            box_.child(
                div()
                    .text_color(rgb(DIM))
                    .text_size(px(size * 0.3))
                    .child("♫"),
            )
        }
    }
    fn home_shelves(&self, cards: &[Pick], width: f32, cx: &mut Context<Self>) -> Stateful<Div> {
        let mut groups: Vec<(&str, Vec<&Pick>)> = Vec::new();
        for card in cards {
            if let Some((_, albums)) = groups.iter_mut().find(|(name, _)| *name == card.group) {
                albums.push(card);
            } else {
                groups.push((&card.group, vec![card]));
            }
        }
        let columns = if width >= 940. { 2 } else { 1 };
        let mut shelves = div()
            .id("home-shelves")
            .overflow_y_scroll()
            .flex_1()
            .min_h_0()
            .px(px(20.))
            .pb(px(18.))
            .flex()
            .flex_col()
            .gap(px(15.));
        for (row, group_row) in groups.chunks(columns).enumerate() {
            let mut band = div().w_full().flex().gap(px(14.)).flex_none();
            for (col, (name, albums)) in group_row.iter().enumerate() {
                let key = row * columns + col;
                let page = Page::Group((*name).into());
                let mut shelf = div()
                    .flex_1()
                    .min_w_0()
                    .bg(rgb(SIDE))
                    .border_1()
                    .border_color(rgb(EDGE))
                    .child(
                        div()
                            .id(("collection", key))
                            .bg(metal())
                            .px(px(12.))
                            .h(px(39.))
                            .flex()
                            .items_center()
                            .justify_between()
                            .cursor_pointer()
                            .hover(|s| s.bg(rgb(HOVER)))
                            .child(
                                div()
                                    .text_color(rgb(TEXT))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child(name.to_uppercase()),
                            )
                            .child(rack::stripe())
                            .gap(px(9.))
                            .child(caption(format!("{}  ›", albums.len())))
                            .on_click(cx.listener(move |s, _, _, cx| s.go(page.clone(), cx))),
                    );
                for (i, p) in albums.iter().take(3).enumerate() {
                    let page = Page::Album(p.artist.clone(), p.album.clone());
                    let id = self.lib.cover_id(&p.artist, &p.album);
                    shelf = shelf.child(
                        div()
                            .id(("shelf-album", key * 3 + i))
                            .bg(rgb(GLASS))
                            .text_color(rgb(PHOSPHOR))
                            .h(px(62.))
                            .px(px(11.))
                            .flex()
                            .items_center()
                            .gap(px(12.))
                            .cursor_pointer()
                            .border_t_1()
                            .border_color(rgb(PANEL))
                            .hover(|s| s.bg(rgb(HOVER)))
                            .child(self.art(id, 46.))
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .flex()
                                    .flex_col()
                                    .gap(px(4.))
                                    .child(label(short_title(&p.album)))
                                    .child(caption(p.artist.clone()).truncate()),
                            )
                            .on_click(cx.listener(move |s, _, _, cx| s.go(page.clone(), cx))),
                    );
                }
                band = band.child(shelf);
            }
            shelves = shelves.child(band);
        }
        shelves
    }
    fn deck(&self, cards: Vec<Pick>, width: f32, cx: &mut Context<Self>) -> Div {
        let visible = if width > 1120. {
            5
        } else if width > 930. {
            4
        } else {
            3
        };
        let art_size = ((width - 206. - 50.) / visible as f32 - 16.).clamp(80., 165.);
        let start = self.deck.min(cards.len().saturating_sub(1));
        let max = cards.len();
        let mut strip = div().flex().gap(px(14.)).overflow_hidden();
        for (i, p) in cards.into_iter().skip(start).take(visible).enumerate() {
            let id = self.lib.cover_id(&p.artist, &p.album);
            let page = Page::Album(p.artist.clone(), p.album.clone());
            strip = strip.child(
                div()
                    .id(("album", i))
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .gap(px(4.))
                    .cursor_pointer()
                    .child(self.art(id, art_size))
                    .child(
                        div()
                            .pt(px(3.))
                            .text_size(px(13.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .truncate()
                            .child(short_title(&p.album)),
                    )
                    .child(
                        div()
                            .text_size(px(12.))
                            .text_color(rgb(DIM))
                            .truncate()
                            .child(p.artist),
                    )
                    .on_click(cx.listener(move |s, _, _, cx| s.go(page.clone(), cx))),
            );
        }
        let mut header = div()
            .flex()
            .items_center()
            .justify_between()
            .mb(px(10.))
            .child(caption(format!("ALBUMS  /  {max}")));
        if max > visible {
            header = header.child(div().flex().gap(px(5.)).children(
                [(-1isize, "‹"), (1, "›")].into_iter().map(|(dir, t)| {
                    div()
                        .id(if dir < 0 { "deck-prev" } else { "deck-next" })
                        .px(px(9.))
                        .cursor_pointer()
                        .bg(rgb(PANEL))
                        .hover(|s| s.bg(rgb(EDGE)))
                        .child(t)
                        .on_click(cx.listener(move |s, _, _, cx| {
                            s.deck = if dir < 0 {
                                s.deck.saturating_sub(visible)
                            } else {
                                (s.deck + visible).min(max.saturating_sub(visible))
                            };
                            s.request_covers();
                            cx.notify()
                        }))
                }),
            ))
        }
        div().px(px(21.)).pb(px(18.)).child(header).child(strip)
    }
    fn row(&self, r: usize, cx: &mut Context<Self>) -> Stateful<Div> {
        let i = self.rows[r];
        let t = &self.lib.data.tracks[i];
        let sel = self.selection.contains(&r);
        let now = self.current == Some(i)
            && (self.page != Page::Queue || self.lib.queue.position == Some(r));
        let marker = if now {
            "▶".into()
        } else {
            format!("{:02}", r + 1)
        };
        div()
            .id(("song", r))
            .h(px(31.))
            .w_full()
            .px(px(16.))
            .flex()
            .items_center()
            .gap(px(9.))
            .border_b_1()
            .border_color(rgb(PANEL))
            .bg(rgb(if sel { SELECTED } else { GLASS }))
            .hover(move |s| s.bg(rgb(if sel { SELECTED } else { STATUS })))
            .cursor_pointer()
            .child(div().w(px(36.)).flex_none().child(caption(marker)))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .text_color(rgb(if now { ACCENT } else { PHOSPHOR }))
                    .child(t.title.clone()),
            )
            .child(
                div()
                    .w(relative(0.24))
                    .truncate()
                    .text_color(rgb(TEXT))
                    .child(t.artist.clone()),
            )
            .child(
                div()
                    .w(relative(0.21))
                    .truncate()
                    .text_color(rgb(DIM))
                    .child(short_title(&t.album)),
            )
            .child(
                div()
                    .w(px(42.))
                    .text_right()
                    .child(caption(clock(t.seconds))),
            )
            .on_click(cx.listener(move |s, e: &ClickEvent, w, cx| {
                s.select(r, e.modifiers().shift, e.modifiers().platform);
                w.focus(&s.focus);
                if e.click_count() == 2 {
                    s.play_row(r)
                }
                cx.notify()
            }))
    }
    fn edit_menu(&self, cx: &mut Context<Self>) -> Div {
        let mut bar = div()
            .flex()
            .flex_wrap()
            .gap(px(6.))
            .p(px(8.))
            .bg(rgb(PANEL));
        for (id, text, a) in [
            ("keep", "Keep in Work", Action::Keep),
            ("remove-work", "Remove from Work", Action::Exclude),
            ("hide", "Hide songs", Action::Hide),
            ("restore", "Restore", Action::Restore),
            ("open", "Open files…", Action::Open),
            ("refresh", "Refresh library", Action::Refresh),
        ] {
            if id == "restore" && self.page != Page::Hidden {
                continue;
            }
            bar = bar.child(self.button(id, text, a, false, cx))
        }
        if matches!(self.page, Page::Playlist(_)) {
            bar = bar.child(self.button("pin", "Pin / unpin playlist", Action::Pin, false, cx))
        }
        bar
    }
    fn display_track(&self) -> Option<&crate::library::Track> {
        let resume = self
            .lib
            .queue
            .position
            .and_then(|p| self.lib.queue.ids.get(p))
            .and_then(|id| self.lib.ids.get(id))
            .copied();
        self.current
            .or(resume)
            .and_then(|i| self.lib.data.tracks.get(i))
    }

    fn elapsed(&self) -> f64 {
        if self.current.is_none() {
            self.lib.queue.resume_point().map_or(0., |b| b.seconds)
        } else {
            self.position
        }
    }
    fn track_duration(&self) -> f64 {
        if self.current.is_none()
            && let Some(b) = self.lib.queue.resume_point()
            && b.duration > 0.
        {
            return b.duration;
        }
        if self.duration > 0. {
            self.duration
        } else {
            self.display_track().map_or(0., |t| t.seconds)
        }
    }

    fn timeline(&self, cx: &mut Context<Self>) -> Stateful<Div> {
        let duration = self.track_duration();
        let fraction = if duration > 0. {
            (self.elapsed() / duration).clamp(0., 1.)
        } else {
            0.
        };
        let bounds = self.seek_bounds.clone();
        let groove = div()
            .relative()
            .w_full()
            .h(px(if self.now_view { 7. } else { 4. }))
            .bg(rgb(if self.now_view { UNLIT } else { EDGE }))
            .child(div().w(relative(fraction as f32)).h_full().bg(rgb(ACCENT)));
        let groove = if self.now_view {
            groove.child(
                div().absolute().inset_0().mx(px(6.)).child(
                    div()
                        .absolute()
                        .left(relative(fraction as f32))
                        .top(px(-3.))
                        .ml(px(-6.))
                        .w(px(12.))
                        .h(px(13.))
                        .border_1()
                        .border_color(rgb(EDGE))
                        .bg(linear_gradient(
                            180.,
                            linear_color_stop(rgb(KEY_HIGHLIGHT), 0.),
                            linear_color_stop(rgb(KEY_SHADOW), 1.),
                        )),
                ),
            )
        } else {
            groove
        };
        div()
            .id("timeline")
            .w_full()
            .h(px(16.))
            .relative()
            .flex()
            .items_center()
            .cursor_pointer()
            .child(groove)
            .child(
                canvas(move |b, _, _| bounds.set(b), |_, _, _, _| {})
                    .absolute()
                    .inset_0(),
            )
            .on_click(cx.listener(move |s, e: &ClickEvent, _, cx| {
                let rect = s.seek_bounds.get();
                if s.current.is_some() && !s.ended && duration > 0. && rect.size.width > px(0.) {
                    let frac = ((e.position().x - rect.origin.x) / rect.size.width).clamp(0., 1.);
                    let t = duration * f64::from(frac);
                    crate::audio::send("seek", serde_json::json!(t));
                    s.position = t;
                    cx.notify();
                }
            }))
    }

    fn breadcrumbs(&self, cx: &mut Context<Self>) -> Div {
        let mut bar = div()
            .h(px(37.))
            .flex_none()
            .flex()
            .items_center()
            .pl(px(80.))
            .pr(px(8.))
            .bg(metal())
            .gap(px(4.));
        let mut path = vec![("Musicman".to_string(), Page::Work)];
        let page = if self.now_view {
            self.display_track()
                .map(|t| Page::Album(t.album_artist.clone(), t.album.clone()))
                .unwrap_or(Page::Queue)
        } else {
            self.page.clone()
        };
        match &page {
            Page::Album(a, b) => {
                if let Some(p) = self
                    .lib
                    .picks
                    .iter()
                    .find(|p| &p.artist == a && &p.album == b)
                {
                    path.push(("Work music".into(), Page::Work));
                    path.push((p.group.clone(), Page::Group(p.group.clone())));
                } else {
                    path.push(("All songs".into(), Page::All));
                }
                if !b.is_empty() {
                    path.push((short_title(b), page.clone()));
                }
            }
            Page::Group(_) | Page::WorkTracks | Page::Playlist(_) => {
                let parent = if matches!(page,Page::Playlist(i) if !self.lib.c.playlists.contains(self.lib.data.playlists[i].key()))
                {
                    Page::All
                } else {
                    Page::Work
                };
                path.push((self.lib.name(&parent), parent));
                path.push((self.lib.name(&page), page.clone()));
            }
            Page::Hidden | Page::Files => {
                path.push(("All songs".into(), Page::All));
                path.push((self.lib.name(&page), page.clone()));
            }
            _ => path.push((self.lib.name(&page), page.clone())),
        }
        for (i, (title, dest)) in path.into_iter().enumerate() {
            if i > 0 {
                bar = bar.child(caption("/"));
            }
            bar = bar.child(
                div()
                    .id(("crumb", i))
                    .min_w_0()
                    .truncate()
                    .px(px(4.))
                    .cursor_pointer()
                    .hover(|s| s.bg(rgb(HOVER)).text_color(rgb(ACCENT)))
                    .child(title)
                    .on_click(cx.listener(move |s, _, w, cx| {
                        s.go(dest.clone(), cx);
                        w.focus(&s.focus);
                    })),
            );
        }
        if self.now_view {
            bar = bar.child(caption("/")).child(
                div()
                    .id("now-crumb")
                    .min_w_0()
                    .truncate()
                    .cursor_pointer()
                    .text_color(rgb(ACCENT))
                    .child(
                        self.display_track()
                            .map(|t| t.title.clone())
                            .unwrap_or_else(|| "Now playing".into()),
                    )
                    .on_click(cx.listener(|s, _, w, cx| {
                        if let Some(track) = s.display_track().cloned() {
                            let page = if track.album.is_empty() {
                                Page::Files
                            } else {
                                Page::Album(track.album_artist, track.album)
                            };
                            s.go(page, cx);
                            if let Some(row) = s
                                .rows
                                .iter()
                                .position(|&i| s.lib.data.tracks[i].id == track.id)
                            {
                                s.select(row, false, false);
                                s.scroll.scroll_to_item(row, ScrollStrategy::Center);
                            }
                        }
                        w.focus(&s.focus);
                        cx.notify();
                    })),
            );
        }
        bar.child(
            div()
                .flex_1()
                .h_full()
                .flex()
                .items_center()
                .px(px(10.))
                .child(rack::stripe())
                .window_control_area(WindowControlArea::Drag),
        )
    }

    fn eq_panel(&self, cx: &mut Context<Self>) -> Div {
        let gains = self.eq.gains();
        let mut presets = div()
            .id("eq-presets")
            .bg(rgb(GLASS))
            .overflow_x_scroll()
            .flex()
            .items_center()
            .gap(px(6.))
            .child(caption("PRESETS"));
        for (i, name) in self.eq.names().enumerate() {
            let key = name.to_owned();
            let active = self.eq.selected == name;
            presets = presets.child(
                div()
                    .id(("eq-preset", i))
                    .flex_none()
                    .px(px(7.))
                    .py(px(3.))
                    .cursor_pointer()
                    .text_color(rgb(BG))
                    .bg(keycap(active))
                    .child(key.clone())
                    .on_click(
                        cx.listener(move |s, _, _, cx| s.action(Action::Preset(key.clone()), cx)),
                    ),
            );
        }
        let mut bands = div().flex().flex_wrap().items_center().gap(px(8.));
        for (i, (name, v)) in [
            ("140 Hz", gains.low),
            ("500 Hz", gains.mid),
            ("1 kHz shelf", gains.high),
        ]
        .into_iter()
        .enumerate()
        {
            bands = bands.child(caption(format!("{name}  {v:+} dB")));
            for (button_id, delta) in [("−", -1), ("+", 1)] {
                bands = bands.child(
                    div()
                        .id(("band", i * 2 + usize::from(delta > 0)))
                        .text_color(rgb(BG))
                        .px(px(6.))
                        .bg(keycap(false))
                        .cursor_pointer()
                        .child(button_id)
                        .on_click(
                            cx.listener(move |s, _, _, cx| s.action(Action::Band(i, delta), cx)),
                        ),
                );
            }
        }
        div()
            .p(px(10.))
            .bg(rgb(SIDE))
            .flex()
            .flex_col()
            .gap(px(7.))
            .child(presets)
            .child(bands)
            .child(caption(format!(
                "{}   ·   ⌘K  save eq <name>",
                if self.active.is_empty() {
                    "Local file EQ"
                } else {
                    "Apple Music stream · EQ bypassed"
                }
            )))
    }

    fn library_panel(&self, name: String, width: f32, cx: &mut Context<Self>) -> Div {
        let command = div()
            .flex()
            .items_center()
            .gap(px(10.))
            .mx(px(20.))
            .my(px(12.))
            .px(px(10.))
            .border_1()
            .border_color(rgb(EDGE))
            .bg(rgb(GLASS))
            .child(div().text_color(rgb(PHOSPHOR)).font_family(MONO).child(">"))
            .child(div().flex_1().min_w_0().child(self.search.clone()))
            .child(caption("⌘ K"));
        let mut main = div()
            .flex_1()
            .min_w_0()
            .flex()
            .flex_col()
            .overflow_hidden()
            .child(command);
        if !self.choices.is_empty() {
            let matches = uniform_list(
                "command-matches",
                self.choices.len(),
                cx.processor(|s, range: std::ops::Range<usize>, _, cx| {
                    range
                        .map(|i| {
                            div()
                                .id(("choice", i))
                                .h(px(25.))
                                .px(px(8.))
                                .flex()
                                .items_center()
                                .bg(rgb(if i == s.choice_row { SELECTED } else { PANEL }))
                                .hover(|s| s.bg(rgb(HOVER)))
                                .cursor_pointer()
                                .child(label(s.choices[i].0.clone()))
                                .on_click(cx.listener(move |s, _, _, cx| s.choose(i, cx)))
                        })
                        .collect()
                }),
            )
            .h(px(25. * self.choices.len().min(6) as f32))
            .flex_none()
            .mx(px(20.))
            .track_scroll(self.choice_scroll.clone());
            main = main.child(matches);
        }
        let mut heading = div()
            .flex()
            .items_center()
            .gap(px(8.))
            .px(px(20.))
            .pt(px(3.))
            .pb(px(16.));
        if !self.back.is_empty() {
            heading = heading.child(self.button("back", "‹", Action::Back, false, cx))
        }
        heading = heading.child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .gap(px(3.))
                .child(
                    div()
                        .text_size(px(23.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .truncate()
                        .child(name.clone()),
                )
                .child(caption(if self.loading {
                    "Reading library…".into()
                } else if let Page::Album(a, b) = &self.page {
                    let mut years = self
                        .lib
                        .replay
                        .get(&(a.clone(), b.clone()))
                        .cloned()
                        .unwrap_or_default();
                    years.sort();
                    years.dedup();
                    let year = self
                        .rows
                        .first()
                        .and_then(|&i| self.lib.data.tracks[i].year)
                        .map(|y| format!(" · {y}"))
                        .unwrap_or_default();
                    format!(
                        "{a}{year}  ·  {} tracks{}",
                        self.rows.len(),
                        if years.is_empty() {
                            String::new()
                        } else {
                            format!("  ·  {}", years.join(", "))
                        }
                    )
                } else {
                    format!("{} tracks", self.rows.len())
                })),
        );
        let cards = self.lib.album_cards(&self.page);
        let home = self.page == Page::Work && self.query.is_empty() && !cards.is_empty();
        if home {
            heading = heading.child(self.button(
                "work-tracks",
                "Track list",
                Action::WorkTracks,
                false,
                cx,
            ));
        }
        if !self.rows.is_empty() && self.page != Page::Hidden {
            heading =
                heading.child(self.button("shuffle", "Shuffle + loop", Action::Shuffle, false, cx))
        }
        main = main.child(heading);
        if home {
            main = main.child(self.home_shelves(&cards, width, cx));
        } else {
            if self.query.is_empty() && !cards.is_empty() {
                main = main.child(self.deck(cards, width, cx))
            }
            if let Page::Album(a, b) = &self.page {
                main = main.child(
                    div()
                        .px(px(20.))
                        .pb(px(12.))
                        .child(self.art(self.lib.cover_id(a, b), 108.)),
                )
            }
            main = main.child(
                div()
                    .h(px(26.))
                    .flex_none()
                    .flex()
                    .items_center()
                    .gap(px(9.))
                    .px(px(16.))
                    .bg(metal())
                    .child(div().w(px(36.)).child(caption("#")))
                    .child(div().flex_1().child(caption("TITLE")))
                    .child(div().w(relative(0.24)).child(caption("ARTIST")))
                    .child(div().w(relative(0.21)).child(caption("ALBUM")))
                    .child(div().w(px(42.)).text_right().child(caption("TIME"))),
            );
            let songs = uniform_list(
                "songs",
                self.rows.len(),
                cx.processor(|s, range: std::ops::Range<usize>, _, cx| {
                    range.map(|r| s.row(r, cx)).collect()
                }),
            )
            .track_scroll(self.scroll.clone())
            .flex_1()
            .min_h_0();
            main = main.child(songs).child(line());
            if self.menu {
                main = main.child(self.edit_menu(cx))
            }
            let mut edit = div()
                .flex()
                .items_center()
                .gap(px(6.))
                .h(px(40.))
                .px(px(12.))
                .bg(metal());
            if self.page != Page::Hidden {
                edit = edit
                    .child(self.button("play-next", "Play next", Action::Enqueue(true), false, cx))
                    .child(self.button("add", "Add to queue", Action::Enqueue(false), false, cx))
            }
            if self.page == Page::Queue {
                edit = edit.child(self.button("remove", "Remove", Action::Remove, false, cx))
            }
            main = main
                .child(
                    edit.child(self.button("curate", "Curate  ···", Action::Menu, false, cx))
                        .child(div().flex_1()),
                )
                .child(line());
        }
        main
    }
    fn player(&self, cx: &mut Context<Self>) -> Div {
        let current = self.display_track();
        let duration = self.track_duration();
        let id = current.and_then(|t| {
            self.lib
                .cover_id(&t.album_artist, &t.album)
                .or(Some(t.id.as_str()))
        });
        let title = if self.pending.is_some() {
            "Loading track…".into()
        } else {
            current
                .map(|t| t.title.clone())
                .unwrap_or_else(|| "Nothing playing".into())
        };
        let mut row = div()
            .h(px(70.))
            .flex_none()
            .flex()
            .items_center()
            .gap(px(11.))
            .px(px(15.))
            .bg(metal())
            .child(
                div()
                    .id("now-cover")
                    .cursor_pointer()
                    .child(self.art(id, 44.))
                    .on_click(cx.listener(|s, _, _, cx| s.action(Action::NowPlaying, cx))),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .gap(px(3.))
                    .child(
                        div()
                            .id("now-title")
                            .text_color(rgb(PHOSPHOR))
                            .cursor_pointer()
                            .child(label(title))
                            .on_click(cx.listener(|s, _, _, cx| s.action(Action::NowPlaying, cx))),
                    )
                    .child(caption(
                        current
                            .map(|t| {
                                if self.current.is_none() {
                                    format!("Queued · {}", t.artist)
                                } else {
                                    t.artist.clone()
                                }
                            })
                            .unwrap_or_default(),
                    )),
            )
            .child(self.button("prev", "❮", Action::Previous, false, cx))
            .child(self.button(
                "toggle",
                if self.playing { "Pause" } else { "Play" },
                Action::Toggle,
                true,
                cx,
            ))
            .child(self.button("next", "❯", Action::Next, false, cx))
            .child(div().px(px(6.)).child(caption(format!(
                "{} / {}",
                clock(self.elapsed()),
                clock(duration)
            ))))
            .child(self.button(
                "repeat",
                format!(
                    "Repeat: {}",
                    match self.lib.queue.mode.as_str() {
                        "One song" => "one",
                        "All" => "all",
                        _ => "off",
                    }
                ),
                Action::Repeat,
                false,
                cx,
            ));
        row = row.child(self.button("eq", "EQ", Action::Equalizer, self.show_eq, cx));
        if self.current.is_some() {
            row = row.child(self.button("details", "Signal", Action::Inspect, false, cx))
        }
        row
    }
    fn key(&mut self, e: &KeyDownEvent, w: &mut Window, cx: &mut Context<Self>) {
        let k = &e.keystroke;
        let text = self.search.read(cx).focus_handle(cx).is_focused(w);
        if k.modifiers.platform {
            match k.key.as_str() {
                "f" | "k" => {
                    self.now_view = false;
                    w.focus(&self.search.read(cx).focus_handle(cx));
                }
                "j" => self.action(Action::NowPlaying, cx),
                "e" => self.action(Action::Equalizer, cx),
                "o" => self.action(Action::Open, cx),
                "i" => self.action(Action::Inspect, cx),
                "r" => self.action(Action::Refresh, cx),
                _ => return,
            }
        } else if !self.choices.is_empty()
            && matches!(k.key.as_str(), "up" | "down" | "enter" | "escape")
        {
            match k.key.as_str() {
                "enter" => {
                    self.choose(self.choice_row, cx);
                    w.focus(&self.focus);
                }
                "escape" => {
                    self.go(self.page.clone(), cx);
                    w.focus(&self.focus);
                }
                "up" => self.choice_row = self.choice_row.saturating_sub(1),
                _ => self.choice_row = (self.choice_row + 1).min(self.choices.len() - 1),
            }
            self.choice_scroll
                .scroll_to_item(self.choice_row, ScrollStrategy::Center);
        } else if text {
            match k.key.as_str() {
                "enter" => {
                    self.run(cx);
                    w.focus(&self.focus)
                }
                "escape" => {
                    self.go(self.page.clone(), cx);
                    w.focus(&self.focus)
                }
                _ => return,
            }
        } else {
            match k.key.as_str() {
                "up" | "down" if !self.rows.is_empty() => {
                    if self.page == Page::Work && self.query.is_empty() {
                        self.go(Page::WorkTracks, cx);
                    }
                    let r = if k.key == "down" {
                        self.selection
                            .last()
                            .map_or(0, |i| i + 1)
                            .min(self.rows.len() - 1)
                    } else {
                        self.selection
                            .first()
                            .copied()
                            .unwrap_or(0)
                            .saturating_sub(1)
                    };
                    self.select(r, k.modifiers.shift, false);
                    self.scroll.scroll_to_item(r, ScrollStrategy::Center)
                }
                "enter" if self.now_view => self.action(Action::Toggle, cx),
                "enter" => self.action(Action::Play, cx),
                "space" => self.action(Action::Toggle, cx),
                "backspace" | "delete" if self.page == Page::Queue => {
                    self.action(Action::Remove, cx)
                }
                "escape" if self.now_view => self.action(Action::NowPlaying, cx),
                "escape" => self.action(Action::Back, cx),
                _ => return,
            }
        }
        cx.stop_propagation();
        cx.notify()
    }
}
impl Render for Workspace {
    fn render(&mut self, w: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let width: f32 = w.viewport_size().width.into();
        let name = self.lib.name(&self.page);
        let titlebar = self.breadcrumbs(cx);
        let main = if self.now_view {
            self.now_panel(width, cx)
        } else {
            self.library_panel(name, width, cx)
        };
        let mut shell = div()
            .size_full()
            .flex()
            .flex_col()
            .bg(rgb(BG))
            .text_color(rgb(TEXT))
            .text_size(px(13.))
            .font_family(MONO)
            .track_focus(&self.focus)
            .on_key_down(cx.listener(Self::key))
            .child(titlebar)
            .child(line())
            .child(
                div()
                    .flex()
                    .flex_1()
                    .min_h_0()
                    .child(self.side(cx))
                    .child(main),
            );
        if !self.message.is_empty() {
            shell = shell.child(
                div()
                    .px(px(14.))
                    .py(px(5.))
                    .text_color(rgb(ACCENT))
                    .child(self.message.clone()),
            )
        }
        if self.show_eq {
            shell = shell.child(self.eq_panel(cx));
        }
        if self.inspector {
            shell = shell.child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap(px(16.))
                    .px(px(15.))
                    .py(px(8.))
                    .bg(rgb(SIDE))
                    .child(caption(format!("Source   {}", self.source)))
                    .child(caption(format!(
                        "Active stream   {}",
                        if self.active.is_empty() {
                            "—"
                        } else {
                            &self.active
                        }
                    )))
                    .child(self.button(
                        "match",
                        if self.matching {
                            "✓ Match rate"
                        } else {
                            "Match rate: off"
                        },
                        Action::Match,
                        false,
                        cx,
                    )),
            )
        }
        if !self.now_view && self.current.is_some() {
            shell = shell.child(self.timeline(cx));
        }
        if !self.now_view || self.display_track().is_none() {
            shell = shell.child(line()).child(self.player(cx));
        }
        shell.child(
            div()
                .h(px(24.))
                .flex_none()
                .px(px(12.))
                .flex()
                .justify_between()
                .items_center()
                .bg(rgb(STATUS))
                .child(caption(if self.source.is_empty() {
                    "Audio idle".into()
                } else {
                    if self.active.is_empty() {
                        self.source.clone()
                    } else {
                        format!("{}  ·  {}", self.source, self.active)
                    }
                }))
                .child(caption(self.output.clone())),
        )
    }
}
fn short_title(s: &str) -> String {
    s.split(" (Original ")
        .next()
        .unwrap_or(s)
        .split(" (Music from ")
        .next()
        .unwrap_or(s)
        .to_owned()
}
