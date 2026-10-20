//! Keeping a download's bindings on its item when the list holding it changes.
//!
//! A binding names a list item by its position, and positions move when items before it
//! are removed or the list is rebuilt. A download bound with `id:` writes its id into the
//! item, so before each report the app looks for the item that holds the id, and moves
//! each binding that shares the item's path with it to wherever that item is now.

use nifty_runtime::{DownloadField, Step};

/// A place in an app's state, as its runner names one.
pub trait StatePlace: Clone + PartialEq {
    /// The fields and items from its state down to it.
    fn steps(&self) -> &[Step];
    /// The same state, with `steps` instead.
    #[must_use]
    fn with_steps(&self, steps: Vec<Step>) -> Self;
    /// Whether `other` is in the same state.
    fn same_root(&self, other: &Self) -> bool;
}

/// Reading an app's state, for finding where a download's id went.
pub trait StateReader {
    type Place: StatePlace;
    /// How many items the list at `place` has; `None` when there's no such place.
    fn list_len(&self, place: &Self::Place) -> Option<usize>;
    /// Whether the text at `place` is `text`; `false` when there's no such place.
    fn holds_text(&self, place: &Self::Place, text: &str) -> bool;
}

/// Moves `bindings` to the item that holds `id` now. `false` when no item holds it any
/// more, so nothing shows the download. Bindings without an `id:` stay where they are.
pub fn follow_id<R: StateReader>(
    reader: &R,
    bindings: &mut [(DownloadField, R::Place)],
    id: &str,
) -> bool {
    let Some(at) = bindings
        .iter()
        .find(|(field, _)| *field == DownloadField::Id)
        .map(|(_, place)| place.clone())
    else {
        return true;
    };
    let Some(now) = find(reader, &at, id) else {
        return false;
    };
    if now != at {
        for (_, place) in bindings.iter_mut() {
            *place = moved(place, &at, &now);
        }
    }
    true
}

/// Where the text `id` is now, among the places shaped like `at`: `at` itself while it
/// still holds it, or else the first such place that does, trying every item of each list.
fn find<R: StateReader>(reader: &R, at: &R::Place, id: &str) -> Option<R::Place> {
    if reader.holds_text(at, id) {
        return Some(at.clone());
    }
    search(reader, at, &mut Vec::new(), at.steps(), id)
}

fn search<R: StateReader>(
    reader: &R,
    at: &R::Place,
    walked: &mut Vec<Step>,
    rest: &[Step],
    id: &str,
) -> Option<R::Place> {
    let Some((step, rest)) = rest.split_first() else {
        let place = at.with_steps(walked.clone());
        return reader.holds_text(&place, id).then_some(place);
    };
    let choices = match *step {
        Step::Field(_) => vec![*step],
        Step::Index(_) => {
            let len = reader.list_len(&at.with_steps(walked.clone()))?;
            (0..len).map(Step::Index).collect()
        }
    };
    choices.into_iter().find_map(|choice| {
        walked.push(choice);
        let found = search(reader, at, walked, rest, id);
        walked.pop();
        found
    })
}

/// `place`, moved the way the id moved from `from` to `to`: the steps it shares with
/// `from` follow `to`, and the rest stay. A place in another state shares no steps, since
/// `from` and `to` only differ in their list positions.
fn moved<P: StatePlace>(place: &P, from: &P, to: &P) -> P {
    if !place.same_root(from) {
        return place.clone();
    }
    let shared = place
        .steps()
        .iter()
        .zip(from.steps())
        .take_while(|(a, b)| a == b)
        .count();
    let mut steps = to.steps()[..shared].to_vec();
    steps.extend_from_slice(&place.steps()[shared..]);
    place.with_steps(steps)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A place in a test's state: which state, and the steps into it.
    #[derive(Clone, Debug, PartialEq)]
    struct Place(usize, Vec<Step>);

    impl StatePlace for Place {
        fn steps(&self) -> &[Step] {
            &self.1
        }

        fn with_steps(&self, steps: Vec<Step>) -> Self {
            Self(self.0, steps)
        }

        fn same_root(&self, other: &Self) -> bool {
            self.0 == other.0
        }
    }

    /// State 0 is a list of rows, each with its id in field 0 and its percentage in field
    /// 1; state 1 is a separate number.
    struct Rows(Vec<&'static str>);

    impl StateReader for Rows {
        type Place = Place;

        fn list_len(&self, place: &Place) -> Option<usize> {
            (place.0 == 0 && place.1.is_empty()).then_some(self.0.len())
        }

        fn holds_text(&self, place: &Place, text: &str) -> bool {
            match place.1[..] {
                [Step::Index(row), Step::Field(0)] if place.0 == 0 => {
                    self.0.get(row) == Some(&text)
                }
                _ => false,
            }
        }
    }

    fn row(index: usize, field: usize) -> Place {
        Place(0, vec![Step::Index(index), Step::Field(field)])
    }

    fn bindings_at(index: usize) -> Vec<(DownloadField, Place)> {
        vec![
            (DownloadField::Id, row(index, 0)),
            (DownloadField::Progress, row(index, 1)),
            (DownloadField::Status, Place(1, Vec::new())),
        ]
    }

    #[test]
    fn bindings_stay_while_the_item_stays() {
        let mut bindings = bindings_at(1);
        assert!(follow_id(&Rows(vec!["a", "b"]), &mut bindings, "b"));
        assert_eq!(bindings, bindings_at(1));
    }

    #[test]
    fn bindings_follow_their_item_to_its_new_position() {
        let mut bindings = bindings_at(2);
        assert!(follow_id(&Rows(vec!["c", "x"]), &mut bindings, "c"));
        assert_eq!(
            bindings,
            [
                (DownloadField::Id, row(0, 0)),
                (DownloadField::Progress, row(0, 1)),
                (DownloadField::Status, Place(1, Vec::new())),
            ],
            "the binding outside the list stays"
        );
    }

    #[test]
    fn a_removed_item_lets_go_of_the_download() {
        let mut bindings = bindings_at(0);
        assert!(!follow_id(&Rows(vec!["x"]), &mut bindings, "a"));
        assert!(!follow_id(&Rows(Vec::new()), &mut bindings, "a"));
    }

    #[test]
    fn bindings_without_an_id_stay_put() {
        let mut bindings = vec![(DownloadField::Progress, row(3, 1))];
        assert!(follow_id(&Rows(Vec::new()), &mut bindings, "a"));
        assert_eq!(bindings, [(DownloadField::Progress, row(3, 1))]);
    }
}
