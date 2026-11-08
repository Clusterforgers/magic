//! Champion (CR 702.72 of the old rules): "When this enters, sacrifice it unless you exile another [type] you control. When
//! this leaves the battlefield, that card returns to the battlefield."

use serde_json::{Value, json};

use crate::translate::ctx::{Ctx, Tr, unmapped};
use crate::translate::valid::selector_any;

/// `Champion:<type>[,<type>]`: the trigger that exiles a creature of the type for as long as the champion stays, or
/// sacrifices the champion. The exile ends, and the card returns, when the champion leaves (CR 610.3).
pub fn champion(ctx: &Ctx, types: &str) -> Tr<Value> {
    let words: Vec<&str> = types.split(',').collect();
    let known = |word: &&str| *word == "Creature" || ctx.world.subtypes.contains(*word);
    if words.iter().any(|word| !known(word)) {
        return unmapped("keyword:Champion-type");
    }
    let spec = words
        .iter()
        .map(|word| format!("Creature.{word}+Other+YouCtrl"))
        .map(|spec| spec.replace("Creature.Creature+", "Creature."))
        .collect::<Vec<_>>()
        .join(",");
    let choices = selector_any(ctx, &spec)?;
    Ok(json!({"Triggered": {
        "when": {"ZoneChange": {"what": ["This"], "to": "Battlefield"}},
        "effects": [
            {"Choose": {"from": {"Each": choices}, "count": {"min": 0, "max": 1}, "as": "champion"}},
            // A champion gone before this resolves has nothing to return the card: it stays exiled (the return is a separate
            // ability that found nothing). The exile below then passes over the card already moved.
            {"If": {"condition": {"Compare": {
                "left": {"Count": [{"Is": "This"}, {"In": "Battlefield"}]}, "op": "Eq", "right": 0}},
                "then": [{"Exile": {"what": {"Bound": "champion"}}}]}},
            {"Exile": {"what": {"Bound": "champion"}, "duration": "WhileSourceOnBattlefield"}},
            {"If": {"condition": {"Not": "Performed"}, "then": [{"Sacrifice": {"filter": ["This"]}}]}}]}}))
}
