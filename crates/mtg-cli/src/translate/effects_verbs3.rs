//! Effects that reveal a hand, name a card, copy a spell or choose a player.

use serde_json::{Value, json};

use super::ctx::{Ctx, Tr, only};
use super::valid::subject;
use crate::forge::script::Params;

/// The effects of a verb this module reads; `None` for any other API.
pub fn effects(ctx: &mut Ctx, p: &Params, api: &str) -> Option<Tr<Vec<Value>>> {
    let node = match api {
        "RevealHand" => reveal_hand(ctx, p),
        _ => return None,
    };
    Some(node.map(|node| vec![node]))
}

/// `RevealHand`: the player shows every card of their hand. Looking at the hand (`Look$`) or remembering the cards
/// are other effects and stay gaps.
fn reveal_hand(ctx: &mut Ctx, p: &Params) -> Tr<Value> {
    only(p, "RevealHand", &["Defined", "ValidTgts"])?;
    let who = subject(ctx, p, "You", None)?;
    Ok(json!({"Reveal": {"what": {"Each": [{"OwnedBy": who}, {"In": "Hand"}]}}}))
}
