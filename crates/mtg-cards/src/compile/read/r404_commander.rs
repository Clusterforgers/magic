//! r40-4 commander colors: "If ~ is your commander, choose a color before the game begins. ~ is the chosen color." is
//! the card's characteristic-defining static ability `SetCommanderColor`, read and written as that one text.

use super::tokens::same_text;
use crate::ability::Ability;
use crate::modify::Modification;
use crate::refs::{Ref, ZoneKind};

/// The text of the ability.
const TEXT: &str =
    "If ~ is your commander, choose a color before the game begins. ~ is the chosen color.";

/// The ability the text stands for.
fn commander_color() -> Ability {
    Ability::Static {
        affected: Ref::This,
        mods: vec![Modification::SetCommanderColor],
        condition: None,
        defining: true,
        from: ZoneKind::Battlefield,
    }
}

/// "If ~ is your commander, choose a color before the game begins. ~ is the chosen color."
pub(super) fn r404_commander_read(text: &str) -> Option<Ability> {
    same_text(text, TEXT).then(commander_color)
}

/// The words of the commander color ability.
pub(super) fn r404_commander_wordings(ability: &Ability, texts: &mut Vec<String>) {
    if *ability == commander_color() && !texts.iter().any(|text| text == TEXT) {
        texts.push(TEXT.to_owned());
    }
}
