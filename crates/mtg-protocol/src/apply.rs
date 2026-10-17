//! The client-side fold: a viewer's [`StateView`] advanced by one [`EventBatch`].
//!
//! This is the whole of what a client does with the game state. It never looks at `events`.

use std::collections::BTreeSet;

use crate::events::EventBatch;
use crate::view::{StateView, TableView};

impl StateView {
    /// Advances this view by `batch`: replaces the table, inserts or replaces `objects`, drops `gone`.
    ///
    /// The result equals [`StateView::for_player`] on the state after the batch, provided this view
    /// was the viewer's view before it. `objects` stays sorted by id.
    pub fn apply(&mut self, batch: &EventBatch) {
        self.table.clone_from(&batch.table);
        for object in &batch.objects {
            match self.objects.binary_search_by_key(&object.id, |o| o.id) {
                Ok(at) => self.objects[at].clone_from(object),
                Err(at) => self.objects.insert(at, object.clone()),
            }
        }
        if !batch.gone.is_empty() {
            let gone: BTreeSet<_> = batch.gone.iter().collect();
            self.objects.retain(|o| !gone.contains(&o.id));
        }
    }

    /// The view a client with no snapshot builds from the batch at `from_seq` 0, or `None` for a later batch.
    pub fn from_first_batch(batch: &EventBatch) -> Option<StateView> {
        (batch.from_seq == 0).then(|| {
            let mut view = StateView {
                table: TableView::clone(&batch.table),
                objects: Vec::new(),
            };
            view.apply(batch);
            view
        })
    }
}
