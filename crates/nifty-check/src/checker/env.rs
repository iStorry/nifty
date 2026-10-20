//! What the checker knows while it walks a body: locals in scope, what context it is in,
//! and which state the code it has seen reads and writes.

use std::collections::{BTreeSet, HashMap};

use crate::hir::{FnId, LocalId, StateSet, Type};

/// What kind of code is being checked, which decides what it may do.
#[derive(Clone, Debug)]
pub(super) enum Context {
    /// A function body; `return` must produce this type.
    Function { return_type: Type },
    /// A window or view body: elements, `if` and `for`.
    Ui,
    /// A button's action: statements that may change state.
    Action,
    /// A state's starting value, which must not read other state.
    StateInit,
}

#[derive(Clone, Debug)]
pub(super) struct LocalInfo {
    pub ty: Type,
    pub kind: LocalKind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum LocalKind {
    /// A parameter, a `let`, or a `for` variable over a computed list.
    Variable,
    /// A `for` variable over a place; writing through it changes the original.
    Alias,
    /// A `for` loop's index or range counter, which the loop sets each time round.
    Counter,
}

/// One function, view or window body: its locals and the scopes that name them.
#[derive(Debug)]
pub(super) struct Body {
    pub locals: Vec<LocalInfo>,
    scopes: Vec<HashMap<String, LocalId>>,
    pub context: Context,
}

impl Body {
    pub(super) fn new(context: Context) -> Self {
        Self {
            locals: Vec::new(),
            scopes: vec![HashMap::new()],
            context,
        }
    }

    /// Allocates a local and names it in the current scope.
    pub(super) fn declare(&mut self, name: &str, info: LocalInfo) -> LocalId {
        let id = self.allocate(info);
        self.bind(name, id);
        id
    }

    /// Allocates a local without naming it yet.
    pub(super) fn allocate(&mut self, info: LocalInfo) -> LocalId {
        let id = LocalId(self.locals.len());
        self.locals.push(info);
        id
    }

    pub(super) fn bind(&mut self, name: &str, id: LocalId) {
        self.scopes
            .last_mut()
            .expect("a body always has a scope")
            .insert(name.to_owned(), id);
    }

    pub(super) fn lookup(&self, name: &str) -> Option<LocalId> {
        self.scopes
            .iter()
            .rev()
            .find_map(|scope| scope.get(name).copied())
    }

    pub(super) fn local(&self, id: LocalId) -> &LocalInfo {
        &self.locals[id.0]
    }

    pub(super) fn push_scope(&mut self) {
        self.scopes.push(HashMap::new());
    }

    pub(super) fn pop_scope(&mut self) {
        self.scopes.pop();
    }
}

/// State reads and writes seen so far, and the functions called (whose effects count too).
#[derive(Debug, Default)]
pub(super) struct Effects {
    pub reads: StateSet,
    pub writes: bool,
    pub calls: BTreeSet<FnId>,
}

impl Effects {
    pub(super) fn merge(&mut self, other: &Self) {
        self.reads.extend(&other.reads);
        self.writes |= other.writes;
        self.calls.extend(other.calls.iter().copied());
    }
}

/// Every function's reads and writes, including those of the functions it calls.
#[derive(Debug, Default)]
pub(super) struct FnEffects {
    pub reads: Vec<StateSet>,
    pub writes: Vec<bool>,
}

impl FnEffects {
    /// Propagates effects along calls until nothing changes.
    pub(super) fn solve(direct: &[Effects]) -> Self {
        let mut reads: Vec<StateSet> = direct.iter().map(|e| e.reads.clone()).collect();
        let mut writes: Vec<bool> = direct.iter().map(|e| e.writes).collect();
        let mut changed = true;
        while changed {
            changed = false;
            for (caller, effects) in direct.iter().enumerate() {
                for callee in &effects.calls {
                    let callee_reads = reads[callee.0].clone();
                    let before = reads[caller].clone();
                    reads[caller].extend(&callee_reads);
                    let callee_writes = writes[callee.0];
                    if reads[caller] != before || (callee_writes && !writes[caller]) {
                        writes[caller] |= callee_writes;
                        changed = true;
                    }
                }
            }
        }
        Self { reads, writes }
    }

    /// The full reads and whether anything is written, for code with `effects`.
    pub(super) fn total(&self, effects: &Effects) -> (StateSet, bool) {
        let mut reads = effects.reads.clone();
        let mut writes = effects.writes;
        for callee in &effects.calls {
            reads.extend(&self.reads[callee.0]);
            writes |= self.writes[callee.0];
        }
        (reads, writes)
    }
}
