//! Wordings of the fourth sample of the names S to Z that the renderers write for a twin of the ability and the card
//! words with other phrases: the twin has one part changed (a condition or a source dropped, a kind of land, a type
//! chosen as the card enters), its wordings are written, and the edits give the card's words ("As long as this card is
//! in your graveyard and you control an Island, creatures you control have flying.").

use super::super::super::amounts::count_word;
use super::super::written_variants;
use super::twin_json::twin;
use crate::ability::Ability;
use serde_json::{Value, json};

/// One edit of a twin's wording into the card's.
enum Edit {
    /// The first `.0` is `.1`.
    Replace(String, String),
    /// The wording opens with "As long as" and this phrase; the old first word loses its capital unless it is a subtype.
    Condition(String, bool),
}

/// The edit that replaces `from` with `to`.
fn rep(from: &str, to: &str) -> Edit {
    Edit::Replace(from.to_owned(), to.to_owned())
}

/// What a maker gives: the twin and the edits that turn its wording into the card's.
type Made = (Ability, Vec<Edit>);

/// A maker of the twin of an ability, for the one shape it knows.
type Maker = fn(&Ability) -> Option<Made>;

/// The makers, one line each.
const MAKERS: [Maker; 13] = [
    in_graveyard,
    mill_then_land,
    untap_that_land,
    opponent_graveyard,
    untapped_shield,
    look_x,
    nonbasic_walk,
    cast_and_attack,
    cast_commander,
    no_creature_died,
    shared_type,
    chosen_died,
    chosen_cast,
];

/// Adds the wordings of the twin of an ability, with the edits applied.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    for maker in MAKERS {
        let Some((made, edits)) = maker(ability) else {
            continue;
        };
        for text in written_variants(&made) {
            if let Some(edited) = edits.iter().try_fold(text, |text, edit| apply(edit, &text)) {
                texts.push(edited);
            }
        }
    }
}

/// The text with the edit applied; `None` when the edit finds nothing to change.
fn apply(edit: &Edit, text: &str) -> Option<String> {
    match edit {
        Edit::Replace(from, to) => text
            .contains(from.as_str())
            .then(|| text.replacen(from.as_str(), to, 1)),
        Edit::Condition(phrase, subtype) => {
            let mut letters = text.chars();
            let first = letters.next()?;
            if text.starts_with("Each ") || text.starts_with("As long as ") {
                return None;
            }
            let rest = if *subtype {
                text.to_owned()
            } else {
                first.to_lowercase().chain(letters).collect()
            };
            Some(format!("As long as {phrase}, {rest}"))
        }
    }
}

/// The JSON of the ability changed by `change`, read back as the twin.
fn made(
    ability: &Ability,
    change: impl FnOnce(&mut Value) -> Option<()>,
    edits: Vec<Edit>,
) -> Option<Made> {
    Some((twin(ability, change)?, edits))
}

/// The body of a static, replacement, triggered or activated ability.
fn body(value: &mut Value) -> Option<&mut Value> {
    ["Static", "Replacement", "Triggered", "Activated"]
        .into_iter()
        .find(|key| value.get(key).is_some())
        .and_then(|key| value.get_mut(key))
}

/// A static that works in the graveyard: "As long as this card is in your graveyard and you control an Island, ...".
fn in_graveyard(ability: &Ability) -> Option<Made> {
    made(
        ability,
        |value| {
            let inner = value.get_mut("Static")?;
            (inner.get("from")? == "Graveyard" && inner.get("condition").is_some()).then_some(())?;
            inner.as_object_mut()?.remove("from");
            Some(())
        },
        vec![rep(
            "As long as ",
            "As long as this card is in your graveyard and ",
        )],
    )
}

/// A mode that mills cards, then lets the player put a land card from the graveyard onto the battlefield tapped.
fn mill_then_land(ability: &Ability) -> Option<Made> {
    let mut milled = 0;
    let made = made(
        ability,
        |value| {
            let effects = value.pointer_mut("/Spell/effects/0/Modal/modes/1/effects")?;
            milled = effects.pointer("/0/Mill/count")?.as_u64()?;
            let land = json!({"Choose": {"from": {"Each": [{"In": "Graveyard"}, {"OwnedBy": "You"}, {"Type": "Land"}]}, "as": "land"}});
            let put = json!({"Move": {"what": {"Bound": "land"}, "to": {"Battlefield": {"tapped": true}}}});
            let shape = json!([{"Mill": {"count": milled}}, {"May": {"effects": [land, put]}}]);
            (*effects == shape).then_some(())?;
            *effects = json!([{"Mill": {"count": milled}}]);
            Some(())
        },
        Vec::new(),
    )?;
    let words = count_word(u32::try_from(milled).ok()?)?;
    let from = format!("\u{2022} Mill {words} cards.");
    let to = format!(
        "\u{2022} Mill {words} cards, then you may put a land card from your graveyard onto the battlefield tapped."
    );
    Some((made.0, vec![Edit::Replace(from, to)]))
}

/// A mode that untaps the land of the event: "Untap that land."
fn untap_that_land(ability: &Ability) -> Option<Made> {
    made(
        ability,
        |value| {
            let first = value.pointer_mut("/Triggered/effects/0/Modal/modes/0/effects")?;
            (*first == json!([{"Untap": {"what": "Triggering"}}])).then_some(())?;
            *first = json!([{"Draw": {}}]);
            Some(())
        },
        vec![rep("\u{2022} Draw a card.", "\u{2022} Untap that land.")],
    )
}

/// A static under "an opponent has eight or more cards in their graveyard".
fn opponent_graveyard(ability: &Ability) -> Option<Made> {
    let mut count = 0;
    let mut subtype = false;
    let made = made(
        ability,
        |value| {
            let inner = value.get_mut("Static")?;
            let cond = inner.get("condition")?;
            let left = json!({"Largest": {"who": "Opponents", "of": [{"In": "Graveyard"}]}});
            let shape = cond.get("Compare")?;
            (shape.get("left")? == &left && shape.get("op")? == "Ge").then_some(())?;
            count = shape.get("right")?.as_u64()?;
            subtype = inner.pointer("/affected/Each/0/Subtype").is_some();
            inner.as_object_mut()?.remove("condition");
            Some(())
        },
        Vec::new(),
    )?;
    let words = count_word(u32::try_from(count).ok()?)?;
    let phrase = format!("an opponent has {words} or more cards in their graveyard");
    Some((made.0, vec![Edit::Condition(phrase, subtype)]))
}

/// A prevention that works while the card is untapped.
fn untapped_shield(ability: &Ability) -> Option<Made> {
    made(
        ability,
        |value| {
            let inner = value.get_mut("Replacement")?;
            let shape = json!({"Matches": {"subject": "This", "selector": ["Untapped"]}});
            (inner.get("condition")? == &shape).then_some(())?;
            inner.as_object_mut()?.remove("condition");
            Some(())
        },
        vec![rep(
            "If a creature would",
            "As long as this artifact is untapped, if a creature would",
        )],
    )
}

/// "Look at the top X cards of your library, then put them back in any order." from the same words for three.
fn look_x(ability: &Ability) -> Option<Made> {
    made(
        ability,
        |value| {
            let look = value.pointer_mut("/Activated/effects/0/LookAtTop/count")?;
            (*look == "X").then_some(())?;
            *look = json!(3);
            Some(())
        },
        vec![rep("top three cards", "top X cards")],
    )
}

/// "Equipped creature has nonbasic landwalk." from swampwalk.
fn nonbasic_walk(ability: &Ability) -> Option<Made> {
    made(
        ability,
        |value| {
            let walk = value.pointer_mut("/Static/mods/0/AddKeyword/Landwalk")?;
            (*walk == json!([{"Type": "Land"}, {"Not": {"Supertype": "Basic"}}])).then_some(())?;
            *walk = json!([{"Subtype": "Swamp"}]);
            Some(())
        },
        vec![rep("swampwalk", "nonbasic landwalk")],
    )
}

/// "When you cast this spell and whenever this creature attacks, ..." from the attack alone.
fn cast_and_attack(ability: &Ability) -> Option<Made> {
    made(
        ability,
        |value| {
            let when = body(value)?.get_mut("when")?;
            let both = json!({"AnyOf": [{"SpellCast": {"what": ["This"], "by": "You"}}, {"Attacks": {"what": ["This"]}}]});
            (*when == both).then_some(())?;
            *when = json!({"Attacks": {"what": ["This"]}});
            Some(())
        },
        vec![rep(
            "Whenever ~ attacks,",
            "When you cast this spell and whenever ~ attacks,",
        )],
    )
}

/// "When you cast your commander, ..." from the cast of a creature spell.
fn cast_commander(ability: &Ability) -> Option<Made> {
    made(
        ability,
        |value| {
            let what = value.pointer_mut("/Triggered/when/SpellCast/what")?;
            (*what == json!(["Commander"])).then_some(())?;
            *what = json!([{"Type": "Creature"}]);
            Some(())
        },
        vec![rep(
            "Whenever you cast a creature spell,",
            "When you cast your commander,",
        )],
    )
}

/// "..., if no creatures died this turn, ..." from the same words for a death.
fn no_creature_died(ability: &Ability) -> Option<Made> {
    made(
        ability,
        |value| {
            let shape = json!({"Compare": {"left": {"Tallied": {"who": "EachPlayer", "what": "CreaturesDied"}}, "op": "Eq", "right": 0}});
            let cond = value.pointer_mut("/Triggered/if")?;
            (*cond == shape).then_some(())?;
            *cond = json!({"Compare": {"left": {"Tallied": {"who": "EachPlayer", "what": "CreaturesDied"}}, "op": "Ge", "right": 1}});
            Some(())
        },
        vec![rep(
            " if a creature died this turn,",
            " if no creatures died this turn,",
        )],
    )
}

/// "for each other attacking creature that shares a creature type with it".
fn shared_type(ability: &Ability) -> Option<Made> {
    made(
        ability,
        |value| {
            let count =
                value.pointer_mut("/Triggered/effects/0/Continuous/mods/0/ModifyPT/power/Count")?;
            let shape = json!(["Attacking", {"Not": {"Is": "Triggering"}}, {"SharesSubtype": "Triggering"}]);
            (*count == shape).then_some(())?;
            *count = json!([{"Type": "Creature"}, "Attacking", "Other"]);
            Some(())
        },
        vec![rep(
            "for each other attacking creature",
            "for each other attacking creature that shares a creature type with it",
        )],
    )
}

/// A creature of the chosen type dies: the twin names a subtype.
fn chosen_died(ability: &Ability) -> Option<Made> {
    made(
        ability,
        |value| {
            let what = value.pointer_mut("/Triggered/when/ZoneChange/what")?;
            (*what == json!([{"Type": "Creature"}, "ChosenSubtype"])).then_some(())?;
            *what = json!([{"Type": "Creature"}, {"Subtype": "Zzz"}]);
            Some(())
        },
        vec![rep(
            "a Zzz creature dies",
            "a creature of the chosen type dies",
        )],
    )
}

/// A creature spell of the chosen type is cast: the twin names a subtype.
fn chosen_cast(ability: &Ability) -> Option<Made> {
    made(
        ability,
        |value| {
            let what = value.pointer_mut("/Triggered/when/SpellCast/what")?;
            (*what == json!([{"Type": "Creature"}, "ChosenSubtype"])).then_some(())?;
            *what = json!([{"Type": "Creature"}, {"Subtype": "Zzz"}]);
            Some(())
        },
        vec![rep(
            "a Zzz creature spell",
            "a creature spell of the chosen type",
        )],
    )
}
