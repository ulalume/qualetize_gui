//! Undo / redo history over [`SettingsBundle`]: engine, Qualetize settings,
//! tilepalquant settings, color correction and palette sort. Nothing else
//! (image loading, zoom, panels) is undoable.

use crate::settings_manager::SettingsBundle;
use crate::time::Instant;
use std::time::Duration;

/// Maximum number of steps kept on the undo stack.
pub const CAP: usize = 100;

/// How long a change has to sit unchanged before it is committed as its own
/// undo step. This coalesces a slider drag (many changes in quick
/// succession) into a single step instead of one per frame.
pub const SETTLE: Duration = Duration::from_millis(600);

/// Undo / redo history over the app's settings.
///
/// Callers report the live settings every frame through [`observe`]; a
/// change is only pushed onto the undo stack once it has stopped changing
/// for [`SETTLE`]. [`undo`] and [`redo`] commit an unsettled change first,
/// so undoing right after a change reverts that change rather than the one
/// before it.
///
/// [`observe`]: SettingsHistory::observe
/// [`undo`]: SettingsHistory::undo
/// [`redo`]: SettingsHistory::redo
pub struct SettingsHistory {
    undo: Vec<SettingsBundle>,
    redo: Vec<SettingsBundle>,
    /// The most recently committed bundle: the baseline the next change is
    /// compared against.
    committed: SettingsBundle,
    /// The uncommitted settings last seen and when they last changed; the
    /// settle timer restarts on every change.
    pending: Option<(SettingsBundle, Instant)>,
}

impl SettingsHistory {
    pub fn new(initial: SettingsBundle) -> Self {
        Self {
            undo: Vec::new(),
            redo: Vec::new(),
            committed: initial,
            pending: None,
        }
    }

    /// Push `committed` onto `undo`, dropping the oldest step above [`CAP`],
    /// then adopt `new_committed` as the committed bundle and clear `redo`.
    fn commit(&mut self, new_committed: SettingsBundle) {
        self.undo
            .push(std::mem::replace(&mut self.committed, new_committed));
        if self.undo.len() > CAP {
            self.undo.remove(0);
        }
        self.redo.clear();
        self.pending = None;
    }

    /// Report the live settings for this frame. Returns `true` when a step
    /// was just recorded.
    ///
    /// A change is committed once it has stayed the same for [`SETTLE`];
    /// every further change restarts that timer, so a slider drag becomes
    /// one undo step however long it lasts. `hold` postpones the commit
    /// without restarting the timer, for as long as a pointer button is
    /// down.
    pub fn observe(&mut self, current: &SettingsBundle, now: Instant, hold: bool) -> bool {
        if *current == self.committed {
            self.pending = None;
            return false;
        }

        match &self.pending {
            Some((seen, since)) if seen == current => {
                if !hold && now.duration_since(*since) >= SETTLE {
                    self.commit(current.clone());
                    return true;
                }
            }
            _ => self.pending = Some((current.clone(), now)),
        }
        false
    }

    /// A change is waiting to settle. The caller uses this to request a
    /// repaint after [`SETTLE`] so the step is recorded even without
    /// further input.
    pub fn pending(&self) -> bool {
        self.pending.is_some()
    }

    /// Undo the last committed step, returning the bundle to restore.
    ///
    /// An unsettled change in `current` is committed first, so calling this
    /// right after a change reverts that change.
    pub fn undo(&mut self, current: &SettingsBundle) -> Option<SettingsBundle> {
        if *current != self.committed {
            self.commit(current.clone());
        }
        let previous = self.undo.pop()?;
        self.redo
            .push(std::mem::replace(&mut self.committed, previous.clone()));
        self.pending = None;
        Some(previous)
    }

    /// Redo the last undone step, returning the bundle to restore.
    ///
    /// An unsettled change in `current` is committed first; since committing
    /// clears `redo`, redoing right after an unsettled change returns `None`.
    pub fn redo(&mut self, current: &SettingsBundle) -> Option<SettingsBundle> {
        if *current != self.committed {
            self.commit(current.clone());
        }
        let next = self.redo.pop()?;
        self.undo
            .push(std::mem::replace(&mut self.committed, next.clone()));
        self.pending = None;
        Some(next)
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{
        QualetizeSettings, color_correction::ColorCorrection, image::PaletteSortSettings,
    };

    fn bundle(tile_width: u16) -> SettingsBundle {
        let settings = QualetizeSettings {
            tile_width,
            ..Default::default()
        };
        SettingsBundle::new(
            settings,
            ColorCorrection::default(),
            PaletteSortSettings::default(),
        )
    }

    #[test]
    fn change_settles_after_delay_and_not_before() {
        let start = bundle(8);
        let mut history = SettingsHistory::new(start.clone());
        let now = Instant::now();
        let changed = bundle(16);

        assert!(!history.observe(&changed, now, false));
        assert!(history.pending());
        assert!(!history.observe(&changed, now + SETTLE - Duration::from_millis(1), false));
        assert!(history.observe(&changed, now + SETTLE, false));
        assert!(!history.pending());
        assert!(history.can_undo());
    }

    #[test]
    fn two_changes_within_settle_become_one_step() {
        let start = bundle(8);
        let mut history = SettingsHistory::new(start.clone());
        let now = Instant::now();

        assert!(!history.observe(&bundle(16), now, false));
        assert!(!history.observe(&bundle(24), now + Duration::from_millis(100), false));
        assert!(history.observe(
            &bundle(24),
            now + Duration::from_millis(100) + SETTLE,
            false
        ));

        // Only one step: undoing once returns to the original.
        let restored = history.undo(&bundle(24)).expect("a step was recorded");
        assert_eq!(restored, start);
        assert!(!history.can_undo());
    }

    #[test]
    fn a_change_that_keeps_moving_does_not_settle() {
        let mut history = SettingsHistory::new(bundle(8));
        let now = Instant::now();
        let step = Duration::from_millis(100);

        // Ten values in a row, each less than SETTLE after the previous one.
        for i in 1..=10u32 {
            assert!(!history.observe(&bundle(8 + i as u16), now + step * i, false));
        }
        // Held down (dragging) it stays pending past SETTLE.
        assert!(!history.observe(&bundle(18), now + step * 10 + SETTLE, true));
        // Released and unchanged for SETTLE: exactly one step.
        assert!(history.observe(&bundle(18), now + step * 10 + SETTLE, false));
        let restored = history.undo(&bundle(18)).expect("a step was recorded");
        assert_eq!(restored, bundle(8));
        assert!(!history.can_undo());
    }

    #[test]
    fn undo_then_redo_round_trips() {
        let start = bundle(8);
        let mut history = SettingsHistory::new(start.clone());
        let changed = bundle(16);
        let now = Instant::now();
        history.observe(&changed, now, false);
        history.observe(&changed, now + SETTLE, false);

        let undone = history.undo(&changed).expect("a step to undo");
        assert_eq!(undone, start);

        let redone = history.redo(&start).expect("a step to redo");
        assert_eq!(redone, changed);
    }

    #[test]
    fn new_change_after_undo_clears_redo() {
        let start = bundle(8);
        let mut history = SettingsHistory::new(start.clone());
        let changed = bundle(16);
        let now = Instant::now();
        history.observe(&changed, now, false);
        history.observe(&changed, now + SETTLE, false);
        history.undo(&changed);
        assert!(history.can_redo());

        let other = bundle(24);
        history.observe(&other, now + SETTLE + Duration::from_millis(1), false);
        history.observe(&other, now + 2 * SETTLE + Duration::from_millis(1), false);

        assert!(!history.can_redo());
    }

    #[test]
    fn undo_right_after_unsettled_change_reverts_it() {
        let start = bundle(8);
        let mut history = SettingsHistory::new(start.clone());
        let changed = bundle(16);
        let now = Instant::now();

        // Change is observed but has not settled yet.
        assert!(!history.observe(&changed, now, false));

        let undone = history.undo(&changed).expect("the pending change to undo");
        assert_eq!(undone, start);
    }

    #[test]
    fn redo_right_after_unsettled_change_returns_none() {
        let start = bundle(8);
        let mut history = SettingsHistory::new(start.clone());
        let changed = bundle(16);
        let now = Instant::now();
        history.observe(&changed, now, false);
        history.observe(&changed, now + SETTLE, false);
        history.undo(&changed);

        // A new, unsettled change is committed first, which empties `redo`.
        let other = bundle(24);
        assert!(history.redo(&other).is_none());
        assert!(!history.can_redo());
    }

    #[test]
    fn cap_drops_the_oldest_step() {
        let start = bundle(0);
        let mut history = SettingsHistory::new(start.clone());
        let mut now = Instant::now();

        for i in 1..=(CAP + 1) as u16 {
            let step = bundle(i);
            history.observe(&step, now, false);
            now += SETTLE;
            history.observe(&step, now, false);
        }

        assert_eq!(history.undo.len(), CAP);
        // The oldest step (the original `start`) was dropped, so undoing all
        // the way back stops at `bundle(1)`, not `start`.
        for _ in 0..CAP {
            history.undo(&history.committed.clone());
        }
        assert!(!history.can_undo());
        assert_eq!(history.committed, bundle(1));
    }
}
