//! StykAnimate, native Rust prototype (no WebView).
//!
//! Phase 1 proves the toolchain and the feel: a native egui window on Android with a touch
//! timeline and a stick figure drawn by the GPU. Phase 2 loads a real .piv/.stk file.

use eframe::egui::{self, Color32, Pos2, Rect, Stroke, Vec2};

pub struct StykApp {
    playing: bool,
    frame: f32,
    frames: u32,
    fps: f32,
}

impl Default for StykApp {
    fn default() -> Self {
        Self { playing: true, frame: 0.0, frames: 24, fps: 12.0 }
    }
}

impl eframe::App for StykApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if self.playing {
            self.frame += ctx.input(|i| i.stable_dt) * self.fps;
            if self.frame >= self.frames as f32 {
                self.frame -= self.frames as f32;
            }
            ctx.request_repaint();
        }

        egui::TopBottomPanel::bottom("timeline").show(ctx, |ui| {
            ui.add_space(10.0);
            ui.horizontal(|ui| {
                let label = if self.playing { "Pause" } else { "Play" };
                if ui.add_sized([84.0, 52.0], egui::Button::new(label)).clicked() {
                    self.playing = !self.playing;
                }
                ui.spacing_mut().slider_width = (ui.available_width() - 110.0).max(80.0);
                let mut f = self.frame;
                let last = (self.frames - 1) as f32;
                if ui.add(egui::Slider::new(&mut f, 0.0..=last).integer().text("frame")).changed() {
                    self.frame = f;
                    self.playing = false;
                }
            });
            ui.add_space(10.0);
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            let (rect, _) = ui.allocate_exact_size(ui.available_size(), egui::Sense::hover());
            let painter = ui.painter_at(rect);
            let side = rect.width().min(rect.height()) * 0.9;
            let canvas = Rect::from_center_size(rect.center(), Vec2::splat(side));
            painter.rect_filled(canvas, 4.0, Color32::WHITE);
            draw_figure(&painter, canvas, self.frame / self.frames as f32);
        });
    }
}

/// A placeholder walking pose, `t` in 0..1 over the loop.
fn draw_figure(painter: &egui::Painter, canvas: Rect, t: f32) {
    let s = canvas.height() / 100.0;
    let p = |x: f32, y: f32| -> Pos2 { canvas.min + Vec2::new(x, y) * s };
    let swing = (t * std::f32::consts::TAU).sin() * 0.8;
    let blue = Color32::from_rgb(10, 153, 255);
    let stroke = Stroke::new((s * 2.2).max(3.0), blue);

    let shoulder = p(50.0, 36.0);
    let hip = p(50.0, 58.0);
    let hand_a = p(50.0 + 20.0 * swing.sin(), 36.0 + 20.0 * swing.cos());
    let hand_b = p(50.0 - 20.0 * swing.sin(), 36.0 + 20.0 * swing.cos());
    let foot_a = p(50.0 - 16.0 * swing.sin(), 58.0 + 24.0 * (swing * 0.6).cos());
    let foot_b = p(50.0 + 16.0 * swing.sin(), 58.0 + 24.0 * (swing * 0.6).cos());

    painter.line_segment([p(50.0, 31.0), hip], stroke);
    painter.line_segment([shoulder, hand_a], stroke);
    painter.line_segment([shoulder, hand_b], stroke);
    painter.line_segment([hip, foot_a], stroke);
    painter.line_segment([hip, foot_b], stroke);
    painter.circle_filled(p(50.0, 25.0), s * 6.0, blue);
    for joint in [shoulder, hip, hand_a, hand_b, foot_a, foot_b] {
        painter.circle_filled(joint, s * 1.1, Color32::from_rgb(220, 30, 30));
    }
}

#[cfg(target_os = "android")]
#[no_mangle]
fn android_main(app: winit::platform::android::activity::AndroidApp) {
    android_logger::init_once(android_logger::Config::default().with_max_level(log::LevelFilter::Info));
    let options = eframe::NativeOptions { android_app: Some(app), ..Default::default() };
    eframe::run_native(
        "StykAnimate",
        options,
        Box::new(|_cc| Ok(Box::new(StykApp::default()))),
    )
    .expect("eframe failed to start");
}
