use std::cell::RefCell;
use std::path::Path;
use std::rc::Rc;

use gtk4::prelude::*;
use gtk4::{DrawingArea, GestureDrag, Overlay, Picture};

use crate::config::Crop;

#[derive(Clone)]
pub struct CropState {
    pub crop: Option<Crop>,
    pub aspect: Option<f64>,
    drag_start_crop: Option<Crop>,
    pub handle: Option<Handle>,
    pub widget_w: f64,
    pub widget_h: f64,
}

#[derive(Clone, Copy, PartialEq)]
pub enum Handle {
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
    Move,
}

#[derive(Clone)]
pub struct CropEditor {
    pub overlay: Overlay,
    pub picture: Picture,
    pub drawing: DrawingArea,
    pub state: Rc<RefCell<CropState>>,
}

impl CropEditor {
    pub fn new(monitor_aspect: Option<f64>) -> Self {
        let picture = Picture::new();
        // GTK 4.6: no ContentFit API — Picture scales to fill by default.
        // We set the picture to fill the container by expanding it.
        picture.set_hexpand(true);
        picture.set_vexpand(true);

        let drawing = DrawingArea::new();
        drawing.set_can_target(true);
        drawing.set_hexpand(true);
        drawing.set_vexpand(true);

        let overlay = Overlay::new();
        overlay.set_child(Some(&picture));
        overlay.add_overlay(&drawing);
        overlay.set_clip_overlay(&drawing, true);

        let state = Rc::new(RefCell::new(CropState {
            crop: None,
            aspect: monitor_aspect,
            drag_start_crop: None,
            handle: None,
            widget_w: 1.0,
            widget_h: 1.0,
        }));

        // Draw callback.
        let state_draw = state.clone();
        drawing.set_draw_func(move |_widget, cr, w, h| {
            let mut s = state_draw.borrow_mut();
            s.widget_w = w as f64;
            s.widget_h = h as f64;
            draw_crop_overlay(cr, &s, w, h);
        });

        // Drag gesture.
        let gesture = GestureDrag::new();

        let state_begin = state.clone();
        let draw_begin = drawing.clone();
        gesture.connect_drag_begin(move |_, x, y| {
            let mut s = state_begin.borrow_mut();
            let (nw, nh) = (s.widget_w, s.widget_h);
            let nx = x / nw;
            let ny = y / nh;
            if let Some(crop) = &s.crop {
                s.handle = hit_test(crop, nx, ny);
            }
            if s.handle.is_none() {
                // Start new crop from scratch.
                let new_crop = Crop {
                    x: nx,
                    y: ny,
                    w: 0.001,
                    h: 0.001,
                };
                s.crop = Some(new_crop);
                s.drag_start_crop = Some(new_crop);
                s.handle = Some(Handle::BottomRight);
            } else {
                s.drag_start_crop = s.crop;
            }
            drop(s);
            draw_begin.queue_draw();
        });

        let state_update = state.clone();
        let draw_update = drawing.clone();
        gesture.connect_drag_update(move |_, total_dx, total_dy| {
            let mut s = state_update.borrow_mut();
            let (nw, nh) = (s.widget_w, s.widget_h);
            let ndx = total_dx / nw;
            let ndy = total_dy / nh;
            if let (Some(handle), Some(start)) = (s.handle, s.drag_start_crop) {
                let mut crop = start;
                apply_handle(&mut crop, handle, ndx, ndy, s.aspect);
                s.crop = Some(clamp_crop(crop));
            }
            drop(s);
            draw_update.queue_draw();
        });

        let state_end = state.clone();
        let draw_end = drawing.clone();
        gesture.connect_drag_end(move |_, _dx, _dy| {
            let mut s = state_end.borrow_mut();
            s.handle = None;
            s.drag_start_crop = None;
            // Collapse tiny crops (accidental click).
            if let Some(c) = s.crop {
                if c.w < 0.02 || c.h < 0.02 {
                    s.crop = None;
                }
            }
            drop(s);
            draw_end.queue_draw();
        });
        drawing.add_controller(gesture);

        CropEditor {
            overlay,
            picture,
            drawing,
            state,
        }
    }

    pub fn set_media(&self, path: &Path) {
        let file = gtk4::gio::File::for_path(path);
        self.picture.set_file(Some(&file));
    }

    pub fn set_crop(&self, crop: Option<Crop>) {
        self.state.borrow_mut().crop = crop;
        self.drawing.queue_draw();
    }

    /// Current crop, sanitized. None = full frame (no crop).
    pub fn crop(&self) -> Option<Crop> {
        self.state.borrow().crop.and_then(|c| c.sanitized())
    }

    pub fn reset(&self) {
        self.state.borrow_mut().crop = None;
        self.drawing.queue_draw();
    }
}

fn draw_crop_overlay(cr: &gtk4::cairo::Context, state: &CropState, w: i32, h: i32) {
    let wf = w as f64;
    let hf = h as f64;

    let Some(crop) = state.crop else { return };

    let rx = crop.x * wf;
    let ry = crop.y * hf;
    let rw = crop.w * wf;
    let rh = crop.h * hf;

    // Darken area outside crop.
    cr.set_source_rgba(0.0, 0.0, 0.0, 0.55);
    cr.paint().ok();
    cr.set_operator(gtk4::cairo::Operator::Clear);
    cr.rectangle(rx, ry, rw, rh);
    cr.fill().ok();
    cr.set_operator(gtk4::cairo::Operator::Over);

    // White border.
    cr.set_source_rgba(1.0, 1.0, 1.0, 0.9);
    cr.set_line_width(1.5);
    cr.rectangle(rx, ry, rw, rh);
    cr.stroke().ok();

    // Rule-of-thirds lines.
    cr.set_source_rgba(1.0, 1.0, 1.0, 0.3);
    cr.set_line_width(0.8);
    for i in 1..3 {
        let x = rx + rw * i as f64 / 3.0;
        cr.move_to(x, ry);
        cr.line_to(x, ry + rh);
        cr.stroke().ok();
        let y = ry + rh * i as f64 / 3.0;
        cr.move_to(rx, y);
        cr.line_to(rx + rw, y);
        cr.stroke().ok();
    }

    // Corner handles (8×8 white squares).
    cr.set_source_rgba(1.0, 1.0, 1.0, 1.0);
    for (hx, hy) in [(rx, ry), (rx + rw, ry), (rx, ry + rh), (rx + rw, ry + rh)] {
        cr.rectangle(hx - 4.0, hy - 4.0, 8.0, 8.0);
        cr.fill().ok();
    }
}

fn hit_test(crop: &Crop, nx: f64, ny: f64) -> Option<Handle> {
    let hs = 0.05;
    let corners = [
        (crop.x, crop.y, Handle::TopLeft),
        (crop.x + crop.w, crop.y, Handle::TopRight),
        (crop.x, crop.y + crop.h, Handle::BottomLeft),
        (crop.x + crop.w, crop.y + crop.h, Handle::BottomRight),
    ];
    for (cx, cy, handle) in corners {
        if (nx - cx).abs() < hs && (ny - cy).abs() < hs {
            return Some(handle);
        }
    }
    if nx > crop.x && nx < crop.x + crop.w && ny > crop.y && ny < crop.y + crop.h {
        return Some(Handle::Move);
    }
    None
}

fn apply_handle(crop: &mut Crop, handle: Handle, dx: f64, dy: f64, aspect: Option<f64>) {
    match handle {
        Handle::TopLeft => {
            crop.x += dx;
            crop.y += dy;
            crop.w -= dx;
            crop.h -= dy;
        }
        Handle::TopRight => {
            crop.y += dy;
            crop.w += dx;
            crop.h -= dy;
        }
        Handle::BottomLeft => {
            crop.x += dx;
            crop.w -= dx;
            crop.h += dy;
        }
        Handle::BottomRight => {
            crop.w += dx;
            crop.h += dy;
        }
        Handle::Move => {
            crop.x += dx;
            crop.y += dy;
        }
    }
    if let Some(ar) = aspect {
        if handle != Handle::Move {
            crop.h = crop.w / ar;
        }
    }
}

fn clamp_crop(mut c: Crop) -> Crop {
    c.w = c.w.max(0.02);
    c.h = c.h.max(0.02);
    c.x = c.x.clamp(0.0, 1.0 - c.w);
    c.y = c.y.clamp(0.0, 1.0 - c.h);
    c
}
