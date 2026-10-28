//! The primitives of the interpreter: one file per `Effect` variant.
//!
//! A file whose effect the engine plays runs it and `Caps` names it (`caps.rs`); the others are stubs that
//! refuse, and a card that uses one is never offered, so a game never meets the refusal. An effect that asks a
//! player (discard, sacrifice, "may", a choice) returns the question while its journal lacks the answer and reads
//! the answer when its step runs again; a modal effect asks nothing, its modes were chosen when it was announced.

mod adapt;
mod add_mana;
mod add_phase;
mod amass;
mod attach;
mod become_copy;
mod become_day_night;
mod become_level;
mod become_monarch;
mod bolster;
mod c10_vp_planechase;
mod cast_from_zone;
mod choose;
mod choose_at_random;
mod connive;
mod continuous;
mod copy_ability;
mod copy_card;
mod copy_spell;
mod crewed;
mod join_attack;
mod retarget;
mod unattach;
mod w2d_villainous;
pub(crate) use continuous::lower;
mod c6_kw_exploit;
mod c6_kw_renown;
mod counter;
mod create_token;
mod deal_damage;
mod delayed_trigger;
mod destroy;
mod discard;
mod done;
mod draw;
mod encode;
mod exile;
mod exile_hidden;
mod exile_top;
mod exile_until;
mod explore;
mod extra_turn;
mod fabricate;
mod fight;
mod flip;
mod flip_coin;
mod gain_life;
mod goad;
mod if_then;
mod learn;
mod look_at_top;
mod lose_life;
mod manifest;
mod may;
mod mill;
mod modal;
mod monstrosity;
mod move_counters;
mod move_to;
mod multiply_counters;
mod myriad_copies;
mod pay;
mod phase_out;
mod prepare;
mod proliferate;
mod put_counters;
mod regenerate;
mod remove_counters;
mod repeat_each;
mod reveal;
mod reveal_until;
mod roll_dice;
mod rule;
mod sacrifice;
mod scry;
mod search;
mod shield;
mod shuffle;
mod surveil;
mod tap;
mod time_travel;
mod transform;
mod untap;
mod vote;
mod w2f_any_counters;
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

/// Runs one effect; a sacrifice or a removal of counters also reports whether it did anything (`done::report`).
pub(super) fn run(cx: &Cx<'_>, effect: &Effect) -> Result<Run, ResolveError> {
    Ok(done::report(cx, effect, dispatch(cx, effect)?))
}

/// Hands the effect to the primitive of its variant.
fn dispatch(cx: &Cx<'_>, effect: &Effect) -> Result<Run, ResolveError> {
    match effect {
        Effect::Draw { .. } => draw::run(cx, effect),
        Effect::Discard { .. } => discard::run(cx, effect),
        Effect::Mill { .. } => mill::run(cx, effect),
        Effect::Scry { .. } => scry::run(cx, effect),
        Effect::LookAtTop { .. } => look_at_top::run(cx, effect),
        Effect::Reveal { .. } => reveal::run(cx, effect),
        Effect::Search { .. } => search::run(cx, effect),
        Effect::Choose { .. } => choose::run(cx, effect),
        Effect::ChooseAtRandom { .. } => choose_at_random::run(cx, effect),
        Effect::Shuffle { .. } => shuffle::run(cx, effect),
        Effect::Move { .. } => move_to::run(cx, effect),
        Effect::Destroy { .. } => destroy::run(cx, effect),
        Effect::Exile { .. } => exile::run(cx, effect),
        Effect::ExileHidden { .. } => exile_hidden::run(cx, effect),
        Effect::Encode { .. } => encode::run(cx, effect),
        Effect::Manifest { .. } | Effect::Cloak { .. } => manifest::run(cx, effect),
        Effect::Sacrifice { .. } => sacrifice::run(cx, effect),
        Effect::Counter { .. } => counter::run(cx, effect),
        Effect::DealDamage { .. } => deal_damage::run(cx, effect),
        Effect::GainLife { .. } => gain_life::run(cx, effect),
        Effect::LoseLife { .. } => lose_life::run(cx, effect),
        Effect::AddMana { .. } => add_mana::run(cx, effect),
        Effect::PutCounters { .. } => put_counters::run(cx, effect),
        Effect::RemoveCounters { .. } => remove_counters::run(cx, effect),
        Effect::MoveCounters { .. } => move_counters::run(cx, effect),
        Effect::MultiplyCounters { .. } => multiply_counters::run(cx, effect),
        Effect::Tap { .. } => tap::run(cx, effect),
        Effect::Untap { .. } => untap::run(cx, effect),
        Effect::PhaseOut { .. } => phase_out::run(cx, effect),
        Effect::Attach { .. } => attach::run(cx, effect),
        Effect::Unattach { .. } => unattach::run(cx, effect),
        Effect::JoinAttack { .. } => join_attack::run(cx, effect),
        Effect::MyriadCopies { .. } => myriad_copies::run(cx, effect),
        Effect::Transform { .. } => transform::run(cx, effect),
        Effect::Flip { .. } => flip::run(cx, effect),
        Effect::Prepare { .. } => prepare::run(cx, effect),
        Effect::Surveil { .. } => surveil::run(cx, effect),
        Effect::Proliferate { .. } => proliferate::run(cx, effect),
        Effect::Goad { .. } => goad::run(cx, effect),
        Effect::Fight { .. } => fight::run(cx, effect),
        Effect::BecomeMonarch { .. } => become_monarch::run(cx, effect),
        Effect::BecomeDayNight { .. } => become_day_night::run(cx, effect),
        Effect::Amass { .. } => amass::run(cx, effect),
        Effect::Monstrosity { .. } => monstrosity::run(cx, effect),
        Effect::Renown { .. } => c6_kw_renown::run(cx, effect),
        Effect::Exploit { .. } => c6_kw_exploit::run(cx, effect),
        Effect::BecomeLevel { .. } => become_level::run(cx, effect),
        Effect::Adapt { .. } => adapt::run(cx, effect),
        Effect::Bolster { .. } => bolster::run(cx, effect),
        Effect::Fabricate { .. } => fabricate::run(cx, effect),
        Effect::Learn { .. } => learn::run(cx, effect),
        Effect::Crewed { .. } => crewed::run(cx, effect),
        Effect::Explore { .. } => explore::run(cx, effect),
        Effect::Connive { .. } => connive::run(cx, effect),
        Effect::ExileUntil { .. } => exile_until::run(cx, effect),
        Effect::RevealUntil { .. } => reveal_until::run(cx, effect),
        Effect::ExileTop { .. } => exile_top::run(cx, effect),
        Effect::CastFromZone { .. } => cast_from_zone::run(cx, effect),
        Effect::TimeTravel { .. } => time_travel::run(cx, effect),
        Effect::ExtraTurn { .. } => extra_turn::run(cx, effect),
        Effect::AddPhase { .. } => add_phase::run(cx, effect),
        Effect::WinGame { .. } => win_game::run(cx, effect),
        Effect::CopySpell { .. } => copy_spell::run(cx, effect),
        Effect::CopyAbility { .. } => copy_ability::run(cx, effect),
        Effect::CopyCard { .. } => copy_card::run(cx, effect),
        Effect::BecomeCopy { .. } => become_copy::run(cx, effect),
        Effect::CreateToken { .. } => create_token::run(cx, effect),
        Effect::Continuous { .. } => continuous::run(cx, effect),
        Effect::Rule { .. } => rule::run(cx, effect),
        Effect::Regenerate { .. } => regenerate::run(cx, effect),
        Effect::Shield { .. } => shield::run(cx, effect),
        Effect::DelayedTrigger { .. } => delayed_trigger::run(cx, effect),
        Effect::If { .. } => if_then::run(cx, effect),
        Effect::May { .. } => may::run(cx, effect),
        Effect::Pay { .. } => pay::run(cx, effect),
        Effect::Modal { .. } => modal::run(cx, effect),
        Effect::RepeatEach { .. } => repeat_each::run(cx, effect),
        Effect::RollDice { .. } => roll_dice::run(cx, effect),
        Effect::FlipCoin { .. } => flip_coin::run(cx, effect),
        Effect::Vote { .. } => vote::run(cx, effect),
        Effect::Planechase { .. } => c10_vp_planechase::run(cx, effect),
        Effect::VillainousChoice { .. } => w2d_villainous::run(cx, effect),
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
        {"Choose":{"from":"This","as":"chosen"}}, {"ChooseAtRandom":{"from":"This","as":"picked"}}, {"Shuffle":{}}, {"Move":{"what":"This","to":"Hand"}},
        {"Destroy":{"what":"This"}}, {"Exile":{"what":"This"}}, {"Sacrifice":{"filter":[]}},
        {"Counter":{"what":"This"}}, {"DealDamage":{"to":"You","amount":1}}, {"GainLife":{"amount":1}},
        {"LoseLife":{"amount":1}}, {"AddMana":{"mana":{"Fixed":["Green"]}}},
        {"PutCounters":{"on":"This","counter":"+1/+1"}}, {"RemoveCounters":{"from":"This","counter":"+1/+1"}},
        {"MoveCounters":{"from":"This","to":"This","counter":"+1/+1"}}, {"MultiplyCounters":{"on":"This"}}, {"Tap":{"what":"This"}},
        {"Untap":{"what":"This"}}, {"PhaseOut":{"what":"This"}}, {"Attach":{"what":"This","to":"This"}}, {"Unattach":{"what":"This"}}, {"MyriadCopies":{"of":"This"}}, {"Transform":{"what":"This"}}, {"Flip":{"what":"This"}}, {"Prepare":{"what":"This"}}, {"WinGame":{}}, {"AddPhase":{"phases":["Combat","Main"]}},
        {"Surveil":{}}, {"Proliferate":{}}, {"Goad":{"what":"This"}}, {"Fight":{"what":"This","with":"This"}},
        {"BecomeMonarch":{}}, {"BecomeDayNight":{"to":"Night"}}, {"Amass":{"subtype":"Orc","count":1}}, {"Monstrosity":{"count":1}}, {"Renown":{"count":1}}, {"Exploit":{}}, {"BecomeLevel":{"level":2}},
        {"Adapt":{"count":1}}, {"Bolster":{"count":1}}, {"Fabricate":{"count":1}}, {"Learn":{}}, {"Crewed":{}}, {"Explore":{"what":"This"}}, {"Connive":{"what":"This"}},
        {"ExileUntil":{"filter":[],"as":"found","exiled_as":"exiled"}}, {"RevealUntil":{"filter":[],"as":"found","revealed_as":"revealed"}}, {"ExileTop":{"as":"top"}}, {"ExileHidden":{"what":"This"}}, {"Encode":{"what":"This","on":"This"}}, {"Manifest":{}}, {"Cloak":{}}, {"CastFromZone":{"what":"This"}}, {"TimeTravel":{}}, {"ExtraTurn":{}},
        {"CopySpell":{"what":"This"}}, {"CopyAbility":{"what":"This"}}, {"CopyCard":{"what":"This","as":"copy"}},
        {"BecomeCopy":{"what":"This","of":"This","duration":"UntilEndOfTurn"}},
        {"CreateToken":{"token":{"Named":"treasure"}}},
        {"Continuous":{"affected":"This","mods":[{"ModifyPT":{"power":1,"toughness":1}}],"duration":"UntilEndOfTurn"}},
        {"Rule":{"rule":{"CantGainLife":"EachPlayer"},"duration":"UntilEndOfTurn"}},
        {"Regenerate":{"what":"This"}},
        {"Shield":{"when":{"Damage":{}},"by":"Prevent","duration":"UntilEndOfTurn"}},
        {"DelayedTrigger":{"when":{"StepBegins":{"step":"Upkeep","whose":"You"}},"effects":[]}},
        {"If":{"condition":"YourTurn","then":[]}}, {"May":{"effects":[]}}, {"Pay":{"cost":"{1}"}},
        {"Modal":{"modes":[]}}, {"RepeatEach":{"over":"EachPlayer","as":"each","effects":[{"Draw":{}}]}},
        {"RollDice":{"sides":6,"results":[{"from":1,"effects":[]}]}}, {"FlipCoin":{"win":[]}},
        {"Vote":{"options":{"Named":["time","knowledge"]}}}, {"Planechase":{"act":"Planeswalk"}},
        {"VillainousChoice":{"who":"Opponents","first":[],"second":[]}}
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
        assert_eq!(effects.len(), 82, "one sample for each variant of Effect");
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
        assert_eq!(running, PLAYED);
    }

    /// The effects the interpreter plays, in the order of the samples.
    const PLAYED: &[&str] = &[
        "Effect.Draw",
        "Effect.Discard",
        "Effect.Mill",
        "Effect.Scry",
        "Effect.LookAtTop",
        "Effect.Reveal",
        "Effect.Search",
        "Effect.Choose",
        "Effect.ChooseAtRandom",
        "Effect.Shuffle",
        "Effect.Move",
        "Effect.Destroy",
        "Effect.Exile",
        "Effect.Sacrifice",
        "Effect.Counter",
        "Effect.DealDamage",
        "Effect.GainLife",
        "Effect.LoseLife",
        "Effect.AddMana",
        "Effect.PutCounters",
        "Effect.RemoveCounters",
        "Effect.MoveCounters",
        "Effect.MultiplyCounters",
        "Effect.Tap",
        "Effect.Untap",
        "Effect.PhaseOut",
        "Effect.Attach",
        "Effect.Unattach",
        "Effect.MyriadCopies",
        "Effect.Transform",
        "Effect.Flip",
        "Effect.Prepare",
        "Effect.WinGame",
        "Effect.AddPhase",
        "Effect.Surveil",
        "Effect.Proliferate",
        "Effect.Goad",
        "Effect.Fight",
        "Effect.BecomeMonarch",
        "Effect.BecomeDayNight",
        "Effect.Amass",
        "Effect.Monstrosity",
        "Effect.Renown",
        "Effect.Exploit",
        "Effect.BecomeLevel",
        "Effect.Adapt",
        "Effect.Bolster",
        "Effect.Fabricate",
        "Effect.Learn",
        "Effect.Crewed",
        "Effect.Explore",
        "Effect.Connive",
        "Effect.ExileUntil",
        "Effect.RevealUntil",
        "Effect.ExileTop",
        "Effect.ExileHidden",
        "Effect.Encode",
        "Effect.Manifest",
        "Effect.Cloak",
        "Effect.CastFromZone",
        "Effect.TimeTravel",
        "Effect.ExtraTurn",
        "Effect.CopySpell",
        "Effect.CopyAbility",
        "Effect.CopyCard",
        "Effect.BecomeCopy",
        "Effect.CreateToken",
        "Effect.Continuous",
        "Effect.Rule",
        "Effect.Regenerate",
        "Effect.Shield",
        "Effect.DelayedTrigger",
        "Effect.If",
        "Effect.May",
        "Effect.Pay",
        "Effect.Modal",
        "Effect.RepeatEach",
        "Effect.RollDice",
        "Effect.FlipCoin",
        "Effect.Vote",
        "Effect.Planechase",
        "Effect.VillainousChoice",
    ];
}
