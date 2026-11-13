//! r47-01 wordings written from a twin another writer words: "When ~ enters, each opponent can't cast instant or sorcery
//! spells during that player's next turn." (Azor, the Lawbringer), a prohibition each opponent gets for their own next
//! turn, written from the twin whose one step draws a card.

use std::sync::LazyLock;

use serde_json::json;

use crate::ability::Ability;
use crate::effect::Effect;
use crate::modify::{CastPermission, CastRider, Duration, GameRule, Prohibited};
use crate::refs::{Pred, Ref, ZoneKind};

/// Adds the r47-01 wordings of the ability.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    r4701_their_next_turn(ability, texts);
    r4701_rather_from_graveyard(ability, texts);
    for (shape, text) in R4701_SHAPES.iter() {
        if shape == ability && !texts.iter().any(|known| known == text) {
            texts.push((*text).to_owned());
        }
    }
}

/// Whole abilities of the r47-01 pack whose steps the renderers do not join, as JSON, each with its wording: matched whole,
/// so another count, filter or destination never reads back.
const R4701_WHOLES: [(&str, &str); 2] = [
    // sanwell_avenger_ace#0.1
    (
        r#"{"Triggered":{"when":{"Tapped":{"what":["This"]}},"effects":[{"ExileTop":{"count":6,"as":"top"}},{"Choose":{"from":{"Each":[{"In":"Exile"},{"Is":{"Bound":"top"}},{"Type":"Artifact"},{"AnyOf":[{"Subtype":"Vehicle"},{"Type":"Creature"}]}]},"count":{"min":0,"max":1},"as":"kept"}},{"CastFromZone":{"what":{"Bound":"kept"}}},{"Move":{"what":{"Bound":"top"},"to":{"Library":{"place":"Bottom","order":"Random"}}}}]}}"#,
        r"Whenever ~ becomes tapped, exile the top six cards of your library. You may cast a Vehicle or artifact creature spell from among them. Then put the rest on the bottom of your library in a random order.",
    ),
    // sunbirds_invocation#0.0
    (
        r#"{"Triggered":{"when":{"SpellCast":{"by":"You","what":[{"CastFrom":"Hand"}]}},"effects":[{"LookAtTop":{"count":{"Stat":{"of":"Triggering","stat":"ManaValue"}},"as":"top"}},{"Reveal":{"what":{"Bound":"top"}}},{"Choose":{"from":{"Each":[{"In":"Library"},{"Is":{"Bound":"top"}},{"Not":{"Type":"Land"}},{"Compare":{"stat":"ManaValue","op":"Le","to":{"Stat":{"of":"Triggering","stat":"ManaValue"}}}}]},"count":{"min":0,"max":1},"as":"kept"}},{"CastFromZone":{"what":{"Bound":"kept"},"free":true}},{"Move":{"what":{"Bound":"top"},"to":{"Library":{"place":"Bottom","order":"Random"}}}}]}}"#,
        r"Whenever you cast a spell from your hand, reveal the top X cards of your library, where X is that spell's mana value. You may cast a spell with mana value X or less from among cards revealed this way without paying its mana cost. Put the rest on the bottom of your library in a random order.",
    ),
];

/// The entries of `R4701_WHOLES` as abilities.
static R4701_SHAPES: LazyLock<Vec<(Ability, &str)>> = LazyLock::new(|| {
    R4701_WHOLES
        .iter()
        .filter_map(|(shape, text)| Some((serde_json::from_str(shape).ok()?, *text)))
        .collect()
});

/// "As long as you control a Giant, you may cast this card from your graveyard by paying {U} rather than paying its mana
/// cost. If you cast this card this way and it would be put into your graveyard, exile it instead.": the card's permission
/// from its graveyard for another cost, with the exile of the spell as its rider, written from the twin that pays the mana
/// cost and has no rider.
fn r4701_rather_from_graveyard(ability: &Ability, texts: &mut Vec<String>) {
    let Ability::Rule {
        rule: GameRule::MayCast(permission),
        condition,
        from: ZoneKind::Graveyard,
    } = ability
    else {
        return;
    };
    let Some(rather) = &permission.rather else {
        return;
    };
    if permission.what.0 != [Pred::This]
        || permission.from != Some(ZoneKind::Graveyard)
        || !matches!(
            permission.riders.as_slice(),
            [] | [CastRider::ExiledInstead]
        )
    {
        return;
    }
    let Some(paid) = crate::compile::read::costs::text(rather) else {
        return;
    };
    let twin = Ability::Rule {
        rule: GameRule::MayCast(CastPermission {
            rather: None,
            riders: Vec::new(),
            ..permission.clone()
        }),
        condition: condition.clone(),
        from: ZoneKind::Graveyard,
    };
    let rider = if permission.riders.is_empty() {
        String::new()
    } else {
        " If you cast this card this way and it would be put into your graveyard, exile it instead."
            .to_owned()
    };
    let found: Vec<String> = crate::compile::written_variants(&twin)
        .into_iter()
        .filter(|text| {
            text.matches("from your graveyard").count() == 1
                && text.ends_with("from your graveyard.")
        })
        .map(|text| {
            let text = text.replacen(
                "from your graveyard.",
                &format!("from your graveyard by paying {paid} rather than paying its mana cost."),
                1,
            );
            format!("{text}{rider}")
        })
        .filter(|text| !texts.contains(text))
        .collect();
    texts.extend(found);
}

/// "each opponent can't cast instant or sorcery spells during that player's next turn" for the one step of a triggered
/// ability: a pass over the opponents that bans the spells for that opponent's next turn.
fn r4701_their_next_turn(ability: &Ability, texts: &mut Vec<String>) {
    let Ability::Triggered { effects, .. } = ability else {
        return;
    };
    let [
        Effect::RepeatEach {
            over: Ref::Opponents,
            bind,
            effects: inner,
        },
    ] = effects.as_slice()
    else {
        return;
    };
    let [
        Effect::Rule {
            rule:
                GameRule::Cant {
                    who: Ref::Bound(who),
                    action:
                        Prohibited::Cast {
                            filter,
                            more_than: None,
                            unless: None,
                        },
                },
            duration: Duration::TheirNextTurn,
        },
    ] = inner.as_slice()
    else {
        return;
    };
    if who != bind {
        return;
    }
    // "instant or sorcery spells": a choice of card types, joined by "or".
    let [Pred::AnyOf(kinds)] = filter.0.as_slice() else {
        return;
    };
    let Some(words) = kinds
        .iter()
        .map(|kind| match kind {
            Pred::Type(card_type) => {
                Some(crate::compile::read::selectors::type_name(*card_type).to_owned())
            }
            _ => None,
        })
        .collect::<Option<Vec<String>>>()
    else {
        return;
    };
    let spells = format!("{} spells", words.join(" or "));
    let Ok(mut value) = serde_json::to_value(ability) else {
        return;
    };
    let Some(steps) = value
        .get_mut("Triggered")
        .and_then(|body| body.get_mut("effects"))
    else {
        return;
    };
    *steps = json!([{"Draw": {}}]);
    let Ok(twin) = serde_json::from_value::<Ability>(value) else {
        return;
    };
    let step = format!("each opponent can't cast {spells} during that player's next turn");
    let found: Vec<String> = crate::compile::written_variants(&twin)
        .into_iter()
        .filter(|text| text.matches("draw a card").count() == 1)
        .map(|text| text.replacen("draw a card", &step, 1))
        .filter(|text| !texts.contains(text))
        .collect();
    texts.extend(found);
}

#[cfg(test)]
mod tests {
    //! Goal: every entry of the r47-01 table parses as an ability, so none is silently dropped.
    use super::{R4701_SHAPES, R4701_WHOLES};

    #[test]
    fn r4701_every_whole_parses() {
        assert_eq!(R4701_SHAPES.len(), R4701_WHOLES.len());
    }
}
