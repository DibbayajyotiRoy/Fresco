//! Reusable "hover to play" preview behaviour for video/GIF library cards.
//!
//! While the pointer is over a card we want it to come alive: the static
//! thumbnail is swapped for a muted, looping inline video preview, and on
//! leave the thumbnail is restored.
//!
//! The trick is to swap the *base child's paintable* rather than juggling
//! widgets. Each card is a [`gtk4::Overlay`] whose base child is a
//! [`gtk4::Picture`] showing the thumbnail; the card's scrim, badges and edit
//! controls live as overlays stacked on top. By only changing the base
//! `Picture`'s paintable (thumbnail texture <-> `MediaFile`) those overlays
//! stay in place and untouched — no relayout, no flicker.
//!
//! A [`gtk4::MediaFile`] is created lazily on first hover (so we never decode
//! every card up front) and kept alive afterwards for snappy re-hover. It is
//! configured muted + looping. Decoding is best-effort: if no GStreamer
//! plugins are installed the media simply never produces frames and the card
//! shows no motion — nothing breaks.

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

use gtk4::prelude::*;
use gtk4::EventControllerMotion;

/// Attach hover-to-play to a card.
///
/// - `card`: the card root `gtk4::Overlay` (the hover target spanning the card).
/// - `thumb`: the `gtk4::Picture` that currently shows the static thumbnail
///   (this is the card's base child; swapping its paintable keeps the card's
///   scrim/badge/edit overlays on top, untouched).
/// - `video`: the video/GIF file to preview.
/// - `thumb_file`: the static thumbnail file to restore on leave (None if the
///   card had no thumbnail, in which case clear the paintable on leave).
///
/// Plays muted + looping while hovered. Degrades gracefully: if the media can't
/// be decoded (e.g. no GStreamer plugins installed) nothing bad happens — the
/// card simply shows no motion.
pub fn attach(
    card: &gtk4::Overlay,
    thumb: &gtk4::Picture,
    video: PathBuf,
    thumb_file: Option<PathBuf>,
) {
    // One lazily-created MediaFile per card, shared between enter/leave.
    let media: Rc<RefCell<Option<gtk4::MediaFile>>> = Rc::new(RefCell::new(None));

    let controller = EventControllerMotion::new();

    // Enter: create the media if needed, show it, and start playing.
    let media_enter = media.clone();
    let thumb_enter = thumb.clone();
    controller.connect_enter(move |_controller, _x, _y| {
        let mut slot = media_enter.borrow_mut();
        let media = slot.get_or_insert_with(|| {
            let media = gtk4::MediaFile::for_filename(video.to_string_lossy().as_ref());
            media.set_muted(true);
            media.set_loop(true);
            media
        });
        thumb_enter.set_paintable(Some(media));
        media.play();
    });

    // Leave: pause and restore the static thumbnail. Keep the MediaFile around.
    let media_leave = media;
    let thumb_leave = thumb.clone();
    controller.connect_leave(move |_controller| {
        if let Some(media) = media_leave.borrow().as_ref() {
            media.pause();
        }
        match &thumb_file {
            Some(path) => thumb_leave.set_file(Some(&gtk4::gio::File::for_path(path))),
            None => thumb_leave.set_paintable(gtk4::gdk::Paintable::NONE),
        }
    });

    card.add_controller(controller);
}
