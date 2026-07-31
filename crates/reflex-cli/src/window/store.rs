use super::{WindowData, WindowEvent, WindowHandle, WindowSnapshot};
use crate::lua::{ErrorKind, LuaError};
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::{Condvar, Mutex};
use std::time::{Duration, Instant};

#[derive(Default)]
struct StoreState {
    windows: HashMap<String, WindowHandle>,
    order: Vec<String>,
    events: VecDeque<WindowEvent>,
    generation: u64,
    ready: bool,
    error: Option<LuaError>,
}

#[derive(Default)]
pub(crate) struct WindowStore {
    state: Mutex<StoreState>,
    changed: Condvar,
}

impl WindowStore {
    pub(crate) fn upsert(&self, id: String, title: String, app_id: Option<String>) {
        let mut state = self.lock();
        if upsert_locked(&mut state, id, title, app_id) {
            self.bump(&mut state);
        }
    }

    pub(crate) fn close(&self, id: &str) {
        let mut state = self.lock();
        if close_locked(&mut state, id) {
            self.bump(&mut state);
        }
    }

    pub(crate) fn replace(&self, windows: Vec<WindowData>) {
        let mut state = self.lock();
        let present = windows
            .iter()
            .map(|window| window.id.clone())
            .collect::<HashSet<_>>();
        let mut changed = false;
        for window in windows {
            changed |= upsert_locked(&mut state, window.id, window.title, window.app_id);
        }

        let stale = state
            .order
            .iter()
            .filter(|id| !present.contains(*id))
            .cloned()
            .collect::<Vec<_>>();
        for id in stale {
            changed |= close_locked(&mut state, &id);
        }
        if changed {
            self.bump(&mut state);
        }
    }

    pub(crate) fn mark_ready(&self) {
        let mut state = self.lock();
        state.ready = true;
    }

    pub(crate) fn fail(&self, error: LuaError) {
        let mut state = self.lock();
        if state.error.is_none() {
            state.error = Some(error);
            self.bump(&mut state);
        }
    }

    pub(crate) fn snapshot(&self) -> Result<WindowSnapshot, LuaError> {
        let state = self.lock();
        check_error(&state)?;
        Ok(WindowSnapshot {
            windows: state
                .order
                .iter()
                .filter_map(|id| state.windows.get(id).cloned())
                .collect(),
            generation: state.generation,
        })
    }

    pub(crate) fn drain_events(&self) -> Result<Vec<WindowEvent>, LuaError> {
        let mut state = self.lock();
        check_error(&state)?;
        Ok(state.events.drain(..).collect())
    }

    pub(crate) fn wait_for_change(
        &self,
        generation: u64,
        timeout: Option<Duration>,
    ) -> Result<bool, LuaError> {
        let mut state = self.lock();
        check_error(&state)?;
        if state.generation != generation {
            return Ok(true);
        }

        match timeout {
            None => {
                while state.generation == generation && state.error.is_none() {
                    state = self
                        .changed
                        .wait(state)
                        .unwrap_or_else(|poisoned| poisoned.into_inner());
                }
            }
            Some(timeout) => {
                let Some(deadline) = Instant::now().checked_add(timeout) else {
                    return Err(LuaError::new(
                        ErrorKind::Runtime,
                        "window wait timeout is too large",
                    ));
                };
                while state.generation == generation && state.error.is_none() {
                    let remaining = deadline.saturating_duration_since(Instant::now());
                    if remaining.is_zero() {
                        return Ok(false);
                    }
                    let (next, result) = self
                        .changed
                        .wait_timeout(state, remaining)
                        .unwrap_or_else(|poisoned| poisoned.into_inner());
                    state = next;
                    if result.timed_out() && state.generation == generation {
                        return Ok(false);
                    }
                }
            }
        }

        check_error(&state)?;
        Ok(state.generation != generation)
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, StoreState> {
        self.state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn bump(&self, state: &mut StoreState) {
        state.generation = state.generation.wrapping_add(1);
        self.changed.notify_all();
    }
}

fn check_error(state: &StoreState) -> Result<(), LuaError> {
    match &state.error {
        Some(error) => Err(error.clone()),
        None => Ok(()),
    }
}

fn upsert_locked(
    state: &mut StoreState,
    id: String,
    title: String,
    app_id: Option<String>,
) -> bool {
    if let Some(window) = state.windows.get(&id).cloned() {
        let previous = window.update(title.clone(), app_id.clone());
        let changed = previous.title != title || previous.app_id != app_id || !previous.exists;
        if previous.title != title && state.ready {
            state
                .events
                .push_back(WindowEvent::TitleChanged { window, title });
        }
        return changed;
    }

    let window = WindowHandle::new(id.clone(), title, app_id);
    state.windows.insert(id.clone(), window.clone());
    state.order.push(id);
    if state.ready {
        state.events.push_back(WindowEvent::Opened(window));
    }
    true
}

fn close_locked(state: &mut StoreState, id: &str) -> bool {
    let Some(window) = state.windows.remove(id) else {
        return false;
    };
    state.order.retain(|candidate| candidate != id);
    window.close();
    if state.ready {
        state.events.push_back(WindowEvent::Closed(window));
    }
    true
}

pub(crate) fn backend_error(message: impl Into<String>) -> LuaError {
    LuaError::new(ErrorKind::Host, message)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn initial_windows_form_a_baseline_without_open_events() {
        let store = WindowStore::default();
        store.upsert("one".into(), "First".into(), Some("app.one".into()));
        store.mark_ready();

        let snapshot = store.snapshot().unwrap();
        assert_eq!(snapshot.windows.len(), 1);
        assert!(store.drain_events().unwrap().is_empty());
    }

    #[test]
    fn updates_live_handles_and_emits_lifecycle_events_once() {
        let store = WindowStore::default();
        store.mark_ready();
        store.upsert("one".into(), "First".into(), Some("app.one".into()));
        let window = store.snapshot().unwrap().windows[0].clone();
        store.upsert("one".into(), "Second".into(), Some("app.two".into()));
        store.close("one");
        store.close("one");

        assert_eq!(window.title(), "Second");
        assert_eq!(window.app_id().as_deref(), Some("app.two"));
        assert!(!window.exists());
        let events = store.drain_events().unwrap();
        assert_eq!(events.len(), 3);
        assert!(matches!(events[0], WindowEvent::Opened(_)));
        assert!(matches!(events[1], WindowEvent::TitleChanged { .. }));
        assert!(matches!(events[2], WindowEvent::Closed(_)));
    }

    #[test]
    fn replace_diffs_open_updates_and_close() {
        let store = WindowStore::default();
        store.upsert("old".into(), "Old".into(), None);
        store.mark_ready();
        let before = store.snapshot().unwrap().generation;
        store.replace(vec![WindowData {
            id: "new".into(),
            title: "New".into(),
            app_id: Some("new.app".into()),
            exists: true,
        }]);

        let events = store.drain_events().unwrap();
        assert_eq!(events.len(), 2);
        assert!(matches!(events[0], WindowEvent::Opened(_)));
        assert!(matches!(events[1], WindowEvent::Closed(_)));
        assert_eq!(store.snapshot().unwrap().generation, before + 1);
    }

    #[test]
    fn wait_for_change_wakes_for_backend_updates_and_times_out_cleanly() {
        let store = Arc::new(WindowStore::default());
        store.mark_ready();
        let generation = store.snapshot().unwrap().generation;
        let worker_store = store.clone();
        let worker = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(10));
            worker_store.upsert("new".into(), "New".into(), None);
        });

        assert!(
            store
                .wait_for_change(generation, Some(Duration::from_secs(1)))
                .unwrap()
        );
        worker.join().unwrap();
        let generation = store.snapshot().unwrap().generation;
        assert!(
            !store
                .wait_for_change(generation, Some(Duration::from_millis(1)))
                .unwrap()
        );
    }
}
