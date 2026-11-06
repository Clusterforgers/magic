//! The wordings of each ability written while one outer call writes its wordings: the writers call each other on
//! changed abilities (selectors reordered, changes sorted, stand-ins swapped in), and changes that meet again would
//! write the same ability once for every path to it. Each is written once per outer call; the table is emptied when
//! that call returns, so it holds only what one ability needs.
//!
//! A writer that joins the wordings of two parts of an ability ("gets +2/+2 and can't attack") writes the parts as
//! parts, and the Oracle text joins at most three ("gets +1/+1 and has flying and lifelink"): a part of a part joins
//! no parts of its own. Deeper joins would multiply the wordings by the number of each part's wordings at every level.

use std::cell::RefCell;
use std::collections::HashMap;

use crate::ability::Ability;

/// How many joins may enclose a wording that joins parts of its own: a sentence of three parts at most.
const MOST_JOINS: u8 = 2;

/// The abilities written so far in the outer call, by the joins around them and their JSON; how deep the calls are.
#[derive(Default)]
struct Table {
    depth: usize,
    joins: u8,
    done: HashMap<(u8, String), Vec<String>>,
    #[cfg(test)]
    writes: usize,
}

thread_local! {
    static TABLE: RefCell<Table> = RefCell::default();
}

/// Leaves a call: one level less, and an empty table when it was the outer one, also when the writer panics.
struct Leave;

impl Drop for Leave {
    fn drop(&mut self) {
        TABLE.with_borrow_mut(|table| {
            table.depth = table.depth.saturating_sub(1);
            if table.depth == 0 {
                table.done.clear();
            }
        });
    }
}

/// `write(ability)`, or the wordings it gave for an equal ability earlier in the same outer call.
pub(super) fn written(ability: &Ability, write: fn(&Ability) -> Vec<String>) -> Vec<String> {
    let Ok(json) = serde_json::to_string(ability) else {
        return write(ability);
    };
    let key = (TABLE.with_borrow(|table| table.joins), json);
    if let Some(found) = TABLE.with_borrow(|table| table.done.get(&key).cloned()) {
        return found;
    }
    TABLE.with_borrow_mut(|table| {
        table.depth += 1;
        #[cfg(test)]
        {
            table.writes += 1;
        }
    });
    let leave = Leave;
    let texts = write(ability);
    TABLE.with_borrow_mut(|table| {
        if table.depth > 1 {
            table.done.insert(key, texts.clone());
        }
    });
    drop(leave);
    texts
}

/// Whether the wordings written now may join parts of their own.
pub(super) fn may_join() -> bool {
    TABLE.with_borrow(|table| table.joins < MOST_JOINS)
}

/// Restores the joins around the wordings, also when the writer panics.
struct Restore(u8);

impl Drop for Restore {
    fn drop(&mut self) {
        TABLE.with_borrow_mut(|table| table.joins = self.0);
    }
}

/// `write()` with the wordings it writes taken as parts of one more join.
pub(super) fn as_parts<T>(write: impl FnOnce() -> T) -> T {
    let before = TABLE.with_borrow_mut(|table| {
        let before = table.joins;
        table.joins = before.saturating_add(1);
        before
    });
    let _restore = Restore(before);
    write()
}

/// How many abilities this thread has written without the table, for tests that bound the work.
#[cfg(test)]
pub(super) fn written_count() -> usize {
    TABLE.with_borrow(|table| table.writes)
}
