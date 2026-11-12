//! Suffixes of the shared noun-phrase reader and writer for the r30-1b pack, consulted after the others: "banded with ~"
//! (CR 702.22).

use super::selectors::Form;
use super::tokens::Reader;
use crate::refs::{Pred, Ref};

/// "banded with ~": a creature in a band with the card itself.
pub(super) fn r301b_suffix(r: &mut Reader<'_>, _form: Form) -> Option<Pred> {
    r.attempt(|r| {
        r.need("banded with")?;
        r.source().then_some(Pred::BandedWith(Ref::This))
    })
}

/// The suffix of this module for a predicate.
pub(super) fn r301b_suffix_text(pred: &Pred, _form: Form) -> Option<String> {
    (*pred == Pred::BandedWith(Ref::This)).then(|| "banded with ~".to_owned())
}
