//! "You may cast ~ as though it had flash. If you cast it any time a sorcery couldn't have been cast, the controller of
//! the permanent it becomes sacrifices it at the beginning of the next cleanup step." is the keyword `FlashSacrifice`.

use super::tokens::Reader;
use crate::ability::Ability;
use crate::keyword::Keyword;

/// The second sentence, after the cast "as though it had flash".
const R6F_SACRIFICE: &str = "if you cast it any time a sorcery couldn't have been cast";
/// What happens to the permanent.
const R6F_CLEANUP: &str = "the controller of the permanent it becomes sacrifices it at the beginning of the next cleanup step";

/// Reads the two sentences of the keyword.
pub(super) fn r6f_flash_sacrifice(r: &mut Reader<'_>) -> Option<Ability> {
    r.need("you may cast")?;
    r.source().then_some(())?;
    r.need("as though it had flash")?;
    r.punct('.').then_some(())?;
    r.need(R6F_SACRIFICE)?;
    r.punct(',').then_some(())?;
    r.need(R6F_CLEANUP)?;
    Some(Ability::Keyword(Keyword::FlashSacrifice))
}

/// The wording of the keyword, without the last period; none for another ability.
pub(super) fn r6f_variants(ability: &Ability) -> Option<Vec<String>> {
    matches!(ability, Ability::Keyword(Keyword::FlashSacrifice)).then(|| {
        vec![format!(
            "You may cast ~ as though it had flash. {}{}, {R6F_CLEANUP}",
            R6F_SACRIFICE[..1].to_uppercase(),
            &R6F_SACRIFICE[1..]
        )]
    })
}
