//! Mikroanimacje powłoki wg mocka „tryby edytora”: przejścia 80/150/240 ms z krzywą ease-out
//! (≈ cubic-bezier(.2,.8,.2,1)) albo z przeskokiem (gałki przełączników, człowiek na ekranie ładowania).
//! Czas z `egui` (`i.time`), nie `std::time::Instant` — ten panikuje na wasm32.

use bevy_egui::egui::{self, Color32, Id, Pos2, Shape, Stroke};
use std::sync::atomic::{AtomicBool, Ordering};

/// Wciśnięcie przycisku.
pub const PRESS: f32 = 0.08;
/// Kolory, pigułki, przełączniki, crossfade.
pub const FAST: f32 = 0.15;
/// Panele i toasty (wjazd).
pub const MED: f32 = 0.24;

/// Odpowiednik `prefers-reduced-motion`: wszystkie przejścia natychmiastowe.
pub static REDUCED: AtomicBool = AtomicBool::new(false);

pub fn lerp_t(p: f32, a: f32, b: f32) -> f32 {
    ((p - a) / (b - a)).clamp(0.0, 1.0)
}
pub fn ease_out(x: f32) -> f32 {
    1.0 - (1.0 - x).powi(3)
}
/// Ease-out z przeskokiem (≈ cubic-bezier(.3,1.4,.5,1)).
pub fn ease_back(x: f32) -> f32 {
    let (c1, x) = (1.6f32, x - 1.0);
    1.0 + (c1 + 1.0) * x.powi(3) + c1 * x * x
}
pub fn hex(h: u32) -> Color32 {
    Color32::from_rgb((h >> 16) as u8, (h >> 8) as u8, h as u8)
}
pub fn mix(a: Color32, b: Color32, t: f32) -> Color32 {
    a.lerp_to_gamma(b, t.clamp(0.0, 1.0))
}

#[derive(Clone, Copy)]
struct Tween {
    from: f32,
    to: f32,
    t0: f64,
}

/// Wartość tweenu po `dt` sekundach (czysta funkcja — testowana niżej).
fn tween_at(from: f32, to: f32, dt: f32, secs: f32, curve: fn(f32) -> f32) -> (f32, bool) {
    if secs <= 0.0 || dt >= secs {
        return (to, true);
    }
    (from + (to - from) * curve((dt / secs).max(0.0)), false)
}

/// Wartość płynnie dążąca do `target`; zmiana celu w trakcie startuje od bieżącej wartości.
pub fn ease_with(ctx: &egui::Context, id: Id, target: f32, secs: f32, curve: fn(f32) -> f32) -> f32 {
    let now = ctx.input(|i| i.time);
    let secs = if REDUCED.load(Ordering::Relaxed) { 0.0 } else { secs };
    let tw = ctx.data_mut(|d| {
        let tw = d.get_temp_mut_or_insert_with(id, || Tween { from: target, to: target, t0: now });
        if tw.to != target {
            let (cur, _) = tween_at(tw.from, tw.to, (now - tw.t0) as f32, secs, curve);
            *tw = Tween { from: cur, to: target, t0: now };
        }
        *tw
    });
    let (v, done) = tween_at(tw.from, tw.to, (now - tw.t0) as f32, secs, curve);
    if !done {
        ctx.request_repaint();
    }
    v
}

pub fn ease(ctx: &egui::Context, id: Id, target: f32, secs: f32) -> f32 {
    ease_with(ctx, id, target, secs, ease_out)
}

/// 0 → 1 przy pierwszej klatce, w której element jest rysowany (także po przerwie — np. panel znów otwarty).
pub fn appear(ctx: &egui::Context, id: Id, secs: f32) -> f32 {
    // dwa osobne wywołania: `ctx` trzyma blokadę wewnątrz `input`, zagnieżdżenie = zakleszczenie
    let now = ctx.input(|i| i.time);
    let frame = ctx.cumulative_frame_nr();
    let secs = if REDUCED.load(Ordering::Relaxed) { 0.0 } else { secs };
    let t0 = ctx.data_mut(|d| {
        let s = d.get_temp_mut_or_insert_with::<(u64, f64)>(id, || (frame, now));
        if s.0 + 1 < frame {
            s.1 = now;
        }
        s.0 = frame;
        s.1
    });
    let (v, done) = tween_at(0.0, 1.0, (now - t0) as f32, secs, ease_out);
    if !done {
        ctx.request_repaint();
    }
    v
}

/// Błysk po zmianie wartości: krycie 0,35 → 0 w 240 ms.
pub fn flash(ctx: &egui::Context, id: Id, changed: bool) -> f32 {
    let now = ctx.input(|i| i.time);
    if REDUCED.load(Ordering::Relaxed) {
        return 0.0;
    }
    let t0 = ctx.data_mut(|d| {
        let t = d.get_temp_mut_or_insert_with::<f64>(id, || f64::NEG_INFINITY);
        if changed {
            *t = now;
        }
        *t
    });
    let dt = (now - t0) as f32;
    if dt < MED {
        ctx.request_repaint();
        0.35 * (1.0 - dt / MED)
    } else {
        0.0
    }
}

/// Skala wciśniętego przycisku (0,96 przez 80 ms) — do `rect.shrink`.
pub fn press(ctx: &egui::Context, resp: &egui::Response) -> f32 {
    ease(ctx, resp.id.with("press"), if resp.is_pointer_button_down_on() { 0.96 } else { 1.0 }, PRESS)
}

/// Prostokąt przeskalowany wokół środka (efekt wciśnięcia).
pub fn scaled(r: egui::Rect, s: f32) -> egui::Rect {
    egui::Rect::from_center_size(r.center(), r.size() * s)
}

/// Kręciołek: łuk 3/4 obrotu na 0,8 s.
pub fn spinner(p: &egui::Painter, c: Pos2, r: f32, col: Color32, width: f32) {
    let t = p.ctx().input(|i| i.time) as f32;
    let a0 = t / 0.8 * std::f32::consts::TAU;
    let pts: Vec<Pos2> = (0..=24).map(|k| a0 + k as f32 / 24.0 * 1.5 * std::f32::consts::PI).map(|a| c + egui::vec2(a.cos(), a.sin()) * r).collect();
    p.add(Shape::line(pts, Stroke::new(width, col)));
    p.ctx().request_repaint();
}

/// Przesuwane tło segmentu (pigułka trybu, widok, nawigacja): rezerwuje miejsce w kolejce
/// rysowania przed przyciskami, a po ich ułożeniu rysuje tam tło przy animowanym prostokącie.
pub struct Slide {
    idx: egui::layers::ShapeIdx,
}

impl Slide {
    pub fn begin(ui: &egui::Ui) -> Self {
        Self { idx: ui.painter().add(Shape::Noop) }
    }
    pub fn end(self, ui: &egui::Ui, id: Id, target: egui::Rect, draw: impl FnOnce(egui::Rect) -> Vec<Shape>) {
        let ctx = ui.ctx();
        let l = ease(ctx, id.with("l"), target.left(), FAST);
        let w = ease(ctx, id.with("w"), target.width(), FAST);
        let r = egui::Rect::from_min_size(egui::pos2(l, target.top()), egui::vec2(w, target.height()));
        ui.painter().set(self.idx, Shape::Vec(draw(r)));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn curves_hit_ends_and_back_overshoots() {
        assert_eq!(ease_out(0.0), 0.0);
        assert_eq!(ease_out(1.0), 1.0);
        let xs: Vec<f32> = (0..=20).map(|k| ease_out(k as f32 / 20.0)).collect();
        assert!(xs.windows(2).all(|w| w[1] >= w[0]));
        assert!((ease_back(1.0) - 1.0).abs() < 1e-6);
        assert!((0..20).any(|k| ease_back(k as f32 / 20.0) > 1.0));
    }

    #[test]
    fn tween_reaches_target_and_retargets_from_current() {
        assert_eq!(tween_at(0.0, 10.0, 0.0, 0.15, ease_out), (0.0, false));
        assert_eq!(tween_at(0.0, 10.0, 0.2, 0.15, ease_out), (10.0, true));
        let (mid, done) = tween_at(0.0, 10.0, 0.075, 0.15, ease_out);
        assert!(!done && mid > 5.0 && mid < 10.0, "ease-out jest za połową w połowie czasu: {mid}");
        assert_eq!(tween_at(3.0, 7.0, 1.0, 0.0, ease_out), (7.0, true));
    }
}
