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
mod bind_ref;
mod bolster;
mod c10_vp_planechase;
mod cast_from_zone;
mod change_targets;
mod choose;
mod choose_at_random;
mod choose_value;
mod conjure;
mod connive;
mod continuous;
mod control_player;
mod copy_ability;
mod copy_card;
mod copy_spell;
mod crewed;
mod join_attack;
mod l31a_performed;
mod let_amount;
mod r110_moves;
pub(crate) mod r110_remove_put;
mod r114_copy_onto;
mod r282_plot;
mod r306_add_step;
mod r370_balance;
mod r370_bound;
mod r5521_reduce_speed;
mod r6401_pass_control;
mod r6403_aura_swap;
mod r6403_meld;
mod r6501_redistribute;
mod r6602_create_attached;
mod r6702_distribute;
mod r6702_per_vote;
mod retarget;
mod ring_tempts;
mod rmt2_shares;
mod unattach;
mod w2d_villainous;
mod w6e_exchange;
pub(crate) use continuous::lower;

/// The name under which the first step of a resolution binds whether it was done ("you may ...", "you may pay ...").
pub(crate) fn first_step_done() -> String {
    may::performed_name(&[0])
}
mod c6_kw_exploit;
mod c6_kw_renown;
mod counter;
mod create_token;
mod deal_damage;
mod delayed_trigger;
mod destroy;
mod discard;
mod dk1c5_either;
mod done;
mod draw;
mod encode;
mod end_turn;
#[cfg(test)]
mod ep21_move_any;
mod ep22_one_kind;
mod ep22_two_coins;
mod exile;
mod exile_hidden;
mod exile_top;
mod exile_until;
mod explore;
mod extra_turn;
mod fabricate;
mod fe1n_damage;
mod fe1n_game;
mod fe1n_life;
mod fe1n_piles;
mod fe1n_repeat_while;
mod fight;
mod flip;
mod flip_coin;
mod gain_life;
mod goad;
mod if_then;
mod l292_flips_won;
mod l292_put_or_remove;
mod l294_enlist;
mod l32a_pay_times;
mod l32a_reattack;
mod learn;
mod look_at_hand;
mod look_at_top;
mod lose_life;
mod manifest;
mod may;
mod mill;
mod modal;
mod monstrosity;
mod move_counters;
mod moved_as;
mod one_of;
mod pair;
mod r111_order_graveyard;
mod r112_double_x;
mod r301b_exchange_toughness;
mod r372_gain_suspend;
mod r404_look_face_down;
mod r406_actions;
mod r5508_choose_within;
mod r5508_empower;
mod reflexive;
mod remember;
mod restart_game;
mod rmt8_turn_down;
mod spell_enters_with;
mod turn_up;
// dk1b
mod dk1c5_unlock;
mod dk27_bid;
mod dk27_numbers;
mod g12b_counters_of;
#[cfg(test)]
mod move_attached;
mod move_to;
mod multiply_counters;
mod myriad_copies;
mod pay;
mod phase_out;
mod prepare;
mod proliferate;
mod put_counters;
mod r110_secret;
mod r111_keep;
mod r112_dice;
mod r112_guess;
mod r112_note;
mod r112_note_chosen;
mod r112_piles;
mod r114_store;
mod r115_blocks;
mod r115_skip_step;
mod r281_lock_door;
mod r281_pay_one_of;
mod r376_choose_words;
mod r378_parade;
mod r5501_echo_rather;
mod r5509_face_down_pile;
mod r5509_foretold;
mod r6102_random_counter;
mod r6400_block;
/// The engine's step for the step a card names.
pub(crate) use r306_add_step::engine_step as r115_engine_step;
mod r116_assimilate;
pub(crate) mod r6402_change_roll;
pub(crate) mod r6502_enters_as;
mod r6701_remove_among;
mod regenerate;
mod remove_counters;
mod remove_from_combat;
mod repeat_each;
mod rephase;
mod reveal;
mod reveal_until;
mod revive;
mod rmt1_become_blocked;
mod rmt1_beneath;
mod rmt1_unspent;
pub(crate) mod roll_dice;
mod rule;
mod sacrifice;
mod scry;
mod search;
mod search_zones;
mod seek;
mod shield;
mod shuffle;
mod surveil;
mod suspect;
mod tap;
#[cfg(test)]
mod tap_bound_tests;
mod time_travel;
mod transform;
mod un33_attractions;
mod un34_stickers;
mod untap;
#[cfg(test)]
mod untap_stun_tests;
mod vote;
mod w11d_venture;
mod w15a_take_initiative;
mod w2f_any_counters;
mod w5d_tap_or_untap;
mod w6b_create_emblem;
mod w6c_pay_x;
mod w7c_incubate;
mod w9b_clash;
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
    let run = moved_as::bind(cx, effect, dispatch(cx, effect)?);
    Ok(done::report(cx, effect, run))
}

/// The game, life, damage, repeat, pile, kept-number and copy effects.
fn fe1n(cx: &Cx<'_>, effect: &Effect) -> Result<Run, ResolveError> {
    match effect {
        Effect::LoseGame { .. } => fe1n_game::lose(cx, effect),
        Effect::DrawGame {} => fe1n_game::draw(cx, effect),
        Effect::SetLife { .. } => fe1n_life::set(cx, effect),
        Effect::ExchangeLife { .. } => fe1n_life::exchange(cx, effect),
        Effect::ExchangeLifeWithToughness { .. } => r301b_exchange_toughness::run(cx, effect),
        Effect::RemoveDamage { .. } => fe1n_damage::run(cx, effect),
        Effect::RepeatWhile { .. } => fe1n_repeat_while::run(cx, effect),
        Effect::SeparatePiles { .. } => fe1n_piles::run(cx, effect),
        Effect::Balance { .. } => r370_balance::run(cx, effect),
        Effect::LookAtHand { .. } => look_at_hand::run(cx, effect),
        Effect::LookAtFaceDown { .. } => r404_look_face_down::run(cx, effect),
        Effect::Let { .. } => let_amount::run(cx, effect),
        Effect::Bind { .. } => bind_ref::run(cx, effect),
        Effect::CopySpell { .. } => copy_spell::run(cx, effect),
        Effect::ChangeTargets { .. } => change_targets::run(cx, effect),
        Effect::CopyAbility { .. } => copy_ability::run(cx, effect),
        Effect::CopyCard { .. } => copy_card::run(cx, effect),
        Effect::BecomeCopy { .. } => become_copy::run(cx, effect),
        _ => Err(wrong_effect("dispatch")),
    }
}

/// Hands the effect to the primitive of its variant.
#[allow(clippy::too_many_lines)] // one arm per effect
fn dispatch(cx: &Cx<'_>, effect: &Effect) -> Result<Run, ResolveError> {
    match effect {
        Effect::Draw { .. } => draw::run(cx, effect),
        Effect::Discard { .. } => discard::run(cx, effect),
        Effect::Mill { .. } => mill::run(cx, effect),
        Effect::Scry { .. } => scry::run(cx, effect),
        Effect::LookAtTop { .. } => look_at_top::run(cx, effect),
        Effect::Reveal { .. } => reveal::run(cx, effect),
        Effect::Search {
            zone: mtg_cards::refs::ZoneKind::Outside,
            bind,
            ..
        } => Ok(search::nothing_outside(bind)),
        Effect::Search { .. } => search::run(cx, effect),
        Effect::Choose { .. } => choose::run(cx, effect),
        Effect::ChooseAtRandom { .. } => choose_at_random::run(cx, effect),
        Effect::PutRandomCounter { .. } => r6102_random_counter::run(cx, effect),
        Effect::ChooseValue { .. } => choose_value::run(cx, effect),
        Effect::Shuffle { .. } => shuffle::run(cx, effect),
        Effect::Move { .. } => move_to::run(cx, effect),
        Effect::Destroy { .. } => destroy::run(cx, effect),
        Effect::Exile { .. } => exile::run(cx, effect),
        Effect::ExileHidden { .. } => exile_hidden::run(cx, effect),
        Effect::Encode { .. } | Effect::Haunt { .. } => encode::run(cx, effect),
        Effect::Remember { .. } | Effect::Forget { .. } | Effect::RememberValue { .. } => {
            remember::run(cx, effect)
        }
        Effect::Manifest { .. } | Effect::Cloak { .. } => manifest::run(cx, effect),
        Effect::FaceDownPile { .. } => r5509_face_down_pile::run(cx, effect),
        Effect::BecomeForetold { .. } => r5509_foretold::run(cx, effect),
        Effect::Sacrifice { .. } | Effect::SacrificeUpTo { .. } => sacrifice::run(cx, effect),
        Effect::Counter { .. } => counter::run(cx, effect),
        Effect::DealDamage { .. } => deal_damage::run(cx, effect),
        Effect::GainLife { .. } => gain_life::run(cx, effect),
        Effect::LoseLife { .. } => lose_life::run(cx, effect),
        Effect::AddMana { .. } => add_mana::run(cx, effect),
        Effect::PutCounters { .. } => put_counters::run(cx, effect),
        Effect::RemoveCounters { .. } => remove_counters::run(cx, effect),
        Effect::RemoveAllCounters { .. } => remove_counters::run_all(cx, effect),
        Effect::R6701RemoveAmong { .. } => r6701_remove_among::run(cx, effect),
        Effect::MoveCounters { .. } => move_counters::run(cx, effect),
        Effect::SpellEntersWith { .. } => spell_enters_with::run(cx, effect),
        Effect::MultiplyCounters { .. } => multiply_counters::run(cx, effect),
        Effect::PutCountersOf { .. } => g12b_counters_of::run(cx, effect),
        Effect::Tap { .. } => tap::run(cx, effect),
        Effect::Untap { .. } => untap::run(cx, effect),
        Effect::TapOrUntap { .. } => w5d_tap_or_untap::run(cx, effect),
        Effect::PhaseOut { .. } => phase_out::run(cx, effect),
        Effect::R113PhaseIn { .. } => phase_out::r113_run_in(cx, effect),
        Effect::R114CopySpellOnto { .. } => r114_copy_onto::run(cx, effect),
        Effect::Rephase => rephase::run(cx, effect),
        Effect::Attach { .. } => attach::run(cx, effect),
        Effect::Unattach { .. } => unattach::run(cx, effect),
        Effect::JoinAttack { .. } => join_attack::run(cx, effect),
        Effect::MyriadCopies { .. } => myriad_copies::run(cx, effect),
        Effect::Transform { .. } => transform::run(cx, effect),
        Effect::Flip { .. } => flip::run(cx, effect),
        Effect::TurnFaceUp { .. } => turn_up::run(cx, effect),
        Effect::TurnFaceDown { .. } | Effect::R6701TurnFaceDownHorror { .. } => {
            rmt8_turn_down::run(cx, effect)
        }
        Effect::Harness { .. } => rmt8_turn_down::harness(cx, effect),
        Effect::Plot { .. } => r282_plot::run(cx, effect),
        Effect::Prepare { .. } => prepare::run(cx, effect),
        Effect::Unprepare { .. } => prepare::run_unprepare(cx, effect),
        Effect::Suspect { .. } | Effect::Unsuspect { .. } | Effect::Solve { .. } => {
            suspect::run(cx, effect)
        }
        Effect::Surveil { .. } => surveil::run(cx, effect),
        Effect::Proliferate { .. } => proliferate::run(cx, effect),
        Effect::Goad { .. } => goad::run(cx, effect),
        Effect::Fight { .. } => fight::run(cx, effect),
        Effect::BecomeMonarch { .. } => become_monarch::run(cx, effect),
        Effect::ReduceSpeed { .. } => r5521_reduce_speed::run(cx, effect),
        Effect::ControlPlayer { .. } | Effect::R6701ControlCombat { .. } => {
            control_player::run(cx, effect)
        }
        Effect::RestartGame { .. } => restart_game::run(cx, effect),
        Effect::RingTempts { .. } => ring_tempts::run(cx, effect),
        Effect::CreateEmblem { .. } => w6b_create_emblem::run(cx, effect),
        Effect::Revive { .. } => revive::run(cx, effect),
        Effect::BecomeDayNight { .. } => become_day_night::run(cx, effect),
        Effect::Amass { .. } => amass::run(cx, effect),
        Effect::Empower { .. } => r5508_empower::run(cx, effect),
        Effect::ChooseWithin { .. } => r5508_choose_within::run(cx, effect),
        Effect::Incubate { .. } => w7c_incubate::run(cx, effect),
        Effect::Conjure { .. } => conjure::run(cx, effect),
        Effect::Meld { .. } => r6403_meld::run(cx, effect),
        Effect::AuraSwap => r6403_aura_swap::run(cx, effect),
        Effect::R6501RedistributeLife => r6501_redistribute::run(cx, effect),
        Effect::Seek { .. } => seek::run(cx, effect),
        Effect::Venture { .. } => w11d_venture::run(cx, effect),
        Effect::OpenAttraction { .. } | Effect::RollToVisit { .. } => {
            un33_attractions::run(cx, effect)
        }
        Effect::PutSticker { .. } | Effect::ClaimPrize { .. } => un34_stickers::run(cx, effect),
        Effect::Parade { .. } => r378_parade::run(cx, effect),
        Effect::UnlockDoor { .. } => dk1c5_unlock::run(cx, effect),
        Effect::LockOrUnlockDoor { .. } => r281_lock_door::run(cx, effect),
        Effect::TakeInitiative { .. } => w15a_take_initiative::run(cx, effect),
        Effect::Monstrosity { .. } => monstrosity::run(cx, effect),
        Effect::Renown { .. } => c6_kw_renown::run(cx, effect),
        Effect::Exploit { .. } => c6_kw_exploit::run(cx, effect),
        Effect::Enlist { .. } => l294_enlist::run(cx, effect),
        Effect::Pair { .. } => pair::run(cx, effect),
        Effect::BecomeLevel { .. } => become_level::run(cx, effect),
        Effect::Adapt { .. } => adapt::run(cx, effect),
        Effect::Bolster { .. } => bolster::run(cx, effect),
        Effect::Fabricate { .. } => fabricate::run(cx, effect),
        Effect::Learn { .. } => learn::run(cx, effect),
        Effect::R111OrderGraveyard { .. } => r111_order_graveyard::run(cx, effect),
        Effect::R112DoubleX { .. } => r112_double_x::run(cx, effect),
        Effect::Performed { .. } => l31a_performed::run(cx, effect),
        Effect::Crewed { .. } => crewed::run(cx, effect),
        Effect::Explore { .. } => explore::run(cx, effect),
        Effect::Connive { .. } => connive::run(cx, effect),
        Effect::ExileUntil { .. } => exile_until::run(cx, effect),
        Effect::RevealUntil { .. } => reveal_until::run(cx, effect),
        Effect::ExileTop { .. } => exile_top::run(cx, effect),
        Effect::CastFromZone { .. } => cast_from_zone::run(cx, effect),
        Effect::TimeTravel { .. } => time_travel::run(cx, effect),
        Effect::GainSuspend { .. } => r372_gain_suspend::run(cx, effect),
        Effect::ExtraTurn { .. } => extra_turn::run(cx, effect),
        Effect::EndTurn => Ok(end_turn::run(cx)),
        Effect::AddPhase { .. } => add_phase::run(cx, effect),
        Effect::AddStep { .. } => r306_add_step::run(cx, effect),
        Effect::WinGame { .. } => win_game::run(cx, effect),
        Effect::CreateToken { .. } => create_token::run(cx, effect),
        Effect::R6602CreateAttached { .. } => r6602_create_attached::run(cx, effect),
        Effect::Continuous { .. } => continuous::run(cx, effect),
        Effect::ExchangeControl { .. } => w6e_exchange::run(cx, effect),
        Effect::R6401PassControl { .. } => r6401_pass_control::run(cx, effect),
        Effect::R6702Distribute { .. } => r6702_distribute::run(cx, effect),
        Effect::R6702CountersPerVote { .. } => r6702_per_vote::run(cx, effect),
        Effect::R110RemovePutThisWay { .. } => r110_remove_put::run(cx, effect),
        Effect::R110AddCombatAfterFirst => Ok(crate::r110_after_first_combat::r110_run(cx)),
        Effect::R110MoveEachKindMissing { .. } => r110_moves::run_each_kind(cx, effect),
        Effect::R110MoveOneOrMore { .. } => r110_moves::run_one_or_more(cx, effect),
        Effect::R110ChooseSecretly(_) => r110_secret::run(cx, effect),
        Effect::R111KeepChosen(_) => r111_keep::run(cx, effect),
        Effect::R112NoteManaSpent => r112_note::run(cx, effect),
        Effect::R112NoteChosen(_) => r112_note_chosen::run(cx, effect),
        Effect::Rule { .. } => rule::run(cx, effect),
        Effect::Regenerate { .. } => regenerate::run(cx, effect),
        Effect::R116Assimilate { .. } => r116_assimilate::run(cx, effect),
        Effect::RemoveFromCombat { .. } => remove_from_combat::run(cx, effect),
        Effect::R6400Block { .. } => r6400_block::run(cx, effect),
        Effect::R115ExchangeBlocks { .. } => r115_blocks::run_exchange_blocks(cx, effect),
        Effect::R115ExchangeBlockers { .. } => r115_blocks::run_exchange_blockers(cx, effect),
        Effect::R115JoinBlock { .. } => r115_blocks::run_join_block(cx, effect),
        Effect::R115SkipStep { .. } => r115_skip_step::run(cx, effect),
        Effect::BecomeBlocked { .. } => rmt1_become_blocked::run(cx, effect),
        Effect::PutBeneathTop { .. } => rmt1_beneath::run(cx, effect),
        Effect::LoseUnspentMana { .. } => rmt1_unspent::run(cx, effect),
        Effect::Shield { .. } => shield::run(cx, effect),
        Effect::DelayedTrigger { .. } => delayed_trigger::run(cx, effect),
        Effect::Reflexive { .. } => reflexive::run(cx, effect),
        Effect::If { .. } => if_then::run(cx, effect),
        Effect::Together { effects } => Ok(Run::Done {
            events: Vec::new(),
            bound: Vec::new(),
            snapshots: Vec::new(),
            next: if effects.is_empty() {
                crate::resolve::Next::Sibling
            } else {
                crate::resolve::Next::Enter(0)
            },
        }),
        Effect::May { .. } => may::run(cx, effect),
        Effect::Pay { .. } => pay::run(cx, effect),
        Effect::PayAnyTimes { .. } => l32a_pay_times::run(cx, effect),
        Effect::Reattack { .. } => l32a_reattack::run(cx, effect),
        Effect::StillResolves => Ok(Run::events(Vec::new())),
        Effect::Modal { .. } => modal::run(cx, effect),
        Effect::RepeatEach { .. } => repeat_each::run(cx, effect),
        Effect::RollDice { .. } => roll_dice::run(cx, effect),
        Effect::FlipCoin { .. } => flip_coin::run(cx, effect),
        Effect::Clash { .. } => w9b_clash::run(cx, effect),
        Effect::Vote { .. } => vote::run(cx, effect),
        Effect::ChooseNumbers { .. } => dk27_numbers::run(cx, effect),
        Effect::ChooseWords { .. } => r376_choose_words::run(cx, effect),
        Effect::R112GuessNumber { .. } => r112_guess::run(cx, effect),
        Effect::R112Piles { .. } => r112_piles::run(cx, effect),
        Effect::R112RollStore { .. } => r112_dice::run_store(cx, effect),
        Effect::R112RerollStored { .. } => r112_dice::run_reroll(cx, effect),
        Effect::R114StoreValue { .. } => r114_store::run(cx, effect),
        Effect::BidLife { .. } => dk27_bid::run(cx, effect),
        Effect::Planechase { .. } => c10_vp_planechase::run(cx, effect),
        Effect::VillainousChoice { .. } => w2d_villainous::run(cx, effect),
        Effect::Either { .. } | Effect::OneOf { .. } => choice_of_lists(cx, effect),
        other => fe1n(cx, other),
    }
}

/// A choice among lists of effects: two printed options ("or") or several.
fn choice_of_lists(cx: &Cx<'_>, effect: &Effect) -> Result<Run, ResolveError> {
    match effect {
        Effect::OneOf { .. } => one_of::run(cx, effect),
        _ => dk1c5_either::run(cx, effect),
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
        {"Draw":{}}, {"Discard":{}}, {"Mill":{}}, {"Scry":{}}, {"LookAtTop":{"as":"top"}}, {"LookAtHand":{"of":"You"}}, {"LookAtFaceDown":{"what":"This"}},
        {"Reveal":{"what":"This"}}, {"Search":{"filter":[],"as":"found"}},
        {"Choose":{"from":"This","as":"chosen"}}, {"ChooseAtRandom":{"from":"This","as":"picked"}}, {"ChooseValue":{"kind":"Color","as":"hue"}}, {"Shuffle":{}}, {"Move":{"what":"This","to":"Hand"}},
        {"Destroy":{"what":"This"}}, {"Exile":{"what":"This"}}, {"Sacrifice":{"filter":[]}}, {"SacrificeUpTo":{"filter":[],"count":2}}, {"Balance":{"what":"Hand"}},
        {"Counter":{"what":"This"}}, {"DealDamage":{"to":"You","amount":1}}, {"GainLife":{"amount":1}},
        {"LoseLife":{"amount":1}}, {"AddMana":{"mana":{"Fixed":["Green"]}}},
        {"PutCounters":{"on":"This","counter":"+1/+1"}}, {"RemoveCounters":{"from":"This","counter":"+1/+1"}},
        {"MoveCounters":{"from":"This","to":"This","counter":"+1/+1"}}, {"SpellEntersWith":{"spell":"This"}}, {"MultiplyCounters":{"on":"This"}}, {"PutCountersOf":{"on":"This","of":"This"}}, {"Tap":{"what":"This"}},
        {"Untap":{"what":"This"}}, {"TapOrUntap":{"what":"This"}}, {"PhaseOut":{"what":"This"}}, {"Attach":{"what":"This","to":"This"}}, {"Unattach":{"what":"This"}}, {"Pair":{"what":"This","with":"This"}}, {"MyriadCopies":{"of":"This"}}, {"Transform":{"what":"This"}}, {"Flip":{"what":"This"}}, {"Prepare":{"what":"This"}}, {"Unprepare":{"what":"This"}}, {"WinGame":{}}, {"LoseGame":{}}, {"DrawGame":{}}, {"SetLife":{"amount":20}}, {"Revive":{"life":10}}, {"ExchangeLife":{"with":"Opponents"}}, {"ExchangeLifeWithToughness":{"what":"This"}}, {"RemoveDamage":{"what":"This"}}, {"RepeatWhile":{"condition":"YourTurn","effects":[{"Draw":{}}],"most":3}}, {"SeparatePiles":{"what":"This","chooser":"Opponents","as":"pile_a","rest_as":"pile_b"}}, {"AddPhase":{"phases":["Combat","Main"]}}, {"AddStep":{"step":"Upkeep"}},
        {"TurnFaceDown":{"what":"This"}}, {"Harness":{"what":"This"}}, {"Plot":{"what":"This"}},
        {"Surveil":{}}, {"Proliferate":{}}, {"Goad":{"what":"This"}}, {"Fight":{"what":"This","with":"This"}},
        {"ControlPlayer":{"who":"Opponents"}}, {"RestartGame":{"keep":[]}},
        {"BecomeMonarch":{}}, {"ReduceSpeed":{"who":"You"}}, {"RingTempts":{}}, {"CreateEmblem":{"abilities":[]}}, {"BecomeDayNight":{"to":"Night"}}, {"Amass":{"subtype":"Orc","count":1}}, {"Empower":{"count":1}}, {"ChooseWithin":{"from":"This","within":{"TotalPowerAtMost":1},"as":"kept"}}, {"Incubate":{"count":1}}, {"Conjure":{"name":"Island","zone":"Hand"}}, {"Meld":{"with":[{"Named":"Bruna"}],"into":"Brisela"}}, "AuraSwap", "R6501RedistributeLife", {"Seek":{"filter":[]}}, {"Venture":{}}, {"OpenAttraction":{}}, {"RollToVisit":{}}, {"PutSticker":{"on":[]}}, {"Parade":{"what":[]}}, {"ClaimPrize":{}}, {"TakeInitiative":{}}, {"Monstrosity":{"count":1}}, {"Renown":{"count":1}}, {"Exploit":{}}, {"Enlist":{"what":"This"}}, {"BecomeLevel":{"level":2}},
        {"Adapt":{"count":1}}, {"Bolster":{"count":1}}, {"Fabricate":{"count":1}}, {"Learn":{}}, {"Crewed":{}}, {"Explore":{"what":"This"}}, {"Connive":{"what":"This"}},
        {"ExileUntil":{"filter":[],"as":"found","exiled_as":"exiled"}}, {"RevealUntil":{"filter":[],"as":"found","revealed_as":"revealed"}}, {"ExileTop":{"as":"top"}}, {"ExileHidden":{"what":"This"}}, {"Encode":{"what":"This","on":"This"}}, {"Remember":{"what":"This","slot":"s"}}, {"Forget":{"slot":"s"}}, {"Manifest":{}}, {"Cloak":{}}, {"FaceDownPile":{"what":"This","then":"Manifest"}}, {"BecomeForetold":{"what":"This"}}, {"CastFromZone":{"what":"This"}}, {"TimeTravel":{}}, {"GainSuspend":{"what":"This"}}, {"ExtraTurn":{}}, "EndTurn",
        {"ChangeTargets":{"what":"This","to":"Another"}},
        {"CopySpell":{"what":"This"}}, {"CopyAbility":{"what":"This"}}, {"CopyCard":{"what":"This","as":"copy"}},
        {"BecomeCopy":{"what":"This","of":"This","duration":"UntilEndOfTurn"}},
        {"CreateToken":{"token":{"Named":"treasure"}}},
        {"R6602CreateAttached":{"token":{"Named":"wicked_role"},"to":"This"}},
        {"Continuous":{"affected":"This","mods":[{"ModifyPT":{"power":1,"toughness":1}}],"duration":"UntilEndOfTurn"}},
        {"ExchangeControl":{"what":["This"],"duration":"Forever"}},
        {"R6401PassControl":{"side":"Left","what":[],"pick":"All"}},
        {"R6702Distribute":{"among":"This","counter":{"Other":"stun"},"count":3}},
        {"R6702CountersPerVote":{"tally":"votes","counter":{"Other":"stun"}}},
        {"R110ChooseSecretly":"Opponent"},
        {"R111KeepChosen":"name"}, "R112NoteManaSpent",
        {"R112GuessNumber":{"min":1,"max":5,"guesser":"Opponents","as":"number","guess_as":"guess","guesser_as":"guesser"}}, {"R112Piles":{"what":"This","piles":["left","right"]}}, {"R112RollStore":{"sides":6,"dice":5,"slot":"stored"}}, {"R112RerollStored":{"sides":6,"slot":"stored"}}, {"R114StoreValue":{"value":2,"slot":"stored"}}, {"R113PhaseIn":{"what":"This"}}, {"R114CopySpellOnto":{"what":"This","onto":"This"}},
        {"R112NoteChosen":"name"},
        {"Rule":{"rule":{"CantGainLife":"EachPlayer"},"duration":"UntilEndOfTurn"}},
        {"Regenerate":{"what":"This"}}, {"R116Assimilate":{"what":"This"}}, {"RemoveFromCombat":{"what":"This"}}, {"R6400Block":{"what":"This"}},
        {"R115ExchangeBlocks":{"what":"This"}}, {"R115ExchangeBlockers":{"what":"This"}}, {"R115JoinBlock":{"what":"This","blocking":"This"}}, {"R115SkipStep":{"step":"Draw"}},
        {"BecomeBlocked":{"what":"This"}},
        {"PutBeneathTop":{"what":"This","below":2}},
        {"LoseUnspentMana":{}},
        {"Shield":{"when":{"Damage":{}},"by":"Prevent","duration":"UntilEndOfTurn"}},
        {"DelayedTrigger":{"when":{"StepBegins":{"step":"Upkeep","whose":"You"}},"effects":[]}},
        {"Reflexive":{"effects":[]}},
        {"If":{"condition":"YourTurn","then":[]}}, {"May":{"effects":[]}}, {"Together":{"effects":[]}}, {"Pay":{"cost":"{1}"}}, {"PayAnyTimes":{"cost":"{1}"}}, "StillResolves", "Rephase", {"Reattack":{"what":"This"}},
        {"Modal":{"modes":[]}}, {"RepeatEach":{"over":"EachPlayer","as":"each","effects":[{"Draw":{}}]}},
        {"RollDice":{"sides":6,"results":[{"from":1,"effects":[]}]}}, {"FlipCoin":{"win":[]}}, {"Clash":{}},
        {"Vote":{"options":{"Named":["time","knowledge"]}}}, {"Planechase":{"act":"Planeswalk"}},
        {"VillainousChoice":{"who":"Opponents","first":[],"second":[]}},
        {"Either":{"first":[],"second":[]}},
        {"OneOf":{"options":[[],[],[]]}},
        {"ChooseWords":{"words":["silence","snitch"],"secret":true}}
    ]"#;

    fn tag(effect: &Effect) -> String {
        let value = serde_json::to_value(effect).unwrap();
        value.as_str().map_or_else(
            || {
                value
                    .as_object()
                    .and_then(|map| map.keys().next().cloned())
                    .unwrap()
            },
            str::to_owned,
        )
    }

    #[test]
    fn the_capabilities_name_exactly_the_effects_that_run() {
        let effects: Vec<Effect> = serde_json::from_str(EFFECTS).unwrap();
        assert_eq!(effects.len(), 169, "one sample for each variant of Effect");
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
                Ok(_) | Err(ResolveError::Invalid(_) | ResolveError::Illegal(_)) => true,
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
        "Effect.LookAtHand",
        "Effect.LookAtFaceDown",
        "Effect.Reveal",
        "Effect.Search",
        "Effect.Choose",
        "Effect.ChooseAtRandom",
        "Effect.ChooseValue",
        "Effect.Shuffle",
        "Effect.Move",
        "Effect.Destroy",
        "Effect.Exile",
        "Effect.Sacrifice",
        "Effect.SacrificeUpTo",
        "Effect.Balance",
        "Effect.Counter",
        "Effect.DealDamage",
        "Effect.GainLife",
        "Effect.LoseLife",
        "Effect.AddMana",
        "Effect.PutCounters",
        "Effect.RemoveCounters",
        "Effect.MoveCounters",
        "Effect.SpellEntersWith",
        "Effect.MultiplyCounters",
        "Effect.PutCountersOf",
        "Effect.Tap",
        "Effect.Untap",
        "Effect.TapOrUntap",
        "Effect.PhaseOut",
        "Effect.Attach",
        "Effect.Unattach",
        "Effect.Pair",
        "Effect.MyriadCopies",
        "Effect.Transform",
        "Effect.Flip",
        "Effect.Prepare",
        "Effect.Unprepare",
        "Effect.WinGame",
        "Effect.LoseGame",
        "Effect.DrawGame",
        "Effect.SetLife",
        "Effect.Revive",
        "Effect.ExchangeLife",
        "Effect.ExchangeLifeWithToughness",
        "Effect.RemoveDamage",
        "Effect.RepeatWhile",
        "Effect.SeparatePiles",
        "Effect.AddPhase",
        "Effect.AddStep",
        "Effect.TurnFaceDown",
        "Effect.Harness",
        "Effect.Plot",
        "Effect.Surveil",
        "Effect.Proliferate",
        "Effect.Goad",
        "Effect.Fight",
        "Effect.ControlPlayer",
        "Effect.RestartGame",
        "Effect.BecomeMonarch",
        "Effect.ReduceSpeed",
        "Effect.RingTempts",
        "Effect.CreateEmblem",
        "Effect.BecomeDayNight",
        "Effect.Amass",
        "Effect.Empower",
        "Effect.ChooseWithin",
        "Effect.Incubate",
        "Effect.Conjure",
        "Effect.Meld",
        "Effect.AuraSwap",
        "Effect.R6501RedistributeLife",
        "Effect.Seek",
        "Effect.Venture",
        "Effect.OpenAttraction",
        "Effect.RollToVisit",
        "Effect.PutSticker",
        "Effect.Parade",
        "Effect.ClaimPrize",
        "Effect.TakeInitiative",
        "Effect.Monstrosity",
        "Effect.Renown",
        "Effect.Exploit",
        "Effect.Enlist",
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
        "Effect.Remember",
        "Effect.Forget",
        "Effect.Manifest",
        "Effect.Cloak",
        "Effect.FaceDownPile",
        "Effect.BecomeForetold",
        "Effect.CastFromZone",
        "Effect.TimeTravel",
        "Effect.GainSuspend",
        "Effect.ExtraTurn",
        "Effect.EndTurn",
        "Effect.ChangeTargets",
        "Effect.CopySpell",
        "Effect.CopyAbility",
        "Effect.CopyCard",
        "Effect.BecomeCopy",
        "Effect.CreateToken",
        "Effect.R6602CreateAttached",
        "Effect.Continuous",
        "Effect.ExchangeControl",
        "Effect.R6401PassControl",
        "Effect.R6702Distribute",
        "Effect.R6702CountersPerVote",
        "Effect.R110ChooseSecretly",
        "Effect.R111KeepChosen",
        "Effect.R112NoteManaSpent",
        "Effect.R112GuessNumber",
        "Effect.R112Piles",
        "Effect.R112RollStore",
        "Effect.R112RerollStored",
        "Effect.R114StoreValue",
        "Effect.R113PhaseIn",
        "Effect.R114CopySpellOnto",
        "Effect.R112NoteChosen",
        "Effect.Rule",
        "Effect.Regenerate",
        "Effect.RemoveFromCombat",
        "Effect.R6400Block",
        "Effect.R115ExchangeBlocks",
        "Effect.R115ExchangeBlockers",
        "Effect.R115JoinBlock",
        "Effect.R115SkipStep",
        "Effect.BecomeBlocked",
        "Effect.PutBeneathTop",
        "Effect.LoseUnspentMana",
        "Effect.Shield",
        "Effect.DelayedTrigger",
        "Effect.Reflexive",
        "Effect.If",
        "Effect.May",
        "Effect.Together",
        "Effect.Pay",
        "Effect.PayAnyTimes",
        "Effect.StillResolves",
        "Effect.Rephase",
        "Effect.Reattack",
        "Effect.Modal",
        "Effect.RepeatEach",
        "Effect.RollDice",
        "Effect.FlipCoin",
        "Effect.Clash",
        "Effect.Vote",
        "Effect.Planechase",
        "Effect.VillainousChoice",
        "Effect.Either",
        "Effect.OneOf",
        "Effect.ChooseWords",
    ];
}
