//! r47-05 wordings of whole abilities whose wording one card prints (`r4705_wholes.json`), shared with the reader.

use crate::ability::Ability;

/// Adds the wording of an ability that one of the r47-05 entries matches whole.
pub(super) fn r4705_add_wholes(ability: &Ability, texts: &mut Vec<String>) {
    for (shape, text) in super::super::super::r4705_reads::R4705_SHAPES.iter() {
        if matches!(shape.as_slice(), [one] if one == ability) {
            texts.push(text.clone());
        }
    }
}
