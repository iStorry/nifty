//! Fine-grained reactivity: a [`Signal`] re-runs exactly the [`Effect`]s that read it.

use std::cell::RefCell;
use std::fmt;
use std::rc::{Rc, Weak};

thread_local! {
    static OBSERVER: RefCell<Option<Weak<EffectNode>>> = const { RefCell::new(None) };
}

struct EffectNode {
    run: Box<dyn Fn()>,
}

impl EffectNode {
    fn execute(self: &Rc<Self>) {
        let _scope = ObserverScope::enter(Rc::downgrade(self));
        (self.run)();
    }
}

/// Restores the previous observer on drop, so nested and panicking effects leave tracking intact.
struct ObserverScope {
    previous: Option<Weak<EffectNode>>,
}

impl ObserverScope {
    fn enter(observer: Weak<EffectNode>) -> Self {
        Self {
            previous: OBSERVER.replace(Some(observer)),
        }
    }
}

impl Drop for ObserverScope {
    fn drop(&mut self) {
        OBSERVER.set(self.previous.take());
    }
}

/// A computation that runs once immediately and again whenever a signal it read changes.
#[must_use = "an effect stops re-running once it is dropped"]
pub struct Effect {
    _node: Rc<EffectNode>,
}

impl Effect {
    pub fn new(run: impl Fn() + 'static) -> Self {
        let node = Rc::new(EffectNode { run: Box::new(run) });
        node.execute();
        Self { _node: node }
    }
}

impl fmt::Debug for Effect {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Effect").finish_non_exhaustive()
    }
}

struct SignalState<T> {
    value: RefCell<T>,
    subscribers: RefCell<Vec<Weak<EffectNode>>>,
}

/// A reactive value. Reading it inside an [`Effect`] subscribes that effect to changes.
pub struct Signal<T>(Rc<SignalState<T>>);

impl<T> Signal<T> {
    pub fn new(value: T) -> Self {
        Self(Rc::new(SignalState {
            value: RefCell::new(value),
            subscribers: RefCell::new(Vec::new()),
        }))
    }

    pub fn with<R>(&self, read: impl FnOnce(&T) -> R) -> R {
        self.track();
        read(&self.0.value.borrow())
    }

    pub fn set(&self, value: T) {
        *self.0.value.borrow_mut() = value;
        self.notify();
    }

    pub fn update(&self, change: impl FnOnce(&mut T)) {
        change(&mut self.0.value.borrow_mut());
        self.notify();
    }

    fn track(&self) {
        OBSERVER.with_borrow(|observer| {
            let Some(observer) = observer else { return };
            let mut subscribers = self.0.subscribers.borrow_mut();
            if !subscribers.iter().any(|existing| existing.ptr_eq(observer)) {
                subscribers.push(observer.clone());
            }
        });
    }

    /// Subscribers are cleared before re-running so each effect re-subscribes to what it reads now.
    fn notify(&self) {
        let subscribers = self.0.subscribers.take();
        for effect in subscribers.iter().filter_map(Weak::upgrade) {
            effect.execute();
        }
    }
}

impl<T: Clone> Signal<T> {
    pub fn get(&self) -> T {
        self.with(T::clone)
    }
}

impl<T> Clone for Signal<T> {
    fn clone(&self) -> Self {
        Self(Rc::clone(&self.0))
    }
}

impl<T: fmt::Debug> fmt::Debug for Signal<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("Signal")
            .field(&self.0.value.borrow())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    fn counting_effect(signal: &Signal<i32>, runs: &Rc<Cell<u32>>) -> Effect {
        let signal = signal.clone();
        let runs = Rc::clone(runs);
        Effect::new(move || {
            signal.with(|_| ());
            runs.set(runs.get() + 1);
        })
    }

    #[test]
    fn effect_runs_immediately_and_on_every_change() {
        let signal = Signal::new(0);
        let runs = Rc::new(Cell::new(0));
        let _effect = counting_effect(&signal, &runs);
        assert_eq!(runs.get(), 1);

        signal.set(1);
        signal.update(|value| *value += 1);
        assert_eq!(runs.get(), 3);
        assert_eq!(signal.get(), 2);
    }

    #[test]
    fn repeated_reads_subscribe_once() {
        let signal = Signal::new(0);
        let runs = Rc::new(Cell::new(0));
        let _effect = Effect::new({
            let signal = signal.clone();
            let runs = Rc::clone(&runs);
            move || {
                signal.get();
                signal.get();
                runs.set(runs.get() + 1);
            }
        });

        signal.set(1);
        assert_eq!(runs.get(), 2);
    }

    #[test]
    fn dropped_effect_stops_running() {
        let signal = Signal::new(0);
        let runs = Rc::new(Cell::new(0));
        drop(counting_effect(&signal, &runs));

        signal.set(1);
        assert_eq!(runs.get(), 1);
    }

    #[test]
    fn reads_outside_effects_are_not_tracked() {
        let signal = Signal::new(0);
        signal.get();
        assert!(signal.0.subscribers.borrow().is_empty());
    }
}
