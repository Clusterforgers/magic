//! Mana of the colours of a card the source exiled: "Add one mana of any of the exiled card's colors" (an imprint, the
//! one card the source remembers as imprinted) and "... of any of the exiled cards' colors" (every card exiled with it).

use crate::refs::{EXILED_SLOT, Pred, Ref, Selector, ZoneKind};

/// The slot an imprint ability remembers its card in.
const IMPRINTED: &str = "imprinted";

/// The words of the mana of the colours among the cards the source exiled, `None` for any other objects.
pub(super) fn ep26_exiled_colors_text(among: &Selector) -> Option<&'static str> {
    let [first, Pred::In(ZoneKind::Exile)] = among.0.as_slice() else {
        return None;
    };
    match first {
        Pred::Is(Ref::Remembered { on, slot }) if **on == Ref::This && slot == IMPRINTED => {
            Some("one mana of any of the exiled card's colors")
        }
        Pred::ExiledWith(Ref::This) => Some("one mana of any of the exiled cards' colors"),
        Pred::Is(Ref::Remembered { on, slot }) if **on == Ref::This && slot == EXILED_SLOT => {
            Some("one mana of any of the exiled cards' colors")
        }
        _ => None,
    }
}
