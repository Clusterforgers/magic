//! `Effect::Continuous`: a continuous effect begins (CR 611.2).

use std::sync::Arc;

use mtg_cards::effect::Effect;
use mtg_cards::keyword::Keyword;
use mtg_cards::modify::Modification as Wanted;
use mtg_cards::refs::{Pred, Selector as PickSelector};
use mtg_cards::value::Amount;
use mtg_core::color::Color;
use mtg_core::ids::EffectId;

use super::wrong_effect;
use crate::choose::{ChoiceOption, OptionChooser, Optioned, pick_options};
use crate::effects::{ContinuousEffect, Modification, Selector};
use crate::events::Event;
use crate::object::{Chosen, GameObject, ObjectRef};
use crate::resolve::{Cx, ResolveError, Run, Scope};
use crate::zones::Zone;

/// The colours a player may choose, in the order of the five-colour wheel.
const COLORS: [Color; 5] = [
    Color::White,
    Color::Blue,
    Color::Black,
    Color::Red,
    Color::Green,
];

/// Each modification becomes one effect that applies to the permanents the reference names now and to no other object,
/// whatever happens to the set later (CR 611.2c). The effects take consecutive timestamps in the order of the card's
/// text (CR 613.7), and their amounts are worked out now. Naming no permanent begins nothing. A modification the engine
/// does not run refuses by name.
pub(super) fn run(cx: &Cx<'_>, effect: &Effect) -> Result<Run, ResolveError> {
    let Effect::Continuous {
        affected,
        mods,
        duration,
    } = effect
    else {
        return Err(wrong_effect("Continuous"));
    };
    let state = cx.state();
    let named: Vec<ObjectRef> = cx
        .scope
        .live_objects(affected)?
        .into_iter()
        .filter(|object| object.zone() == Zone::Battlefield)
        .map(GameObject::reference)
        .collect();
    if named.is_empty() {
        return Ok(Run::events(Vec::new()));
    }
    // The colour of "protection from the color of your choice" is chosen as the effect begins, once for each.
    let choosers: Vec<OptionChooser> = mods
        .iter()
        .filter(|wanted| matches!(wanted, Wanted::AddProtectionFromChosenColor))
        .map(|_| OptionChooser {
            player: cx.controller(),
            options: COLORS.iter().copied().map(ChoiceOption::Color).collect(),
            text: "Choose a color.".into(),
        })
        .collect();
    let mut chosen = match pick_options(cx, &choosers)? {
        Optioned::Ask(decision) => return Ok(Run::Ask(decision)),
        Optioned::Done(taken) => taken.into_iter(),
    };
    let mut events = Vec::new();
    for (made, wanted) in (0u64..).zip(mods) {
        let modification = match wanted {
            Wanted::AddProtectionFromChosenColor => {
                let color = chosen
                    .next()
                    .and_then(|(_, index)| COLORS.get(index).copied())
                    .ok_or_else(|| ResolveError::Invalid("no color was chosen".into()))?;
                Modification::AddKeyword(Keyword::Protection(PickSelector(vec![Pred::Color(
                    color,
                )])))
            }
            other => lower(&cx.scope, other)?,
        };
        events.push(Event::EffectCreated {
            effect: ContinuousEffect::new(
                EffectId(state.next_effect + made),
                Selector::Objects(named.clone()),
                modification,
                *duration,
                state.next_timestamp + made,
            )
            .from_source(cx.scope.source)
            .begun(cx.controller(), state.turn().number()),
        });
    }
    Ok(Run::events(events))
}

/// The keyword with the colour or creature type the source chose as it entered written into a protection quality: the
/// holder of a granted protection (an Aura's host, a creature of a lord) made no choice, so "the chosen color" is
/// fixed by the source that grants it.
fn chosen_in(scope: &Scope<'_>, keyword: &Keyword) -> Keyword {
    let Keyword::Protection(quality) = keyword else {
        return keyword.clone();
    };
    let Some(chosen) = scope.entered_choice() else {
        return keyword.clone();
    };
    let fixed = |pred: &Pred| match (pred, chosen) {
        (Pred::ChosenColor, Chosen::Color(color)) => Pred::Color(*color),
        (Pred::ChosenSubtype, Chosen::CreatureType(subtype)) => Pred::Subtype(subtype.clone()),
        _ => pred.clone(),
    };
    Keyword::Protection(PickSelector(quality.0.iter().map(fixed).collect()))
}

/// What the engine applies for a modification of the card: its amounts worked out and the player it names found in
/// `scope`. An effect of a spell or ability does this as it begins and a static ability whenever the state changes, so
/// both run the same modification.
pub(crate) fn lower(scope: &Scope<'_>, wanted: &Wanted) -> Result<Modification, ResolveError> {
    let amount = |amount: &Amount| -> Result<i32, ResolveError> {
        i32::try_from(scope.amount(amount)?).map_err(|_| {
            ResolveError::Invalid("a change of power or toughness is out of range".into())
        })
    };
    Ok(match wanted {
        Wanted::ModifyPT { power, toughness } => Modification::ModifyPowerToughness {
            power: amount(power)?,
            toughness: amount(toughness)?,
        },
        Wanted::SetPT { power, toughness } => Modification::SetPowerToughness {
            power: amount(power)?,
            toughness: amount(toughness)?,
        },
        Wanted::SwitchPT => Modification::SwitchPowerToughness,
        Wanted::AddType(ty) => Modification::AddType(*ty),
        Wanted::RemoveType(ty) => Modification::RemoveType(*ty),
        Wanted::SetTypes(types) => Modification::SetTypes(types.iter().copied().collect()),
        Wanted::RemoveSupertype(_) => {
            return Err(ResolveError::Unsupported(
                "Mod.RemoveSupertype outside the exceptions of a copy",
            ));
        }
        Wanted::AddSubtype(subtype) => Modification::AddSubtype(subtype.clone()),
        Wanted::SetCreatureTypes(types) => Modification::SetCreatureTypes(types.clone()),
        Wanted::AddAllCreatureTypes => Modification::AddAllCreatureTypes,
        Wanted::SetColors(colors) => Modification::SetColors(colors.iter().copied().collect()),
        Wanted::AddKeyword(keyword) => Modification::AddKeyword(chosen_in(scope, keyword)),
        Wanted::RemoveKeyword(keyword) => Modification::RemoveKeyword(keyword.clone()),
        Wanted::GrantAbility(granted) => Modification::GrantAbility(Arc::clone(granted)),
        Wanted::LoseAbilities => Modification::LoseAbilities,
        // A reference that names several players names the first in turn order from the active player.
        Wanted::SetController(who) => {
            Modification::SetController(*scope.players(who)?.first().ok_or_else(|| {
                ResolveError::Invalid("a change of control names no player".into())
            })?)
        }
        Wanted::AddProtectionFromChosenColor => {
            return Err(ResolveError::Unsupported(
                "Mod.AddProtectionFromChosenColor outside an effect",
            ));
        }
        Wanted::CantAttack => Modification::CantAttack,
        Wanted::CantAttackYou(attacked) => Modification::CantAttackYou {
            you: scope.controller,
            attacked: *attacked,
        },
        Wanted::CantBlock => Modification::CantBlock,
        Wanted::DoesntUntap => Modification::DoesntUntap,
        Wanted::CantBeBlocked => Modification::CantBeBlocked,
        Wanted::Goaded => Modification::Goaded {
            by: scope.controller,
        },
        Wanted::CantBeBlockedBy(blockers) => Modification::CantBeBlockedBy {
            by: blockers.clone(),
            controller: scope.controller,
            source: scope.source,
        },
        Wanted::CanBlockOnly(only) => Modification::CanBlockOnly {
            only: only.clone(),
            controller: scope.controller,
            source: scope.source,
        },
        Wanted::MinBlockers(count) => Modification::MinBlockers(*count),
        Wanted::MaxBlockers(count) => Modification::MaxBlockers(*count),
        Wanted::BlocksAdditional(count) => Modification::BlocksAdditional(*count),
    })
}

#[cfg(test)]
mod tests {
    //! Goal: a continuous effect begins as the card says: one effect for each modification, in the order of the text, with the next ids and timestamps of the state, naming the permanents the reference names now and no object that has since changed zones or is not a permanent, with its source, controller, turn and duration, its amounts worked out at that moment, and the modifications the engine does not run refuse by their own names.
    use mtg_cards::refs::{Bounds, Selector as Pick, TargetSpec, Targetable};
    use mtg_core::ids::PlayerId;
    use rstest::rstest;

    use super::*;
    use crate::choose::{Choices, Target};
    use crate::fixtures::{creature, state};
    use crate::resolve::{Resolving, Scope};
    use crate::state::GameState;

    const ME: PlayerId = PlayerId(0);

    fn game() -> GameState {
        state(
            vec![vec![creature("Pump", 0, 0)], vec![]],
            vec![
                vec![creature("Source", 1, 1), creature("Wolf", 2, 2)],
                vec![creature("Bear", 2, 2), creature("Ogre", 3, 4)],
            ],
        )
    }

    fn named(game: &GameState, name: &str) -> ObjectRef {
        game.objects()
            .find(|o| o.card().name == name)
            .map(GameObject::reference)
            .unwrap()
    }

    fn effect(json: &str) -> Effect {
        serde_json::from_str(json).unwrap()
    }

    fn creature_spec() -> TargetSpec {
        TargetSpec {
            what: Targetable::Object(Pick(vec![mtg_cards::refs::Pred::Type(
                mtg_core::card::CardType::Creature,
            )])),
            bounds: Bounds::default(),
            divide: None,
        }
    }

    /// Runs `effect` as the Source's with `chosen` as the targets of one creature specification.
    fn run_with(
        game: &GameState,
        chosen: Vec<Target>,
        effect: &Effect,
    ) -> Result<Run, ResolveError> {
        let source = named(game, "Source");
        let resolving = Resolving::begin(source.id);
        let choices = Choices {
            targets: vec![chosen],
            ..Choices::default()
        };
        let spec = creature_spec();
        let mut scope = Scope::announcing(game, ME, source);
        scope.choices = &choices;
        scope.specs = vec![&spec];
        let cx = Cx {
            scope,
            resolving: &resolving,
        };
        run(&cx, effect)
    }

    fn begun(run: Run) -> Vec<ContinuousEffect> {
        let Run::Done {
            events,
            bound,
            snapshots,
            ..
        } = run
        else {
            panic!("the effect is done");
        };
        assert!(bound.is_empty() && snapshots.is_empty());
        events
            .into_iter()
            .map(|event| match event {
                Event::EffectCreated { effect } => effect,
                other => panic!("only effects begin, got {other:?}"),
            })
            .collect()
    }

    const GROWTH: &str = r#"{"Continuous":{"affected":{"Target":0},"mods":[{"ModifyPT":{"power":3,"toughness":3}}],"duration":"UntilEndOfTurn"}}"#;

    #[test]
    fn a_pump_names_what_was_targeted_and_begins_now() {
        let mut game = game();
        game.turn.number = 4;
        let bear = named(&game, "Bear");
        let made = begun(run_with(&game, vec![Target::Object(bear)], &effect(GROWTH)).unwrap());
        let source = named(&game, "Source");
        assert_eq!(
            made,
            [ContinuousEffect::new(
                EffectId(0),
                Selector::Objects(vec![bear]),
                Modification::ModifyPowerToughness {
                    power: 3,
                    toughness: 3
                },
                mtg_cards::modify::Duration::UntilEndOfTurn,
                1,
            )
            .from_source(source)
            .begun(ME, 4)]
        );
    }

    #[test]
    fn the_modifications_take_the_next_ids_and_timestamps_in_the_order_of_the_text() {
        let mut game = game();
        game.next_effect = 7;
        game.next_timestamp = 40;
        let rush = effect(
            r#"{"Continuous":{"affected":{"Target":0},"mods":[{"AddKeyword":"Haste"},{"ModifyPT":{"power":1,"toughness":0}},{"AddKeyword":"Flying"}],"duration":"Forever"}}"#,
        );
        let wolf = named(&game, "Wolf");
        let made = begun(run_with(&game, vec![Target::Object(wolf)], &rush).unwrap());
        let seen: Vec<_> = made
            .iter()
            .map(|e| (e.id.0, e.timestamp, e.layer, e.selector.clone()))
            .collect();
        assert_eq!(
            seen,
            [
                (
                    7,
                    40,
                    Some(crate::effects::Layer::Ability),
                    Selector::Objects(vec![wolf])
                ),
                (
                    8,
                    41,
                    Some(crate::effects::Layer::PtModify),
                    Selector::Objects(vec![wolf])
                ),
                (
                    9,
                    42,
                    Some(crate::effects::Layer::Ability),
                    Selector::Objects(vec![wolf])
                ),
            ]
        );
        assert!(
            made.iter()
                .all(|e| e.duration == mtg_cards::modify::Duration::Forever)
        );
    }

    #[rstest]
    #[case::my_creatures(r#"[{"Type":"Creature"},{"ControlledBy":"You"}]"#, &["Source", "Wolf"])]
    #[case::theirs(r#"[{"Type":"Creature"},{"ControlledBy":"Opponents"}]"#, &["Bear", "Ogre"])]
    #[case::the_big_ones(r#"[{"Compare":{"stat":"Power","op":"Ge","to":3}}]"#, &["Ogre"])]
    #[case::nothing_matches(r#"[{"Type":"Land"}]"#, &[])]
    #[case::cards_in_hand_are_not_permanents(r#"[{"In":"Hand"}]"#, &[])]
    fn a_selector_names_the_permanents_it_matches_now_and_all_are_one_effect(
        #[case] filter: &str,
        #[case] expected: &[&str],
    ) {
        let game = game();
        let all = effect(&format!(
            r#"{{"Continuous":{{"affected":{{"Each":{filter}}},"mods":[{{"ModifyPT":{{"power":1,"toughness":1}}}}],"duration":"UntilEndOfTurn"}}}}"#
        ));
        let made = begun(run_with(&game, Vec::new(), &all).unwrap());
        let Some(first) = made.first() else {
            assert!(expected.is_empty(), "something was named");
            return;
        };
        assert_eq!(made.len(), 1, "one modification is one effect");
        let Selector::Objects(objects) = &first.selector else {
            panic!("the objects are named");
        };
        let mut names: Vec<_> = objects
            .iter()
            .map(|o| game.object(o.id).unwrap().card().name.clone())
            .collect();
        names.sort();
        let mut want: Vec<_> = expected.iter().map(ToString::to_string).collect();
        want.sort();
        assert_eq!(names, want);
    }

    #[test]
    fn a_target_that_became_a_new_object_is_not_named() {
        let mut game = game();
        let bear = named(&game, "Bear");
        game.objects.get_mut(&bear.id).unwrap().counter += 1;
        let made = begun(run_with(&game, vec![Target::Object(bear)], &effect(GROWTH)).unwrap());
        assert!(made.is_empty(), "the creature that was targeted is gone");
    }

    #[test]
    fn the_amounts_are_worked_out_when_the_effect_begins() {
        let game = game();
        let growth = effect(
            r#"{"Continuous":{"affected":{"Target":0},"mods":[{"ModifyPT":{"power":{"Count":[{"Type":"Creature"}]},"toughness":-2}}],"duration":"UntilEndOfTurn"}}"#,
        );
        let ogre = named(&game, "Ogre");
        let made = begun(run_with(&game, vec![Target::Object(ogre)], &growth).unwrap());
        assert_eq!(
            made[0].modification,
            Modification::ModifyPowerToughness {
                power: 4,
                toughness: -2
            }
        );
    }

    fn granted() -> Arc<mtg_cards::ability::Ability> {
        Arc::new(
            serde_json::from_str(r#"{"Activated":{"cost":["Tap"],"effects":[{"Draw":{}}]}}"#)
                .unwrap(),
        )
    }

    #[rstest]
    #[case::set(r#"{"SetPT":{"power":1,"toughness":2}}"#, Modification::SetPowerToughness { power: 1, toughness: 2 })]
    #[case::set_from_an_amount(r#"{"SetPT":{"power":{"Count":[{"Type":"Creature"}]},"toughness":0}}"#, Modification::SetPowerToughness { power: 4, toughness: 0 })]
    #[case::switch(r#""SwitchPT""#, Modification::SwitchPowerToughness)]
    #[case::a_type(
        r#"{"AddType":"Artifact"}"#,
        Modification::AddType(mtg_core::card::CardType::Artifact)
    )]
    #[case::a_type_removed(
        r#"{"RemoveType":"Creature"}"#,
        Modification::RemoveType(mtg_core::card::CardType::Creature)
    )]
    #[case::a_subtype(r#"{"AddSubtype":"Elf"}"#, Modification::AddSubtype(mtg_core::card::Subtype("Elf".into())))]
    #[case::colours(r#"{"SetColors":["Blue","Red"]}"#, Modification::SetColors(mtg_core::color::ColorSet::EMPTY.with(mtg_core::color::Color::Blue).with(mtg_core::color::Color::Red)))]
    #[case::no_colour(
        r#"{"SetColors":[]}"#,
        Modification::SetColors(mtg_core::color::ColorSet::EMPTY)
    )]
    #[case::a_keyword(
        r#"{"AddKeyword":"Haste"}"#,
        Modification::AddKeyword(mtg_cards::keyword::Keyword::Haste)
    )]
    #[case::a_keyword_removed(
        r#"{"RemoveKeyword":"Flying"}"#,
        Modification::RemoveKeyword(mtg_cards::keyword::Keyword::Flying)
    )]
    #[case::a_granted_ability(
        r#"{"GrantAbility":{"Activated":{"cost":["Tap"],"effects":[{"Draw":{}}]}}}"#,
        Modification::GrantAbility(granted())
    )]
    #[case::every_ability_lost(r#""LoseAbilities""#, Modification::LoseAbilities)]
    #[case::control_to_the_controller(
        r#"{"SetController":"You"}"#,
        Modification::SetController(ME)
    )]
    #[case::control_to_the_opponent(
        r#"{"SetController":"Opponents"}"#,
        Modification::SetController(PlayerId(1))
    )]
    #[case::control_to_the_owner(
        r#"{"SetController":{"OwnerOf":{"Target":0}}}"#,
        Modification::SetController(PlayerId(1))
    )]
    #[case::not_attacking(r#""CantAttack""#, Modification::CantAttack)]
    #[case::not_blocking(r#""CantBlock""#, Modification::CantBlock)]
    fn every_modification_of_the_card_becomes_the_one_the_engine_applies(
        #[case] modification: &str,
        #[case] expected: Modification,
    ) {
        let game = game();
        let bear = named(&game, "Bear");
        let asked = effect(&format!(
            r#"{{"Continuous":{{"affected":{{"Target":0}},"mods":[{modification}],"duration":"UntilEndOfTurn"}}}}"#
        ));
        let made = begun(run_with(&game, vec![Target::Object(bear)], &asked).unwrap());
        assert_eq!(made.len(), 1);
        assert_eq!(made[0].modification, expected);
        assert_eq!(
            made[0].layer,
            expected.layer(),
            "the layer follows the modification"
        );
    }

    #[test]
    fn a_restriction_on_being_blocked_keeps_who_and_what_it_was_made_by() {
        let game = game();
        let bear = named(&game, "Bear");
        let source = named(&game, "Source");
        let small = Pick(vec![
            mtg_cards::refs::Pred::Type(mtg_core::card::CardType::Creature),
            mtg_cards::refs::Pred::Compare {
                stat: mtg_cards::value::StatKind::Power,
                op: mtg_cards::value::Cmp::Le,
                to: mtg_cards::value::Amount::Fixed(2),
            },
        ]);
        let cases = [
            (r#""CantBeBlocked""#, Modification::CantBeBlocked),
            (
                r#"{"CantBeBlockedBy":[{"Type":"Creature"},{"Compare":{"stat":"Power","op":"Le","to":2}}]}"#,
                Modification::CantBeBlockedBy {
                    by: small,
                    controller: ME,
                    source,
                },
            ),
        ];
        for (modification, expected) in cases {
            let asked = effect(&format!(
                r#"{{"Continuous":{{"affected":{{"Target":0}},"mods":[{modification}],"duration":"UntilEndOfTurn"}}}}"#
            ));
            let made = begun(run_with(&game, vec![Target::Object(bear)], &asked).unwrap());
            assert_eq!(made.len(), 1);
            assert_eq!(made[0].modification, expected);
            assert_eq!(made[0].layer, None, "a restriction has no layer");
        }
    }

    #[test]
    fn a_change_of_control_that_names_no_player_is_refused() {
        let game = game();
        let asked = effect(
            r#"{"Continuous":{"affected":"This","mods":[{"SetController":{"Each":[{"Type":"Land"}]}}],"duration":"UntilEndOfTurn"}}"#,
        );
        assert!(matches!(
            run_with(&game, Vec::new(), &asked),
            Err(ResolveError::Invalid(_))
        ));
    }
}
