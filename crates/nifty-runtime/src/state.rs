//! Which app state a part of the UI reads, so it can be shown again when that state changes.

/// One `state` declaration of the app, numbered in the order they are declared.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct StateId(pub usize);

/// States read by a dynamic part of the UI, sorted and without duplicates.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct StateSet(Vec<StateId>);

impl StateSet {
    pub fn insert(&mut self, state: StateId) {
        if let Err(position) = self.0.binary_search(&state) {
            self.0.insert(position, state);
        }
    }

    pub fn extend(&mut self, other: &Self) {
        for state in &other.0 {
            self.insert(*state);
        }
    }

    #[must_use]
    pub fn contains(&self, state: StateId) -> bool {
        self.0.binary_search(&state).is_ok()
    }

    #[must_use]
    pub fn intersects(&self, other: &Self) -> bool {
        self.0.iter().any(|state| other.contains(*state))
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = StateId> + '_ {
        self.0.iter().copied()
    }
}
