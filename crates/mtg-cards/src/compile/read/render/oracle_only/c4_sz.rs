//! Sentences of the fourth sample of the names S to Z: the effects of an ability are matched as data and worded as one
//! sentence after the head the renderers already write ("Whenever a Dragon you control becomes the target of a spell or
//! ability an opponent controls, ~ deals 3 damage to that player."). Each writer returns the sentence, in lower case
//! and without a period, for exactly one shape of effects.

use super::super::super::amounts::count_word;
use super::super::written_variants;
use super::standin::with_effects;
use crate::ability::Ability;
use crate::effect::Effect;
use crate::refs::Ref;
use crate::trigger::{Step, TriggerPattern};
use crate::value::Amount;
use serde_json::{Value, json};

/// A writer of the sentence for the effects of an ability.
type Writer = fn(&Ability) -> Option<String>;

/// The writers, one line each.
const WRITERS: [Writer; 16] = [
    dragon_blow,
    dragon_destroy,
    extra_phase,
    syphon,
    dismissal,
    commander_study,
    returned_later,
    stolen_artifact,
    reckoning,
    exiled_and_drawn,
    counter_four,
    doctor_choice,
    relic_copies,
    discard_bonus,
    discard_unless,
    threshold_instead,
];

/// Adds the wordings of an ability whose effects one of the writers words.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    for writer in WRITERS {
        if let Some(sentence) = writer(ability) {
            let written = with_sentence(ability, &sentence);
            // The ability word before a sentence is flavor: the clause may be read without it.
            texts.extend(
                written
                    .iter()
                    .filter(|text| text.contains("\nThreshold \u{2014} "))
                    .map(|text| text.replacen("Threshold \u{2014} ", "", 1)),
            );
            texts.extend(written);
        }
    }
}

/// The wordings of the ability with `sentence` (no period, in lower case) for its targets and effects, after a head the
/// checker writes too ("When ~ enters, if it was kicked,").
fn with_sentence(ability: &Ability, sentence: &str) -> Vec<String> {
    let plain = with_effects(ability, sentence);
    if !plain.is_empty() {
        return plain;
    }
    let mut stand = ability.clone();
    match &mut stand {
        Ability::Spell { targets, effects }
        | Ability::Activated {
            targets, effects, ..
        }
        | Ability::Triggered {
            targets, effects, ..
        } => {
            targets.clear();
            *effects = vec![Effect::Draw {
                who: Ref::You,
                count: Amount::one(),
            }];
        }
        _ => return Vec::new(),
    }
    written_variants(&stand)
        .iter()
        .filter_map(|text| {
            text.strip_suffix("draw a card.")
                .map(|head| format!("{head}{sentence}."))
        })
        .collect()
}

/// The JSON of a value, which the guards compare whole.
fn data<T: serde::Serialize + ?Sized>(value: &T) -> Value {
    serde_json::to_value(value).unwrap_or(Value::Null)
}

/// The effects of the ability as JSON.
fn effects(ability: &Ability) -> Value {
    data(ability.effects())
}

/// The targets of the ability as JSON.
fn targets(ability: &Ability) -> Value {
    data(ability.targets())
}

/// The event of a triggered ability.
fn event(ability: &Ability) -> Option<&TriggerPattern> {
    match ability {
        Ability::Triggered { when, .. } => Some(when),
        _ => None,
    }
}

/// Whether the ability is a trigger on a permanent becoming a target.
fn on_target(ability: &Ability) -> bool {
    matches!(event(ability), Some(TriggerPattern::BecomesTarget { .. }))
}

/// "~ deals 3 damage to that player" for the controller of the spell or ability that targeted.
fn dragon_blow(ability: &Ability) -> Option<String> {
    let amount = effects(ability).pointer("/0/DealDamage/amount")?.as_u64()?;
    let shape = json!([{"DealDamage": {"source": "This", "to": {"ControllerOf": "Dealer"}, "amount": amount}}]);
    (on_target(ability) && effects(ability) == shape)
        .then(|| format!("~ deals {amount} damage to that player"))
}

/// "destroy target nonland permanent that player controls" for the same controller.
fn dragon_destroy(ability: &Ability) -> Option<String> {
    let spec = json!([{"what": {"Object": [{"Not": {"Type": "Land"}}, {"ControlledBy": {"ControllerOf": "Dealer"}}]}}]);
    let shape = json!([{"Destroy": {"what": {"Target": 0}}}]);
    (on_target(ability) && targets(ability) == spec && effects(ability) == shape)
        .then(|| "destroy target nonland permanent that player controls".to_owned())
}

/// An additional beginning phase after a postcombat main phase.
fn extra_phase(ability: &Ability) -> Option<String> {
    let main = matches!(
        event(ability),
        Some(TriggerPattern::StepBegins {
            step: Step::Main2,
            ..
        })
    );
    (main && effects(ability) == json!([{"AddPhase": {"phases": ["Beginning"]}}]))
        .then(|| "there is an additional beginning phase after this phase".to_owned())
}

/// Every other player discards a card and the controller draws for each card discarded.
fn syphon(ability: &Ability) -> Option<String> {
    let shape = json!([
        {"Discard": {"who": "Opponents", "as": "d"}},
        {"Draw": {"count": {"Count": [{"Is": {"Bound": "d"}}]}}}
    ]);
    (matches!(ability, Ability::Spell { .. }) && effects(ability) == shape).then(|| {
        "each other player discards a card. You draw a card for each card discarded this way"
            .to_owned()
    })
}

/// All other spells exiled and all abilities countered.
fn dismissal(ability: &Ability) -> Option<String> {
    let shape = json!([
        {"Exile": {"what": {"Each": ["Spell", "Other"]}}},
        {"Counter": {"what": {"Each": [{"AnyOf": [{"Ability": "Activated"}, {"Ability": "Triggered"}]}]}}}
    ]);
    (effects(ability) == shape)
        .then(|| "exile all other spells and counter all abilities".to_owned())
}

/// A draw and a loss of the mana value of a commander the controller owns.
fn commander_study(ability: &Ability) -> Option<String> {
    let amount = json!({"Greatest": {"of": ["Commander", {"OwnedBy": "You"}, {"AnyOf": [{"In": "Battlefield"}, {"In": "Command"}]}], "stat": "ManaValue"}});
    let shape = json!([{"Draw": {"count": amount}}, {"LoseLife": {"amount": amount}}]);
    (matches!(ability, Ability::Spell { .. }) && effects(ability) == shape).then(|| {
        "you draw X cards and you lose X life, where X is the mana value of a commander you own on the battlefield or in the command zone".to_owned()
    })
}

/// A card of an event returned at the beginning of the next end step, if its controller wishes.
fn returned_later(ability: &Ability) -> Option<String> {
    let shape = json!([{"May": {"effects": [{"DelayedTrigger": {"when": {"StepBegins": {"step": "End"}}, "effects": [{"Move": {"what": "Triggering", "to": "Hand"}}]}}]}}]);
    let dies = matches!(event(ability), Some(TriggerPattern::ZoneChange { .. }));
    (dies && effects(ability) == shape).then(|| {
        "you may return that card to its owner's hand at the beginning of the next end step"
            .to_owned()
    })
}

/// Control of the artifact an opponent tapped for mana until the end of the controller's next turn.
fn stolen_artifact(ability: &Ability) -> Option<String> {
    let shape = json!([{"Continuous": {"affected": "Triggering", "mods": [{"SetController": "You"}], "duration": "UntilEndOfYourNextTurn"}}]);
    let tapped = matches!(event(ability), Some(TriggerPattern::TappedForMana { what, .. }) if data(what) == json!([{"Type": "Artifact"}]));
    (tapped && effects(ability) == shape)
        .then(|| "gain control of that artifact until the end of your next turn".to_owned())
}

/// Each creature deals damage to itself equal to its power.
fn reckoning(ability: &Ability) -> Option<String> {
    let name = effects(ability)
        .pointer("/0/RepeatEach/as")?
        .as_str()?
        .to_owned();
    let bound = json!({"Bound": name});
    let shape = json!([{"RepeatEach": {"over": {"Each": [{"Type": "Creature"}]}, "as": name, "effects": [
        {"DealDamage": {"source": bound, "to": bound, "amount": {"Stat": {"of": bound, "stat": "Power"}}}}
    ]}}]);
    (matches!(ability, Ability::Spell { .. }) && effects(ability) == shape)
        .then(|| "each creature deals damage to itself equal to its power".to_owned())
}

/// The player of an event exiles cards from the top of their library and the controller draws as many.
fn exiled_and_drawn(ability: &Ability) -> Option<String> {
    let count = effects(ability).pointer("/0/ExileTop/count")?.as_u64()?;
    let name = effects(ability)
        .pointer("/0/ExileTop/as")?
        .as_str()?
        .to_owned();
    let shape = json!([
        {"ExileTop": {"who": {"ControllerOf": "Triggering"}, "count": count, "as": name}},
        {"Draw": {"count": count}}
    ]);
    let words = count_word(u32::try_from(count).ok()?)?;
    (effects(ability) == shape && event(ability).is_some()).then(|| {
        format!(
            "that player exiles the top {words} cards of their library and you draw {words} cards"
        )
    })
}

/// A counter of four kinds of objects on the stack.
fn counter_four(ability: &Ability) -> Option<String> {
    let spec = json!([{"what": {"Either": [
        {"Object": ["Spell", {"Type": "Instant"}]},
        {"Object": ["Spell", {"Type": "Sorcery"}]},
        {"Object": [{"Ability": "Activated"}]},
        {"Object": [{"Ability": "Triggered"}]}
    ]}}]);
    (targets(ability) == spec && effects(ability) == json!([{"Counter": {"what": {"Target": 0}}}]))
        .then(|| {
            "counter target instant spell, sorcery spell, activated ability, or triggered ability"
                .to_owned()
        })
}

/// A choice of one token of three: "create your choice of a Clue token, a Food token, or a Treasure token".
fn doctor_choice(ability: &Ability) -> Option<String> {
    let token = |id: &str| json!({"effects": [{"CreateToken": {"token": {"Named": id}}}]});
    let shape = json!([{"Modal": {"modes": [token("clue"), token("food"), token("treasure")]}}]);
    (matches!(ability, Ability::Triggered { .. }) && effects(ability) == shape)
        .then(|| "create your choice of a Clue token, a Food token, or a Treasure token".to_owned())
}

/// Two tapped tokens that are copies of the card.
fn relic_copies(ability: &Ability) -> Option<String> {
    let shape =
        json!([{"CreateToken": {"token": {"CopyOf": {"of": "This"}}, "count": 2, "tapped": true}}]);
    (effects(ability) == shape).then(|| "create two tapped tokens that are copies of ~".to_owned())
}

/// A draw and a discard, then a bonus of the mana value of the discarded card until end of turn.
fn discard_bonus(ability: &Ability) -> Option<String> {
    let name = effects(ability)
        .pointer("/1/Discard/as")?
        .as_str()?
        .to_owned();
    let bound = json!({"Bound": name});
    let shape = json!([
        {"Draw": {}},
        {"Discard": {"as": name}},
        {"Continuous": {"affected": "This", "mods": [{"ModifyPT": {"power": {"Stat": {"of": bound, "stat": "ManaValue"}}, "toughness": 0}}], "duration": "UntilEndOfTurn"}}
    ]);
    (effects(ability) == shape).then(|| {
        "draw a card, then discard a card. ~ gets +X/+0 until end of turn, where X is the discarded card's mana value".to_owned()
    })
}

/// A draw and a discard that another discard of a kind of card avoids: "Then discard two cards unless you discard an
/// artifact card."
fn discard_unless(ability: &Ability) -> Option<String> {
    let all = effects(ability);
    let drawn_count = all.pointer("/0/Draw/count")?.as_u64()?;
    let card_type = all.pointer("/1/Pay/cost/0/Discard/filter/0/Type")?.clone();
    let thrown_count = all.pointer("/1/Pay/unpaid/0/Discard/count")?.as_u64()?;
    let shape = json!([
        {"Draw": {"count": drawn_count}},
        {"Pay": {"cost": [{"Discard": {"filter": [{"Type": card_type}]}}], "unpaid": [{"Discard": {"count": thrown_count}}]}}
    ]);
    let kind = card_type.as_str()?.to_lowercase();
    let (draw_word, discard_word) = (
        count_word(u32::try_from(drawn_count).ok()?)?,
        count_word(u32::try_from(thrown_count).ok()?)?,
    );
    let article = if kind.starts_with(['a', 'e', 'i', 'o', 'u']) {
        "an"
    } else {
        "a"
    };
    (matches!(ability, Ability::Spell { .. }) && all == shape).then(|| {
        format!("draw {draw_word} cards. Then discard {discard_word} cards unless you discard {article} {kind} card")
    })
}

/// A return to the hand that goes to the battlefield instead when the graveyard holds enough cards.
fn threshold_instead(ability: &Ability) -> Option<String> {
    let size = effects(ability)
        .pointer("/0/If/condition/Compare/right")?
        .as_u64()?;
    let condition = json!({"Compare": {"left": {"Count": [{"In": "Graveyard"}, {"OwnedBy": "You"}]}, "op": "Ge", "right": size}});
    let shape = json!([{"If": {"condition": condition,
        "then": [{"Move": {"what": {"Target": 0}, "to": {"Battlefield": {}}}}],
        "otherwise": [{"Move": {"what": {"Target": 0}, "to": "Hand"}}]}}]);
    let spec = json!([{"what": {"Object": [{"Type": "Creature"}, {"In": "Graveyard"}, {"OwnedBy": "You"}]}}]);
    let words = count_word(u32::try_from(size).ok()?)?;
    (matches!(ability, Ability::Spell { .. }) && targets(ability) == spec && effects(ability) == shape).then(|| {
        format!("return target creature card from your graveyard to your hand.\nThreshold \u{2014} Return that card from your graveyard to the battlefield instead if there are {words} or more cards in your graveyard")
    })
}
