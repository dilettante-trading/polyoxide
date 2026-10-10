//! The allow-lists a wire-agreement test keeps, and which of their entries a
//! run used.
//!
//! An entry excuses one key path, with a written reason. A suite that keeps
//! one fixture per type writes `(fixture, path, reason)`; one that names its
//! paths from the type writes `(path, reason)`. Both are [`Excuse`]s, so each
//! suite's const tables stay as written.
//!
//! A [`Ledger`] records the entries a run used, so an entry no fixture needs
//! any more can fail the test instead of excusing a difference that is gone.

use std::collections::BTreeSet;

/// One allow-list entry: a key path, the fixture it applies to, and why.
pub trait Excuse {
    /// The fixture the entry applies to, or `None` for every fixture.
    fn fixture(&self) -> Option<&str>;

    /// The key path the entry excuses.
    fn path(&self) -> &str;

    /// Why the path is excused.
    fn reason(&self) -> &str;

    /// Whether the entry excuses `path` in `fixture`.
    fn excuses(&self, fixture: &str, path: &str) -> bool {
        self.fixture().is_none_or(|f| f == fixture) && self.path() == path
    }
}

/// `(fixture, path, reason)`.
impl Excuse for (&str, &str, &str) {
    fn fixture(&self) -> Option<&str> {
        Some(self.0)
    }

    fn path(&self) -> &str {
        self.1
    }

    fn reason(&self) -> &str {
        self.2
    }
}

/// `(path, reason)`, for every fixture.
impl Excuse for (&str, &str) {
    fn fixture(&self) -> Option<&str> {
        None
    }

    fn path(&self) -> &str {
        self.0
    }

    fn reason(&self) -> &str {
        self.1
    }
}

/// A test's allow-lists, and the entries a run has used.
///
/// An entry is used once any check is excused by it, and every entry with the
/// same fixture and path counts as used with it, whichever list it is in.
/// [`stale`](Self::stale) is what no check used.
///
/// ```
/// use polyoxide_test_support::agreement::Ledger;
///
/// const IGNORED: &[(&str, &str, &str)] = &[("trades", "/data[]/fee", "not modelled yet")];
/// const EXPECTED_ABSENT: &[(&str, &str, &str)] = &[("trades", "/cursor", "last page")];
///
/// let mut ledger = Ledger::new(&[IGNORED, EXPECTED_ABSENT]);
/// assert!(ledger.excuse(IGNORED, "trades", "/data[]/fee").is_some());
/// assert!(ledger.excuse(IGNORED, "trades", "/cursor").is_none());
/// assert_eq!(ledger.stale(), [&("trades", "/cursor", "last page")]);
/// ```
#[derive(Debug, Clone)]
pub struct Ledger<'a, E> {
    lists: Vec<&'a [E]>,
    used: BTreeSet<(Option<String>, String)>,
}

impl<'a, E: Excuse> Ledger<'a, E> {
    /// A ledger over `lists`, none of whose entries is used yet.
    pub fn new(lists: &[&'a [E]]) -> Self {
        Self {
            lists: lists.to_vec(),
            used: BTreeSet::new(),
        }
    }

    /// The entry of `list` that excuses `path` in `fixture`, now marked used,
    /// or `None` when no entry does. `list` must be one of the ledger's lists.
    #[track_caller]
    pub fn excuse(&mut self, list: &'a [E], fixture: &str, path: &str) -> Option<&'a E> {
        self.assert_held(list);
        let entry = list.iter().find(|e| e.excuses(fixture, path))?;
        self.mark(entry);
        Some(entry)
    }

    /// For each entry of `list` that applies to `fixture`, asserts that no
    /// path in `sent` starts with the entry's path, and marks it used.
    ///
    /// The list names paths a capture is known to lack, so the type behind
    /// each has never been checked against the wire. The first capture that
    /// carries one fails here, to be checked by hand.
    #[track_caller]
    pub fn never_on_wire(&mut self, list: &'a [E], fixture: &str, sent: &BTreeSet<String>) {
        self.assert_held(list);
        for entry in list
            .iter()
            .filter(|e| e.fixture().is_none_or(|f| f == fixture))
        {
            let np = entry.path();
            let present = sent.iter().any(|p| p.starts_with(np));
            assert!(
                !present,
                "{fixture}: the server now sends {np} ({}); verify the shape by hand and remove the NEVER_ON_WIRE row",
                entry.reason()
            );
            self.mark(entry);
        }
    }

    /// Every entry, across the lists in order, that no check used.
    pub fn stale(&self) -> Vec<&'a E> {
        self.lists
            .iter()
            .flat_map(|list| list.iter())
            .filter(|e| !self.used.contains(&key(*e)))
            .collect()
    }

    fn mark(&mut self, entry: &E) {
        self.used.insert(key(entry));
    }

    /// By content, since a `const` table is not promised one address.
    #[track_caller]
    fn assert_held(&self, list: &[E]) {
        let same =
            |held: &&[E]| held.len() == list.len() && held.iter().map(key).eq(list.iter().map(key));
        assert!(
            self.lists.iter().any(same),
            "an allow-list this ledger was not built over"
        );
    }
}

fn key(entry: &impl Excuse) -> (Option<String>, String) {
    (entry.fixture().map(str::to_owned), entry.path().to_owned())
}

#[cfg(test)]
mod tests {
    use std::panic::catch_unwind;

    use super::*;

    const IGNORED: &[(&str, &str, &str)] = &[("book", "/levels[]/n", "a count, not data")];
    const EXPECTED_ABSENT: &[(&str, &str, &str)] = &[
        ("fills", "/cursor", "sent only when another page exists"),
        ("board", "/account", "sent only for an address"),
    ];
    const NEVER_ON_WIRE: &[(&str, &str, &str)] =
        &[("index", "/constituents[]", "every index answered []")];

    #[test]
    fn a_triple_excuses_its_own_fixture_only_and_a_pair_every_fixture() {
        let triple = ("book", "/a", "why");
        assert!(triple.excuses("book", "/a"));
        assert!(!triple.excuses("trades", "/a"));
        assert!(!triple.excuses("book", "/b"));
        let pair = ("comment.profile.bio", "why");
        assert!(pair.excuses("anything", "comment.profile.bio"));
        assert_eq!(
            (pair.fixture(), pair.path(), pair.reason()),
            (None, "comment.profile.bio", "why")
        );
    }

    #[test]
    fn excuse_returns_the_entry_and_marks_it_used() {
        let mut ledger = Ledger::new(&[IGNORED, EXPECTED_ABSENT]);
        assert_eq!(ledger.stale().len(), 3);
        assert_eq!(
            ledger.excuse(EXPECTED_ABSENT, "fills", "/cursor"),
            Some(&EXPECTED_ABSENT[0])
        );
        assert_eq!(ledger.excuse(EXPECTED_ABSENT, "board", "/cursor"), None);
        // The entry is in the other list, so this list does not excuse it.
        assert_eq!(ledger.excuse(EXPECTED_ABSENT, "book", "/levels[]/n"), None);
        assert_eq!(ledger.stale(), [&IGNORED[0], &EXPECTED_ABSENT[1]]);
    }

    #[test]
    fn never_on_wire_passes_while_absent_and_marks_the_entry() {
        let mut ledger = Ledger::new(&[NEVER_ON_WIRE]);
        let sent: BTreeSet<String> = ["/name".to_owned(), "/constituents".to_owned()].into();
        ledger.never_on_wire(NEVER_ON_WIRE, "time", &sent);
        assert_eq!(
            ledger.stale().len(),
            1,
            "another fixture's entry is untouched"
        );
        ledger.never_on_wire(NEVER_ON_WIRE, "index", &sent);
        assert!(ledger.stale().is_empty());
    }

    #[test]
    fn never_on_wire_fails_on_the_first_capture_that_carries_the_path() {
        let mut ledger = Ledger::new(&[NEVER_ON_WIRE]);
        let sent: BTreeSet<String> = ["/constituents[]/weight".to_owned()].into();
        let panic =
            catch_unwind(move || ledger.never_on_wire(NEVER_ON_WIRE, "index", &sent)).unwrap_err();
        let message = panic.downcast_ref::<String>().unwrap();
        assert_eq!(
            message,
            "index: the server now sends /constituents[] (every index answered []); verify \
             the shape by hand and remove the NEVER_ON_WIRE row"
        );
    }

    #[test]
    fn a_list_the_ledger_does_not_hold_is_refused() {
        let mut ledger = Ledger::new(&[IGNORED]);
        let panic = catch_unwind(move || {
            ledger.excuse(EXPECTED_ABSENT, "fills", "/cursor");
        })
        .unwrap_err();
        let message = panic.downcast_ref::<&str>().unwrap();
        assert_eq!(*message, "an allow-list this ledger was not built over");
    }
}
