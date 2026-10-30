//! Land types: the mana ability each basic land type carries (CR 305.6) and which land has it once the type layer has made
//! the land types of every land (CR 305.7).

use mtg_cards::{AbilityDef, CardDefinition, ManaAbility};
use mtg_core::card::{CardType, LAND_TYPES, Subtype};
use mtg_core::mana::ManaType;

use crate::characteristics::Characteristics;

/// The basic land types in the order of CR 305.6, each with the mana its intrinsic ability adds.
const BASIC: [(&str, ManaType); 5] = [
    ("Plains", ManaType::White),
    ("Island", ManaType::Blue),
    ("Swamp", ManaType::Black),
    ("Mountain", ManaType::Red),
    ("Forest", ManaType::Green),
];

/// Whether the subtype is a land type, which an effect that sets the land types replaces (CR 205.3i).
pub(crate) fn is_land_type(subtype: &Subtype) -> bool {
    LAND_TYPES.contains(&subtype.0.as_str())
}

/// The intrinsic mana abilities of the basic land types `has` says a land has, in the order of the rules.
fn intrinsic(has: impl Fn(&Subtype) -> bool) -> Vec<AbilityDef> {
    BASIC
        .iter()
        .filter(|(name, _)| has(&Subtype((*name).to_owned())))
        .map(|&(_, produces)| AbilityDef::Mana(ManaAbility { produces }))
        .collect()
}

/// Gives a land the mana ability of each basic land type it has now in place of the ones its card prints for its printed
/// types: a type an effect gave brings its ability, a type an effect took away takes it, and a permanent that is no
/// longer a land has none. The other plain mana abilities of the permanent stay. It runs once the type layer is through,
/// so the abilities the later layers give or take away (CR 613.1f) act on the result.
pub(crate) fn sync(chars: &mut Characteristics, card: &CardDefinition) {
    if card.is_type(CardType::Land) {
        for printed in intrinsic(|subtype| card.subtypes.contains(subtype)) {
            if let Some(at) = chars.abilities.iter().position(|held| *held == printed) {
                chars.abilities.remove(at);
            }
        }
    }
    if chars.types.contains(CardType::Land) {
        let mut abilities = intrinsic(|subtype| chars.has_subtype(&card.subtypes, subtype));
        abilities.append(&mut chars.abilities);
        chars.abilities = abilities;
    }
}
