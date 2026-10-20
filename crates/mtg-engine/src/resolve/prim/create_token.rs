//! `Effect::CreateToken`: tokens enter the battlefield under a player's control (CR 111).

use std::sync::Arc;

use mtg_cards::assemble::token_definition;
use mtg_cards::card::TokenSpec;
use mtg_cards::effect::{Effect, TokenRef};
use mtg_core::ids::{ObjectId, PlayerId};

use super::wrong_effect;
use crate::caps::caps;
use crate::choose::{ChoiceOption, OptionChooser, Optioned, pick_options};
use crate::combat::{AttackTarget, defenders_for};
use crate::events::Event;
use crate::resolve::{Cx, ResolveError, Run};
use crate::state::GameState;
use crate::tokens::TokenCreated;

/// What a predefined token is: the description in its file (`cards/tokens/<id>.json`).
fn predefined(id: &str) -> Result<&'static TokenSpec, ResolveError> {
    mtg_cards::builtin::library()
        .ok()
        .and_then(|library| library.token(id))
        .map(|file| &file.token)
        .ok_or_else(|| ResolveError::Invalid(format!("there is no predefined token `{id}`")))
}

/// What a player who puts tokens onto the battlefield attacking may have them attack: the other players and the
/// planeswalkers they control, while that player's own attack is under way and its attackers are declared.
fn attackable(state: &GameState, player: PlayerId) -> Vec<(AttackTarget, ChoiceOption)> {
    if state.turn().active() != player || !state.combat().attackers_declared() {
        return Vec::new();
    }
    defenders_for(state, player)
        .into_iter()
        .map(|target| {
            let option = match target {
                AttackTarget::Player(seat) => ChoiceOption::Player(seat),
                AttackTarget::Planeswalker(walker) => ChoiceOption::Label(
                    state
                        .object(walker)
                        .map_or_else(String::new, |o| o.card().name.clone()),
                ),
            };
            (target, option)
        })
        .collect()
}

/// Each player `who` names who is still in the game creates `count` tokens, which enter tapped when the effect says
/// so. A token that enters attacking (CR 508.4) was never declared as an attacker, so nothing triggers on its
/// attack; its controller chooses the player or planeswalker it attacks when there is more than one (an attack by
/// someone else's turn, or with no attackers declared yet, is no attack: the token just enters).
///
/// The characteristics and abilities come from the description, with the abilities the engine runs; they are in the
/// event, so a log folds with no card data at hand.
pub(super) fn run(cx: &Cx<'_>, effect: &Effect) -> Result<Run, ResolveError> {
    let Effect::CreateToken {
        who,
        token,
        count,
        tapped,
        attacking,
    } = effect
    else {
        return Err(wrong_effect("CreateToken"));
    };
    let spec = match token {
        TokenRef::Named(id) => predefined(id)?,
        TokenRef::Custom(spec) => &**spec,
        TokenRef::CopyOf { .. } => return Err(ResolveError::Unsupported("Effect.CreateToken.Copy")),
    };
    let state = cx.state();
    let number = usize::try_from(cx.scope.amount(count)?.max(0)).unwrap_or(0);
    let players: Vec<PlayerId> = cx
        .scope
        .players(who)?
        .into_iter()
        .filter(|player| state.is_alive(*player))
        .collect();
    if number == 0 || players.is_empty() {
        return Ok(Run::events(Vec::new()));
    }
    // Where each player's tokens attack: chosen by their controller, once for all of them.
    let mut aims: Vec<(PlayerId, Option<AttackTarget>)> =
        players.iter().map(|player| (*player, None)).collect();
    if *attacking {
        let options: Vec<(PlayerId, Vec<(AttackTarget, ChoiceOption)>)> = players
            .iter()
            .map(|player| (*player, attackable(state, *player)))
            .filter(|(_, targets)| !targets.is_empty())
            .collect();
        let choosers: Vec<OptionChooser> = options
            .iter()
            .map(|(player, targets)| OptionChooser {
                player: *player,
                options: targets.iter().map(|(_, option)| option.clone()).collect(),
                text: "Choose a player or planeswalker for the tokens to attack.".into(),
            })
            .collect();
        match pick_options(cx, &choosers)? {
            Optioned::Ask(decision) => return Ok(Run::Ask(decision)),
            Optioned::Done(taken) => {
                for (player, index) in taken {
                    let target = options
                        .iter()
                        .find(|(seat, _)| *seat == player)
                        .and_then(|(_, targets)| targets.get(index))
                        .map(|(target, _)| *target);
                    if let Some(aim) = aims.iter_mut().find(|(seat, _)| *seat == player) {
                        aim.1 = target;
                    }
                }
            }
        }
    }
    let definition = Arc::new(token_definition(spec, &caps()));
    let first = state.next_object().get();
    let mut made = 0;
    let mut events = Vec::with_capacity(players.len() * number);
    for (controller, aim) in aims {
        for _ in 0..number {
            events.push(Event::TokenCreated(TokenCreated {
                object: ObjectId(first + made),
                controller,
                name: definition.name.clone(),
                tapped: *tapped,
                attacking: aim,
                definition: Arc::clone(&definition),
            }));
            made += 1;
        }
    }
    Ok(Run::events(events))
}

#[cfg(test)]
mod tests {
    //! Goal: a token maker creates the number asked for each player it names, under that player's control, with the characteristics and the abilities of the description it names (inline or predefined), tapped when it says so; a token enters attacking only on its controller's own attack after attackers are declared, and the controller is asked which defender only when there is more than one; an unknown token is refused and a copy waits for its task.
    use rstest::rstest;

    use super::*;
    use crate::choose::Choice;
    use crate::decision::DecisionKind;
    use crate::fixtures::{creature, state};
    use crate::resolve::{Resolving, Scope};
    use crate::zones::Zone;

    const ME: PlayerId = PlayerId(0);
    const FOE: PlayerId = PlayerId(1);
    const THIRD: PlayerId = PlayerId(2);

    const SOLDIER: &str = r#"{"Custom":{"name":"Soldier","colors":["White"],"types":["Creature"],"subtypes":["Soldier"],"power":1,"toughness":1}}"#;

    /// Seat 0 controls the Source; the other seats have nothing.
    fn game(seats: usize) -> GameState {
        let mut battlefields = vec![Vec::new(); seats];
        battlefields[0].push(creature("Source", 1, 1));
        state(vec![Vec::new(); seats], battlefields)
    }

    /// The same game in the middle of seat 0's attack.
    fn attacking_game(seats: usize) -> GameState {
        let mut game = game(seats);
        game.combat.attackers_declared = true;
        game
    }

    fn effect(token: &str, extra: &str) -> Effect {
        serde_json::from_str(&format!(r#"{{"CreateToken":{{"token":{token}{extra}}}}}"#)).unwrap()
    }

    fn run_with(
        game: &GameState,
        journal: Vec<(PlayerId, Choice)>,
        effect: &Effect,
    ) -> Result<Run, ResolveError> {
        let source = game
            .object(game.zone(Zone::Battlefield)[0])
            .expect("the source");
        let resolving = Resolving {
            journal,
            ..Resolving::begin(source.id())
        };
        let cx = Cx {
            scope: Scope::announcing(game, ME, source.reference()),
            resolving: &resolving,
        };
        run(&cx, effect)
    }

    fn tokens(run: Run) -> Vec<TokenCreated> {
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
                Event::TokenCreated(token) => token,
                other => panic!("only tokens were expected, got {other:?}"),
            })
            .collect()
    }

    #[rstest]
    #[case::one_by_default("", &[ME])]
    #[case::three(r#","count":3"#, &[ME, ME, ME])]
    #[case::nothing(r#","count":0"#, &[])]
    #[case::each_opponent(r#","who":"Opponents""#, &[FOE, THIRD])]
    #[case::counted_per_player(r#","who":"EachPlayer","count":2"#, &[ME, ME, FOE, FOE, THIRD, THIRD])]
    fn each_player_named_gets_the_number_of_tokens_asked_for(
        #[case] extra: &str,
        #[case] controllers: &[PlayerId],
    ) {
        let game = game(3);
        let made = tokens(run_with(&game, Vec::new(), &effect(SOLDIER, extra)).unwrap());
        assert_eq!(
            made.iter().map(|t| t.controller).collect::<Vec<_>>(),
            controllers
        );
        let first = game.next_object().get();
        for (at, token) in made.iter().enumerate() {
            assert_eq!(token.object.get(), first + at as u64, "ids follow on");
            assert_eq!((token.tapped, token.attacking), (false, None));
        }
    }

    #[test]
    fn a_token_has_the_characteristics_of_its_description() {
        let game = game(2);
        let made =
            tokens(run_with(&game, Vec::new(), &effect(SOLDIER, r#","tapped":true"#)).unwrap());
        let [token] = made.as_slice() else {
            panic!("one token");
        };
        assert!(token.tapped);
        assert_eq!(token.name, "Soldier");
        let def = &token.definition;
        assert_eq!((def.power, def.toughness), (Some(1), Some(1)));
        assert!(def.is_type(mtg_core::card::CardType::Creature));
        assert_eq!(def.subtypes[0].0, "Soldier");
        assert_eq!(def.colors().len(), 1);
    }

    #[rstest]
    #[case::treasure("treasure", "Treasure")]
    #[case::clue("clue", "Clue")]
    #[case::food("food", "Food")]
    fn a_predefined_token_is_the_one_in_its_file(#[case] id: &str, #[case] name: &str) {
        let game = game(2);
        let token = format!(r#"{{"Named":"{id}"}}"#);
        let made = tokens(run_with(&game, Vec::new(), &effect(&token, "")).unwrap());
        assert_eq!(made[0].definition.name, name);
        assert!(
            made[0]
                .definition
                .is_type(mtg_core::card::CardType::Artifact)
        );
        assert_eq!(made[0].definition.subtypes[0].0, name);
    }

    #[test]
    fn a_treasure_carries_its_mana_ability() {
        let game = game(2);
        let made =
            tokens(run_with(&game, Vec::new(), &effect(r#"{"Named":"treasure"}"#, "")).unwrap());
        assert_eq!(made[0].definition.rules.len(), 1);
        assert!(made[0].definition.rules[0].is_mana_ability());
    }

    #[test]
    fn an_unknown_token_is_refused() {
        let error =
            run_with(&game(2), Vec::new(), &effect(r#"{"Named":"gold"}"#, "")).unwrap_err();
        assert!(matches!(error, ResolveError::Invalid(_)), "{error:?}");
    }

    #[test]
    fn a_copy_waits_for_the_copy_effects() {
        let error = run_with(
            &game(2),
            Vec::new(),
            &effect(r#"{"CopyOf":{"of":"This"}}"#, ""),
        )
        .unwrap_err();
        assert!(matches!(
            error,
            ResolveError::Unsupported("Effect.CreateToken.Copy")
        ));
    }

    #[test]
    fn a_dead_player_gets_no_token() {
        let mut game = game(3);
        game.players[2].status =
            crate::player::PlayerStatus::Lost(crate::player::LossReason::Conceded);
        let made = tokens(
            run_with(&game, Vec::new(), &effect(SOLDIER, r#","who":"Opponents""#)).unwrap(),
        );
        assert_eq!(made.iter().map(|t| t.controller).collect::<Vec<_>>(), [FOE]);
    }

    #[test]
    fn the_only_defender_is_attacked_without_a_question() {
        let game = attacking_game(2);
        let made = tokens(
            run_with(
                &game,
                Vec::new(),
                &effect(SOLDIER, r#","count":2,"attacking":true"#),
            )
            .unwrap(),
        );
        assert_eq!(made.len(), 2);
        assert!(
            made.iter()
                .all(|t| t.attacking == Some(AttackTarget::Player(FOE)))
        );
    }

    #[test]
    fn with_several_defenders_the_controller_is_asked_once_for_all_the_tokens() {
        let game = attacking_game(3);
        let attacking = effect(SOLDIER, r#","count":2,"attacking":true"#);
        let Run::Ask(decision) = run_with(&game, Vec::new(), &attacking).unwrap() else {
            panic!("the controller chooses");
        };
        assert_eq!(decision.player, ME);
        let DecisionKind::ChooseOption(options) = decision.kind else {
            panic!("a choice of option");
        };
        assert_eq!(
            options.options,
            [ChoiceOption::Player(FOE), ChoiceOption::Player(THIRD)]
        );
        for (index, defender) in [(0, FOE), (1, THIRD)] {
            let made =
                tokens(run_with(&game, vec![(ME, Choice::Option(index))], &attacking).unwrap());
            assert_eq!(made.len(), 2);
            assert!(
                made.iter()
                    .all(|t| t.attacking == Some(AttackTarget::Player(defender)))
            );
        }
    }

    #[test]
    fn tokens_do_not_attack_before_attackers_are_declared_or_on_another_turn() {
        let attacking = effect(SOLDIER, r#","attacking":true"#);
        // No attack is under way.
        let made = tokens(run_with(&game(3), Vec::new(), &attacking).unwrap());
        assert_eq!(made[0].attacking, None);
        // The opponents make them during seat 0's attack: it is not their attack.
        let made = tokens(
            run_with(
                &attacking_game(3),
                Vec::new(),
                &effect(SOLDIER, r#","who":"Opponents","attacking":true"#),
            )
            .unwrap(),
        );
        assert!(made.iter().all(|t| t.attacking.is_none()));
    }
}
