//! The primitives of the interpreter: one file per `Effect` variant.
//!
//! A file whose effect the engine plays runs it and `Caps` names it (`caps.rs`); the others are stubs that
//! refuse, and a card that uses one is never offered, so a game never meets the refusal. An effect that asks a
//! player (discard, sacrifice, "may", a choice) returns the question while its journal lacks the answer and reads
//! the answer when its step runs again; a modal effect asks nothing, its modes were chosen when it was announced.

mod add_mana;
mod choose;
mod continuous;
mod counter;
mod create_token;
mod deal_damage;
mod delayed_trigger;
mod destroy;
mod discard;
mod draw;
mod exile;
mod gain_life;
mod if_then;
mod look_at_top;
mod lose_life;
mod may;
mod mill;
mod modal;
mod move_to;
mod pay;
mod put_counters;
mod reveal;
mod rule;
mod sacrifice;
mod scry;
mod search;
mod shuffle;
mod tap;
mod transform;
mod untap;
mod win_game;

use mtg_cards::effect::Effect;

use super::{Cx, ResolveError, Run};

/// Refuses an effect the engine does not play yet.
fn unsupported(name: &'static str) -> Result<Run, ResolveError> {
    Err(ResolveError::Unsupported(name))
}

/// The error for a primitive that was handed an effect of another variant: a bug in the dispatch.
fn wrong_effect(name: &str) -> ResolveError {
    ResolveError::Invalid(format!("the {name} primitive was given another effect"))
}

/// Runs one effect.
pub(super) fn run(cx: &Cx<'_>, effect: &Effect) -> Result<Run, ResolveError> {
    match effect {
        Effect::Draw { .. } => draw::run(cx, effect),
        Effect::Discard { .. } => discard::run(cx, effect),
        Effect::Mill { .. } => mill::run(cx, effect),
        Effect::Scry { .. } => scry::run(cx, effect),
        Effect::LookAtTop { .. } => look_at_top::run(cx, effect),
        Effect::Reveal { .. } => reveal::run(cx, effect),
        Effect::Search { .. } => search::run(cx, effect),
        Effect::Choose { .. } => choose::run(cx, effect),
        Effect::Shuffle { .. } => shuffle::run(cx, effect),
        Effect::Move { .. } => move_to::run(cx, effect),
        Effect::Destroy { .. } => destroy::run(cx, effect),
        Effect::Exile { .. } => exile::run(cx, effect),
        Effect::Sacrifice { .. } => sacrifice::run(cx, effect),
        Effect::Counter { .. } => counter::run(cx, effect),
        Effect::DealDamage { .. } => deal_damage::run(cx, effect),
        Effect::GainLife { .. } => gain_life::run(cx, effect),
        Effect::LoseLife { .. } => lose_life::run(cx, effect),
        Effect::AddMana { .. } => add_mana::run(cx, effect),
        Effect::PutCounters { .. } => put_counters::run(cx, effect),
        Effect::Tap { .. } => tap::run(cx, effect),
        Effect::Untap { .. } => untap::run(cx, effect),
        Effect::Transform { .. } => transform::run(cx, effect),
        Effect::WinGame { .. } => win_game::run(cx, effect),
        Effect::CreateToken { .. } => create_token::run(cx, effect),
        Effect::Continuous { .. } => continuous::run(cx, effect),
        Effect::Rule { .. } => rule::run(cx, effect),
        Effect::DelayedTrigger { .. } => delayed_trigger::run(cx, effect),
        Effect::If { .. } => if_then::run(cx, effect),
        Effect::May { .. } => may::run(cx, effect),
        Effect::Pay { .. } => pay::run(cx, effect),
        Effect::Modal { .. } => modal::run(cx, effect),
    }
}

#[cfg(test)]
mod tests {
    //! Goal: the capabilities name exactly the effects the interpreter plays: every effect that runs is named, and every other one refuses by its own name, so a card is never offered with a step the engine cannot take.
    use mtg_core::ids::PlayerId;

    use super::*;
    use crate::caps::caps;
    use crate::fixtures::{creature, state};
    use crate::resolve::{Resolving, Scope};
    use crate::zones::Zone;

    /// One effect of every variant of the vocabulary.
    const EFFECTS: &str = r#"[
        {"Draw":{}}, {"Discard":{}}, {"Mill":{}}, {"Scry":{}}, {"LookAtTop":{"as":"top"}},
        {"Reveal":{"what":"This"}}, {"Search":{"filter":[],"as":"found"}},
        {"Choose":{"from":"This","as":"chosen"}}, {"Shuffle":{}}, {"Move":{"what":"This","to":"Hand"}},
        {"Destroy":{"what":"This"}}, {"Exile":{"what":"This"}}, {"Sacrifice":{"filter":[]}},
        {"Counter":{"what":"This"}}, {"DealDamage":{"to":"You","amount":1}}, {"GainLife":{"amount":1}},
        {"LoseLife":{"amount":1}}, {"AddMana":{"mana":{"Fixed":["Green"]}}},
        {"PutCounters":{"on":"This","counter":"+1/+1"}}, {"Tap":{"what":"This"}},
        {"Untap":{"what":"This"}}, {"Transform":{"what":"This"}}, {"WinGame":{}},
        {"CreateToken":{"token":{"Named":"treasure"}}},
        {"Continuous":{"affected":"This","mods":[{"ModifyPT":{"power":1,"toughness":1}}],"duration":"UntilEndOfTurn"}},
        {"Rule":{"rule":"DamageCantBePrevented","duration":"UntilEndOfTurn"}},
        {"DelayedTrigger":{"when":{"StepBegins":{"step":"Upkeep","whose":"You"}},"effects":[]}},
        {"If":{"condition":"YourTurn","then":[]}}, {"May":{"effects":[]}}, {"Pay":{"cost":"{1}"}},
        {"Modal":{"modes":[]}}
    ]"#;

    fn tag(effect: &Effect) -> String {
        serde_json::to_value(effect)
            .unwrap()
            .as_object()
            .and_then(|map| map.keys().next().cloned())
            .unwrap()
    }

    #[test]
    fn the_capabilities_name_exactly_the_effects_that_run() {
        let effects: Vec<Effect> = serde_json::from_str(EFFECTS).unwrap();
        assert_eq!(effects.len(), 31, "one sample for each variant of Effect");
        let game = state(
            vec![vec![], vec![]],
            vec![vec![creature("Bear", 2, 2)], vec![]],
        );
        let bear = game.object(game.zone(Zone::Battlefield)[0]).unwrap();
        let resolving = Resolving::begin(bear.id());
        let scope = Scope::announcing(&game, PlayerId(0), bear.reference());
        let cx = Cx {
            scope,
            resolving: &resolving,
        };
        let mut running = Vec::new();
        for effect in &effects {
            let name = format!("Effect.{}", tag(effect));
            let plays = match run(&cx, effect) {
                Err(ResolveError::Unsupported(refused)) => {
                    assert_eq!(
                        refused, name,
                        "a stub refuses by the name of its own effect"
                    );
                    false
                }
                Ok(_) | Err(ResolveError::Invalid(_)) => true,
            };
            assert_eq!(plays, caps().runs(&name), "{name}");
            if plays {
                running.push(name);
            }
        }
        assert_eq!(
            running,
            [
                "Effect.Draw",
                "Effect.Discard",
                "Effect.Scry",
                "Effect.LookAtTop",
                "Effect.Reveal",
                "Effect.Choose",
                "Effect.Shuffle",
                "Effect.Move",
                "Effect.Destroy",
                "Effect.Exile",
                "Effect.Sacrifice",
                "Effect.Counter",
                "Effect.DealDamage",
                "Effect.GainLife",
                "Effect.LoseLife",
                "Effect.Continuous",
                "Effect.DelayedTrigger",
                "Effect.May",
                "Effect.Pay",
                "Effect.Modal"
            ]
        );
    }
}
