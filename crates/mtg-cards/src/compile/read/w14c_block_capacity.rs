//! "~ can block an additional creature each combat.", "Each creature you control can block an additional creature each
//! combat.": a static that lets its creatures block more than one attacker (CR 509.1a).

use super::Reader;
use super::statics::subject;
use super::amounts::count_word;
use crate::ability::Ability;
use crate::modify::Modification;
use crate::refs::ZoneKind;

/// "[subject] can block an additional creature each combat", "... an additional two creatures each combat".
pub(super) fn w14c_blocks_additional(r: &mut Reader<'_>) -> Option<Ability> {
    let (affected, _) = subject(r, true)?;
    r.words("can block an additional").then_some(())?;
    let count = if r.word("creature") {
        1
    } else {
        let count = r.number().filter(|count| *count > 1)?;
        r.word("creatures").then_some(())?;
        count
    };
    r.words("each combat").then_some(())?;
    Some(Ability::Static {
        affected,
        mods: vec![Modification::BlocksAdditional(count)],
        condition: None,
        defining: false,
        from: ZoneKind::Battlefield,
    })
}

/// The predicate of the modification: "can block an additional creature each combat".
pub(super) fn w14c_blocks_additional_text(modification: &Modification) -> Option<String> {
    match modification {
        Modification::BlocksAdditional(1) => {
            Some("can block an additional creature each combat".to_owned())
        }
        Modification::BlocksAdditional(count @ 2..) => Some(format!(
            "can block an additional {} creatures each combat",
            count_word(*count)?
        )),
        _ => None,
    }
}
