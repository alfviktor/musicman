use super::*;

const ETCH: u32 = 0x899a86;
const CHASSIS: u32 = 0x303541;

fn bezel() -> Div {
    div()
        .flex_none()
        .p(px(2.))
        .bg(finish(0x888b90, 0x101216))
        .flex()
        .flex_col()
}

pub(super) fn stripe() -> Div {
    let mut rails = div().flex_1().h(px(9.)).flex().flex_col().justify_between();
    for _ in 0..3 {
        rails = rails.child(div().w_full().h(px(1.)).bg(rgb(0x929387)));
    }
    rails
}

pub(super) fn faceplate(title: &str) -> Div {
    div()
        .h(px(24.))
        .px(px(10.))
        .flex()
        .items_center()
        .gap(px(10.))
        .bg(finish(0x434857, 0x272d38))
        .child(stripe())
        .child(
            div()
                .text_size(px(11.))
                .font_weight(FontWeight::BOLD)
                .text_color(rgb(0xe1e2d3))
                .child(title.to_owned()),
        )
        .child(stripe())
}

fn stamp(text: impl Into<SharedString>) -> Div {
    div()
        .text_size(px(10.))
        .text_color(rgb(0xb4b7ad))
        .child(text.into())
}

// Seven-segment LCD. The unlit segments remain visible in the smoked display.
fn counter(seconds: f64) -> Div {
    const SEGMENTS: [u8; 10] = [0x3f, 0x06, 0x5b, 0x4f, 0x66, 0x6d, 0x7d, 0x07, 0x7f, 0x6f];
    let t = seconds.max(0.) as u64;
    let digits = format!("{:02}:{:02}", t / 60, t % 60);
    let mut row = div().flex().items_center().gap(px(4.));
    for ch in digits.chars() {
        let Some(n) = ch.to_digit(10) else {
            row = row.child(
                div()
                    .w(px(3.))
                    .h(px(13.))
                    .flex()
                    .flex_col()
                    .justify_between()
                    .child(div().size(px(3.)).bg(rgb(PHOSPHOR)))
                    .child(div().size(px(3.)).bg(rgb(PHOSPHOR))),
            );
            continue;
        };
        let mask = SEGMENTS[n as usize];
        row = row.child(
            canvas(
                |_, _, _| (),
                move |b, _, window, _| {
                    for (i, (x, y, w, h)) in [
                        (3., 0., 12., 2.),
                        (15., 2., 2., 11.),
                        (15., 16., 2., 11.),
                        (3., 27., 12., 2.),
                        (1., 16., 2., 11.),
                        (1., 2., 2., 11.),
                        (3., 13.5, 12., 2.),
                    ]
                    .into_iter()
                    .enumerate()
                    {
                        let rect = Bounds::new(b.origin + point(px(x), px(y)), size(px(w), px(h)));
                        window.paint_quad(fill(
                            rect,
                            rgb(if mask & (1 << i) != 0 {
                                PHOSPHOR
                            } else {
                                0x223027
                            }),
                        ));
                    }
                },
            )
            .w(px(18.))
            .h(px(29.)),
        );
    }
    row
}

impl Workspace {
    fn piano_key(
        &self,
        id: &'static str,
        symbol: &str,
        title: &'static str,
        active: bool,
        action: Action,
        cx: &mut Context<Self>,
    ) -> Div {
        let key = div()
            .id(id)
            .w_full()
            .h(px(29.))
            .border_1()
            .border_color(rgb(0x101319))
            .p(px(2.))
            .bg(finish(0xf2f0dd, 0x616975))
            .cursor_pointer()
            .hover(|s| s.bg(rgb(0xe5ddb9)))
            .active(|s| s.bg(rgb(0x626871)))
            .child(
                div()
                    .size_full()
                    .flex()
                    .items_center()
                    .justify_center()
                    .bg(keycap(active))
                    .text_size(px(14.))
                    .font_weight(FontWeight::BOLD)
                    .text_color(rgb(0x212734))
                    .child(symbol.to_owned()),
            )
            .on_click(cx.listener(move |s, _, _, cx| s.action(action.clone(), cx)));
        div()
            .flex_1()
            .min_w_0()
            .flex()
            .flex_col()
            .gap(px(4.))
            .child(stamp(title))
            .child(key)
    }

    pub(super) fn now_panel(&self, width: f32, cx: &mut Context<Self>) -> Div {
        let narrow = width < 930.;
        let mut rack = div()
            .id("rack-scroll")
            .overflow_y_scroll()
            .size_full()
            .flex()
            .flex_col()
            .p(px(if narrow { 12. } else { 22. }))
            .gap(px(12.));
        let Some(track) = self.display_track() else {
            return div()
                .flex_1()
                .p(px(22.))
                .child(label("Queue a track to open Now Playing."));
        };
        let album_id = self
            .lib
            .cover_id(&track.album_artist, &track.album)
            .unwrap_or(&track.id);
        let status = if self.pending.is_some() {
            "LOADING"
        } else if self.ended {
            "FINISHED"
        } else if self.playing {
            "PLAYING"
        } else if self.current.is_some() {
            "PAUSED"
        } else {
            "QUEUED"
        };
        let track_number = self
            .lib
            .queue
            .position
            .map_or("—".to_owned(), |i| format!("{:02}", i + 1));
        let stamp_row = div()
            .flex()
            .items_center()
            .justify_between()
            .gap(px(6.))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(7.))
                    .child(
                        div()
                            .size(px(6.))
                            .bg(rgb(if self.playing { PHOSPHOR } else { ACCENT })),
                    )
                    .child(stamp(status)),
            )
            .child(stamp(format!("TRACK {track_number}")));
        let glass = div()
            .flex_1()
            .min_w_0()
            .flex()
            .flex_col()
            .justify_between()
            .gap(px(8.))
            .p(px(13.))
            .border_1()
            .border_color(rgb(0x171919))
            .bg(finish(0x19211c, GLASS))
            .child(stamp_row)
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(2.))
                    .child(
                        div()
                            .text_color(rgb(PHOSPHOR))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_size(px(if narrow { 17. } else { 21. }))
                            .line_height(px(24.))
                            .child(track.title.clone()),
                    )
                    .child(
                        div()
                            .text_size(px(13.))
                            .text_color(rgb(0xc6d2bf))
                            .child(track.artist.clone()),
                    )
                    .child(
                        div()
                            .text_color(rgb(ETCH))
                            .text_size(px(11.))
                            .child(short_title(&track.album)),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .items_end()
                    .justify_between()
                    .gap(px(6.))
                    .child(counter(self.position))
                    .child(stamp(format!(
                        "−{}  /  {}",
                        clock((self.track_duration() - self.position).max(0.)),
                        clock(self.track_duration())
                    ))),
            )
            .child(self.timeline(cx));
        let sleeve = bezel().child(
            div()
                .p(px(4.))
                .bg(rgb(0x0c0c10))
                .child(self.art(Some(album_id), if narrow { 123. } else { 163. })),
        );
        let display = div()
            .flex()
            .gap(px(10.))
            .items_center()
            .child(sleeve)
            .child(glass);
        let mut keys = div().flex().gap(px(4.));
        let repeat = match self.lib.queue.mode.as_str() {
            "One song" => "ONE",
            "All" => "ALL",
            _ => "OFF",
        };
        for (id, symbol, title, lit, act) in [
            ("rack-prev", "◀◀", "PREV", false, Action::Previous),
            (
                "rack-play",
                if self.playing { "Ⅱ" } else { "▶" },
                if self.playing { "PAUSE" } else { "PLAY" },
                true,
                Action::Toggle,
            ),
            ("rack-next", "▶▶", "NEXT", false, Action::Next),
            (
                "rack-repeat",
                repeat,
                "REPEAT",
                repeat != "OFF",
                Action::Repeat,
            ),
            ("rack-eq", "EQ", "PRESET", self.show_eq, Action::Equalizer),
            (
                "rack-signal",
                "≡",
                "SIGNAL",
                self.inspector,
                Action::Inspect,
            ),
        ] {
            keys = keys.child(self.piano_key(id, symbol, title, lit, act, cx));
        }
        let player = div()
            .p(px(11.))
            .flex()
            .flex_col()
            .gap(px(12.))
            .bg(finish(CHASSIS, 0x222530))
            .child(display)
            .child(keys);
        rack = rack.child(bezel().child(faceplate("MUSICMAN")).child(player));
        let mut signal = div()
            .p(px(10.))
            .flex()
            .flex_col()
            .gap(px(4.))
            .bg(rgb(GLASS));
        for (label, value) in [
            ("SOURCE", self.source.as_str()),
            ("STREAM", self.active.as_str()),
            ("OUTPUT", self.output.as_str()),
            ("EQ", self.eq_route.as_str()),
        ] {
            if !value.is_empty() {
                signal = signal.child(
                    div()
                        .flex()
                        .items_start()
                        .gap(px(8.))
                        .child(div().w(px(52.)).flex_none().child(stamp(label)))
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .text_size(px(11.))
                                .text_color(rgb(PHOSPHOR))
                                .child(value.to_owned()),
                        ),
                );
            }
        }
        let gains = self.eq.gains();
        let mut bands = div().flex().items_center().gap(px(11.));
        for (label, gain) in [("140", gains.low), ("500", gains.mid), ("1k", gains.high)] {
            let fader = canvas(
                |_, _, _| (),
                move |b, _, w, _| {
                    let rect = |x, y, width, height| {
                        Bounds::new(b.origin + point(px(x), px(y)), size(px(width), px(height)))
                    };
                    for i in 0..7 {
                        w.paint_quad(fill(rect(0., 3. + i as f32 * 4., 24., 1.), rgb(0x723f37)));
                    }
                    w.paint_quad(fill(rect(10., 0., 4., 30.), rgb(0x07090b)));
                    let y = 3. + (6. - f32::from(gain)) * 2.;
                    w.paint_quad(fill(rect(5., y - 2., 14., 5.), keycap(false)));
                },
            )
            .w(px(24.))
            .h(px(30.));
            bands = bands.child(
                div()
                    .flex()
                    .flex_col()
                    .items_center()
                    .gap(px(2.))
                    .child(fader)
                    .child(stamp(label)),
            );
        }
        let curve = div()
            .id("rack-curve")
            .cursor_pointer()
            .flex_none()
            .py(px(6.))
            .px(px(12.))
            .flex()
            .flex_col()
            .gap(px(5.))
            .bg(metal())
            .child(stamp(format!("EQ · {}", self.eq.selected)))
            .child(bands)
            .on_click(cx.listener(|s, _, _, cx| s.action(Action::Equalizer, cx)));
        rack = rack.child(
            bezel().child(
                div()
                    .flex()
                    .bg(rgb(GLASS))
                    .items_center()
                    .child(signal.flex_1().min_w_0())
                    .child(curve),
            ),
        );
        let q = &self.lib.queue;
        let start = q.position.unwrap_or(0);
        let mut list = div().bg(rgb(GLASS)).flex().flex_col().p(px(5.));
        for (i, id) in q.ids.iter().enumerate().skip(start).take(6) {
            let Some(t) = self
                .lib
                .ids
                .get(id)
                .and_then(|i| self.lib.data.tracks.get(*i))
            else {
                continue;
            };
            let selected = q.position == Some(i);
            list = list.child(
                div()
                    .id(("rack-song", i))
                    .h(px(26.))
                    .px(px(6.))
                    .flex()
                    .gap(px(8.))
                    .items_center()
                    .bg(rgb(if selected { 0x283139 } else { GLASS }))
                    .cursor_pointer()
                    .hover(|s| s.bg(rgb(0x29312c)))
                    .text_color(rgb(if selected { 0xe4e6d9 } else { PHOSPHOR }))
                    .child(
                        div()
                            .w(px(27.))
                            .flex_none()
                            .text_color(rgb(ETCH))
                            .child(format!("{:02}", i + 1)),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .child(format!("{}  ·  {}", t.title, t.artist)),
                    )
                    .child(div().text_size(px(11.)).child(clock(t.seconds)))
                    .on_click(cx.listener(move |s, _, w, cx| {
                        s.go(Page::Queue, cx);
                        s.select(i, false, false);
                        s.scroll.scroll_to_item(i, ScrollStrategy::Center);
                        w.focus(&s.focus);
                        cx.notify();
                    })),
            );
        }
        let queue_head = div()
            .id("rack-queue")
            .cursor_pointer()
            .child(faceplate(&format!(
                "PLAY QUEUE   {}/{}   ›",
                if q.ids.is_empty() { 0 } else { start + 1 },
                q.ids.len()
            )))
            .on_click(cx.listener(|s, _, _, cx| s.go(Page::Queue, cx)));
        rack = rack.child(bezel().child(queue_head).child(list));
        div()
            .flex_1()
            .min_w_0()
            .min_h_0()
            .bg(rgb(0x151619))
            .child(rack)
    }
}
