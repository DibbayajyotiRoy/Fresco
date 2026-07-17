//! Reusable "hover to play" preview behaviour for video/GIF library cards.
//!
//! While the pointer is over a card we want it to come alive: a muted, looping
//! inline video preview fades in over the static thumbnail, and on leave the
//! thumbnail shows again.
//!
//! The trick is that the preview must NEVER influence the card's measured
//! size. A raw video paintable reports its full resolution (an 8K wallpaper
//! reports 7680×4320) as its natural size; putting it where layout can see it
//! makes the FlowBox re-measure, the card grows and shifts, the pointer falls
//! outside it, hover "leaves", the card snaps back, hover "enters" — an
//! endless glitch loop. So the card's base child stays the thumbnail
//! [`gtk4::Picture`] (constant intrinsic size), and the video lives in a
//! separate `Picture` stacked in a [`gtk4::Overlay`] — overlay children are
//! excluded from size measurement, so the card's geometry never changes.
//!
//! A [`gtk4::MediaFile`] is created lazily on first hover (so we never decode
//! every card up front) and kept alive afterwards for snappy re-hover. It is
//! configured muted + looping. Decoding is best-effort: if no GStreamer
//! plugins are installed the media simply never produces frames and the card
//! shows no motion — nothing breaks.

use std::cell::{Cell, RefCell};
use std::path::PathBuf;
use std::rc::Rc;
use std::time::Duration;

use gtk4::prelude::*;
use gtk4::EventControllerMotion;

/// Grace period before a hover-leave hides the preview. Moving the pointer
/// across the card's revealed Edit button / overlays emits brief leave→enter
/// crossings; without this debounce the preview swaps back and forth (the glitch).
const LEAVE_GRACE: Duration = Duration::from_millis(140);

/// Attach hover-to-play to a card.
///
/// - `card`: the card root `gtk4::Overlay` (the hover target spanning the card).
///   Its base child must currently be `thumb`.
/// - `thumb`: the `gtk4::Picture` showing the static thumbnail. It stays the
///   card's size-driving base child forever; the video preview is layered above
///   it inside an inner `Overlay` so it can never trigger a relayout.
/// - `video`: the video/GIF file to preview.
///
/// Plays muted + looping while hovered. Degrades gracefully: if the media can't
/// be decoded (e.g. no GStreamer plugins installed) nothing bad happens — the
/// card simply shows no motion.
pub fn attach(card: &gtk4::Overlay, thumb: &gtk4::Picture, video: PathBuf) {
    // Re-parent the thumbnail into an inner overlay and stack the (initially
    // hidden) video layer above it. Only the thumbnail is measured.
    let inner = gtk4::Overlay::new();
    card.set_child(None::<&gtk4::Widget>);
    inner.set_child(Some(thumb));
    let video_pic = gtk4::Picture::new();
    video_pic.set_can_shrink(true);
    video_pic.set_keep_aspect_ratio(true);
    video_pic.set_can_target(false);
    video_pic.set_visible(false);
    inner.add_overlay(&video_pic);
    card.set_child(Some(&inner));

    // One lazily-created MediaFile per card, shared between enter/leave.
    let media: Rc<RefCell<Option<gtk4::MediaFile>>> = Rc::new(RefCell::new(None));
    // Whether the pointer is currently over the card. The leave handler defers to
    // this after the grace period, so transient crossings don't hide the preview.
    let hovered = Rc::new(Cell::new(false));
    // True once the MediaFile has produced its first frame. Showing the video
    // layer BEFORE that blanks the card — for however long the decoder takes,
    // or forever when the codec's GStreamer plugin is missing. The thumbnail
    // must stay visible until real frames exist.
    let ready = Rc::new(Cell::new(false));

    let controller = EventControllerMotion::new();

    // Enter: create the media if needed, start playing, and reveal it only once
    // (and as soon as) it has frames to show.
    let media_enter = media.clone();
    let video_enter = video_pic.clone();
    let hovered_enter = hovered.clone();
    let ready_enter = ready.clone();
    controller.connect_enter(move |_controller, _x, _y| {
        hovered_enter.set(true);
        let mut slot = media_enter.borrow_mut();
        if slot.is_none() {
            let m = gtk4::MediaFile::for_filename(video.to_string_lossy().as_ref());
            m.set_muted(true);
            m.set_loop(true);
            video_enter.set_paintable(Some(&m));
            // First decoded frame → reveal the live preview (if still hovered).
            let ready = ready_enter.clone();
            let video_pic = video_enter.clone();
            let hovered = hovered_enter.clone();
            m.connect_invalidate_contents(move |_| {
                if !ready.get() {
                    ready.set(true);
                    if hovered.get() {
                        video_pic.set_visible(true);
                    }
                }
            });
            *slot = Some(m);
        }
        let media = slot.as_ref().expect("just inserted");
        if ready_enter.get() {
            video_enter.set_visible(true);
        }
        media.play();
    });

    // Leave: after a short grace period (so a flicker across the Edit button /
    // overlays doesn't count), if the pointer really left, pause and hide the
    // preview again. Keep the MediaFile around for snappy re-hover.
    let media_leave = media;
    let video_leave = video_pic;
    controller.connect_leave(move |_controller| {
        hovered.set(false);
        let media_leave = media_leave.clone();
        let video_leave = video_leave.clone();
        let hovered = hovered.clone();
        gtk4::glib::timeout_add_local_once(LEAVE_GRACE, move || {
            if hovered.get() {
                return; // pointer came back within the grace period — keep playing
            }
            if let Some(media) = media_leave.borrow().as_ref() {
                media.pause();
            }
            video_leave.set_visible(false);
        });
    });

    card.add_controller(controller);
}
