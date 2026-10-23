//! Windowed card grid: a `FlowBox` of fixed-size slots whose cards exist only
//! while their slot is near the viewport.
//!
//! A `FlowBox` is not virtualised. With every card built up front, a
//! 4,000-wallpaper library meant ~100,000 live widgets and 4,000 decoded
//! thumbnails, rebuilt in full on every refresh, and the window hung for as
//! long as that took. Here each wallpaper costs one empty, correctly sized
//! slot; the card is built when its slot comes within half a viewport of the
//! visible area and dropped again once it is more than two viewports away, so
//! the live widget count follows the window height, not the library size.
//! (A `GtkGridView` would virtualise properly, but the library is a stack of
//! labelled sections inside one page-level scroller, and a `GridView` only
//! recycles rows when it owns its scroller.)

use std::cell::{Cell, RefCell};
use std::collections::BTreeSet;
use std::ops::Range;
use std::rc::{Rc, Weak};
use std::time::Duration;

use gtk4::glib::{self, SignalHandlerId};
use gtk4::prelude::*;

/// Card footprint: a 16:9 poster, never smaller than this many pixels.
const SLOT_MIN: (i32, i32) = (260, 146);

/// How many times (30 ms apart) a pass that found no layout yet is retried.
const RETRIES: u8 = 40;

// ─── Pure logic ──────────────────────────────────────────────────────────────

/// Indices of the `n` items whose vertical span `[y, y + item_h]` overlaps
/// `[top, bottom]`, given that `y_of` never decreases with the index (a grid
/// fills row by row). Binary search: `O(log n)` calls to `y_of`.
pub fn visible_range(
    n: usize,
    y_of: impl Fn(usize) -> f64,
    item_h: f64,
    top: f64,
    bottom: f64,
) -> Range<usize> {
    let first = partition(n, |i| y_of(i) + item_h < top);
    let end = partition(n, |i| y_of(i) <= bottom);
    first..end.max(first)
}

/// First index in `0..n` for which `before` is false; `before` must be true
/// for a prefix and false after it.
fn partition(n: usize, before: impl Fn(usize) -> bool) -> usize {
    let (mut lo, mut hi) = (0, n);
    while lo < hi {
        let mid = lo + (hi - lo) / 2;
        if before(mid) {
            lo = mid + 1;
        } else {
            hi = mid;
        }
    }
    lo
}

/// What one pass over the grid has to do.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Pass {
    /// Slots that are in range and empty: build their card.
    pub build: Vec<usize>,
    /// Slots holding a card that has drifted out of range: drop it.
    pub drop: Vec<usize>,
}

/// Cards are built within half a viewport of the visible area and dropped
/// beyond two viewports of it. The gap between the two is hysteresis: a card
/// near the edge is not built and thrown away again on every wheel notch.
pub fn plan_pass(
    n: usize,
    y_of: impl Fn(usize) -> f64,
    item_h: f64,
    top: f64,
    bottom: f64,
    built: &BTreeSet<usize>,
) -> Pass {
    if n == 0 || item_h <= 0.0 || bottom <= top {
        return Pass::default();
    }
    let view = bottom - top;
    let want = visible_range(n, &y_of, item_h, top - view / 2.0, bottom + view / 2.0);
    let keep = visible_range(n, &y_of, item_h, top - 2.0 * view, bottom + 2.0 * view);
    Pass {
        build: want.filter(|i| !built.contains(i)).collect(),
        drop: built
            .iter()
            .copied()
            .filter(|i| !keep.contains(i))
            .collect(),
    }
}

// ─── GTK glue ────────────────────────────────────────────────────────────────

/// An empty card-sized slot.
fn new_slot() -> gtk4::AspectFrame {
    let slot = gtk4::AspectFrame::new(0.5, 0.0, 16.0 / 9.0, false);
    slot.set_size_request(SLOT_MIN.0, SLOT_MIN.1);
    slot.add_css_class("wp-slot");
    slot
}

struct Grid {
    flow: glib::WeakRef<gtk4::FlowBox>,
    slots: Vec<gtk4::AspectFrame>,
    built: RefCell<BTreeSet<usize>>,
    build: Box<dyn Fn(usize) -> gtk4::Widget>,
    /// The scroller's adjustment while this grid is on screen, with the two
    /// handlers to take off it again.
    hooked: RefCell<Option<(gtk4::Adjustment, [SignalHandlerId; 2])>>,
    /// An idle pass is already queued.
    queued: Cell<bool>,
    /// Layout-not-ready retries left.
    retries: Cell<u8>,
}

/// Append `n` slots to `flow` and keep their cards (made by `build(i)`) in
/// step with the scroll position.
pub fn fill(flow: &gtk4::FlowBox, n: usize, build: impl Fn(usize) -> gtk4::Widget + 'static) {
    let slots: Vec<_> = (0..n).map(|_| new_slot()).collect();
    for s in &slots {
        flow.append(s);
    }
    let grid = Rc::new(Grid {
        flow: flow.downgrade(),
        slots,
        built: RefCell::new(BTreeSet::new()),
        build: Box::new(build),
        hooked: RefCell::new(None),
        queued: Cell::new(false),
        retries: Cell::new(RETRIES),
    });
    {
        let grid = grid.clone();
        flow.connect_map(move |flow| grid.hook(flow, &grid));
    }
    {
        let grid = grid.clone();
        flow.connect_unmap(move |_| grid.unhook());
    }
}

impl Grid {
    fn hook(&self, flow: &gtk4::FlowBox, this: &Rc<Grid>) {
        self.unhook();
        let Some(sw) = flow
            .ancestor(gtk4::ScrolledWindow::static_type())
            .and_downcast::<gtk4::ScrolledWindow>()
        else {
            return;
        };
        let adj = sw.vadjustment();
        let on_scroll = Rc::downgrade(this);
        let a = adj.connect_value_changed(move |_| {
            if let Some(g) = on_scroll.upgrade() {
                g.pass();
            }
        });
        // `changed` fires from inside the scroller's own allocation, where
        // adding widgets is a bad idea: defer to idle.
        let on_layout = Rc::downgrade(this);
        let b = adj.connect_changed(move |_| {
            if let Some(g) = on_layout.upgrade() {
                g.queue(&g);
            }
        });
        *self.hooked.borrow_mut() = Some((adj, [a, b]));
        self.queue(this);
    }

    fn unhook(&self) {
        if let Some((adj, ids)) = self.hooked.borrow_mut().take() {
            for id in ids {
                adj.disconnect(id);
            }
        }
    }

    /// Run a pass once the current layout has settled.
    fn queue(&self, this: &Rc<Grid>) {
        if self.queued.replace(true) {
            return;
        }
        let weak: Weak<Grid> = Rc::downgrade(this);
        glib::idle_add_local_full(glib::Priority::DEFAULT_IDLE, move || {
            if let Some(g) = weak.upgrade() {
                g.queued.set(false);
                if g.pass() {
                    g.retries.set(RETRIES);
                } else if g.retries.get() > 0 {
                    // Not laid out yet (first map, or mid-resize): look again shortly.
                    g.retries.set(g.retries.get() - 1);
                    let weak = Rc::downgrade(&g);
                    glib::timeout_add_local_once(Duration::from_millis(30), move || {
                        if let Some(g) = weak.upgrade() {
                            g.queue(&g);
                        }
                    });
                }
            }
            glib::ControlFlow::Break
        });
    }

    /// One build/drop sweep against the current viewport. `false` when the
    /// layout is not ready to be measured.
    fn pass(&self) -> bool {
        let Some(flow) = self.flow.upgrade() else {
            return true;
        };
        let Some(sw) = flow
            .ancestor(gtk4::ScrolledWindow::static_type())
            .and_downcast::<gtk4::ScrolledWindow>()
        else {
            return true;
        };
        // Where the viewport sits is read from the adjustment, which moves the
        // instant the user scrolls, and the grid's place in the scrolled
        // content, which only layout changes. Asking the widget tree instead
        // (`flow.compute_bounds(&sw)`) lags a frame behind the scroll, so the
        // last position the user stopped on would be left unbuilt.
        let adj = sw.vadjustment();
        let content = sw
            .child()
            .and_downcast::<gtk4::Viewport>()
            .and_then(|vp| vp.child());
        let item_h = self.slots.first().map_or(0, |s| s.height()) as f64;
        let Some(origin) = content.and_then(|c| flow.compute_bounds(&c)) else {
            return false;
        };
        if item_h <= 0.0 || adj.page_size() <= 0.0 {
            return false;
        }
        let top = adj.value() - origin.y() as f64;
        let bottom = top + adj.page_size();
        let y_of = |i: usize| {
            self.slots[i]
                .compute_bounds(&flow)
                .map_or(f64::MAX, |r| r.y() as f64)
        };
        let plan = plan_pass(
            self.slots.len(),
            y_of,
            item_h,
            top,
            bottom,
            &self.built.borrow(),
        );
        // The card that has keyboard focus (an inline rename, say) stays.
        let focus = flow.root().and_then(|r| gtk4::prelude::RootExt::focus(&r));
        for i in plan.drop {
            let slot = &self.slots[i];
            if focus.as_ref().is_some_and(|f| f.is_ancestor(slot)) {
                continue;
            }
            slot.set_child(None::<&gtk4::Widget>);
            slot.add_css_class("wp-slot");
            self.built.borrow_mut().remove(&i);
        }
        for i in plan.build {
            let slot = &self.slots[i];
            slot.set_child(Some(&(self.build)(i)));
            slot.remove_css_class("wp-slot");
            self.built.borrow_mut().insert(i);
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `cols` per row, `row_h` tall rows (item 100 + 12 gap).
    fn row_y(cols: usize, row_h: f64) -> impl Fn(usize) -> f64 {
        move |i| (i / cols) as f64 * row_h
    }

    #[test]
    fn visible_range_covers_exactly_the_overlapping_rows() {
        let y = row_y(4, 112.0);
        // Viewport 0..300 on 100px items: rows 0, 1, 2 (y = 0, 112, 224).
        assert_eq!(visible_range(100, &y, 100.0, 0.0, 300.0), 0..12);
        // Scrolled so row 3 (y 336..436) is the first overlapping row.
        assert_eq!(visible_range(100, &y, 100.0, 400.0, 600.0), 12..24);
    }

    #[test]
    fn visible_range_clamps_to_the_ends() {
        let y = row_y(4, 112.0);
        assert_eq!(visible_range(10, &y, 100.0, -5000.0, -4000.0), 0..0);
        assert_eq!(visible_range(10, &y, 100.0, 9000.0, 9500.0), 10..10);
        assert_eq!(visible_range(0, &y, 100.0, 0.0, 500.0), 0..0);
        assert_eq!(visible_range(10, &y, 100.0, -1e9, 1e9), 0..10);
    }

    #[test]
    fn plan_builds_the_visible_band_and_half_a_view_each_side() {
        let y = row_y(4, 112.0);
        let p = plan_pass(1000, &y, 100.0, 1120.0, 1520.0, &BTreeSet::new());
        // View 400px tall: build 920..1720 -> rows 8 (y 896, ends 996 >= 920) .. 15.
        assert_eq!(p.build.first(), Some(&32));
        assert_eq!(p.build.last(), Some(&63));
        assert!(p.drop.is_empty());
    }

    #[test]
    fn plan_skips_built_slots_and_drops_far_ones() {
        let y = row_y(4, 112.0);
        let built: BTreeSet<usize> = [0, 1, 2, 3, 40, 41].into();
        let p = plan_pass(1000, &y, 100.0, 1120.0, 1520.0, &built);
        // 0..4 sit 1000px above the top: beyond two views (800px) -> dropped.
        assert_eq!(p.drop, vec![0, 1, 2, 3]);
        assert!(!p.build.contains(&40) && !p.build.contains(&41));
        assert!(p.build.contains(&42));
    }

    #[test]
    fn plan_keeps_cards_inside_the_hysteresis_band() {
        let y = row_y(4, 112.0);
        // Row 4 (y 448) is 672px above the top of 1120: past the build margin
        // (200px) but inside the keep margin (800px).
        let built: BTreeSet<usize> = [16].into();
        let p = plan_pass(1000, &y, 100.0, 1120.0, 1520.0, &built);
        assert!(p.drop.is_empty());
        assert!(!p.build.contains(&16));
    }

    #[test]
    fn plan_waits_for_a_measured_layout() {
        let y = row_y(4, 112.0);
        let none = BTreeSet::new();
        assert_eq!(plan_pass(10, &y, 0.0, 0.0, 400.0, &none), Pass::default());
        assert_eq!(plan_pass(10, &y, 100.0, 0.0, 0.0, &none), Pass::default());
        assert_eq!(plan_pass(0, &y, 100.0, 0.0, 400.0, &none), Pass::default());
    }

    #[test]
    fn live_cards_track_the_window_not_the_library() {
        let y = row_y(5, 158.0);
        let mut built = BTreeSet::new();
        let mut peak = 0;
        // Scroll a 4,000-card library from top to bottom in 700px steps.
        let mut top = 0.0;
        while top < 800.0 * 158.0 {
            let p = plan_pass(4000, &y, 146.0, top, top + 700.0, &built);
            for i in p.drop {
                built.remove(&i);
            }
            built.extend(p.build);
            peak = peak.max(built.len());
            top += 700.0;
        }
        assert!(peak < 150, "peak {peak} live cards for 4000 entries");
    }
}
