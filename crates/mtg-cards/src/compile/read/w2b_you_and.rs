//! "You and target opponent each draw two cards.": you and one other player each draw, one `Draw` for each.

use super::draws::drawn;
use super::effects::player;
use super::tokens::Reader;
use crate::effect::Effect;
use crate::refs::Ref;

/// "You and target opponent each draw two cards.", "You and each opponent each draw a card.": your draw, then the
/// other player's.
pub(super) fn w2b_you_and_each(r: &mut Reader<'_>) -> Option<Vec<Effect>> {
    r.need("you and")?;
    let other = player(r)?;
    if other == Ref::You {
        return None;
    }
    r.need("each draw")?;
    let count = drawn(r)?;
    Some(vec![
        Effect::Draw {
            who: Ref::You,
            count: count.clone(),
        },
        Effect::Draw { who: other, count },
    ])
}
