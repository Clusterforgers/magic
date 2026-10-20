//! The one reader of an object's abilities: every path of the engine that needs an ability of an object, to offer it,
//! trigger it, cast it or resolve it, asks here and never indexes the rules of a card itself.
//!
//! An ability is named by an [`AbilityRef`] (the object, where the ability comes from and an index), so abilities that
//! are not printed on the card can be named and read through the same two functions (`docs/engine.md`, Spells and
//! abilities).

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::keyword::Keyword;
use mtg_core::ids::ObjectId;

use crate::abilities::{AbilityRef, AbilitySource, Grant};
use crate::effects::Modification;
use crate::object::{GameObject, ObjectKind};
use crate::state::GameState;
use crate::triggers::triggers_something;
use crate::zones::Zone;

/// The object whose card the abilities of `id` are read from: the object itself, or the token it was before it
/// ceased to exist, since an ability it put on the stack still needs the rules it was printed with (CR 113.7a). An
/// ability on the stack has no abilities of its own.
fn holder(state: &GameState, id: ObjectId) -> Option<&GameObject> {
    state
        .source_object(id)
        .filter(|object| object.kind() != ObjectKind::Ability)
}

/// The abilities printed on the card of `object`, each with its address, in the order of the card's rules.
pub(crate) fn printed(object: &GameObject) -> impl Iterator<Item = (AbilityRef, &Arc<Ability>)> {
    object
        .def
        .rules
        .iter()
        .enumerate()
        .map(|(index, rules)| (AbilityRef::printed(object.id(), index), rules))
}

/// The static abilities printed on the card of `object`, each with its index among the card's rules: what
/// `statics` gathers the effects of.
pub(crate) fn static_abilities(
    object: &GameObject,
) -> impl Iterator<Item = (usize, &Arc<Ability>)> {
    printed(object)
        .filter(|(_, rules)| matches!(***rules, Ability::Static { .. }))
        .map(|(address, rules)| (address.ability, rules))
}

/// Whether anything a card with this definition does can make something trigger (`GameState::watches`): it prints a
/// triggered ability, sets up a delayed one or grants one.
pub(crate) fn definition_triggers(definition: &CardDefinition) -> bool {
    definition
        .rules
        .iter()
        .any(|rules| triggers_something(rules))
}

/// The abilities `id` has, each with the address that names it: those printed on its card in the order of its rules,
/// less the ones other effects took away, then the ones they gave it in the order the layers applied them.
///
/// Printed abilities are shared with the card (`Arc`) and a granted one with the effect that grants it. An effect
/// that takes every ability away (`LoseAbilities`) empties the list as it stands where it applies, and a keyword taken
/// away is no longer listed. Only a permanent has effects: for any other object, wherever it is, these are the printed
/// abilities, and whether one works from a zone is for the caller to ask. An object that is not in the game, and an
/// ability on the stack, have none.
pub fn abilities_of(state: &GameState, id: ObjectId) -> Vec<(AbilityRef, Arc<Ability>)> {
    let Some(object) = holder(state, id) else {
        return Vec::new();
    };
    if object.zone() == Zone::Battlefield
        && let Some(held) = state.layers().get(object.id())
    {
        return held
            .abilities
            .iter()
            .map(|held| (held.address, Arc::clone(&held.body)))
            .collect();
    }
    printed(object)
        .map(|(address, rules)| (address, Arc::clone(rules)))
        .collect()
}

/// The ability `address` names, or `None` when it names nothing in the game: the object is not there (an ability that
/// resolved, an id that was never used) or has no such ability.
///
/// A name is read from the data: a printed ability from the card, a granted one from the effect or the static ability
/// that grants it, which stays readable after the effect has ended or the object with the static ability has left, so an
/// ability that is on its way (a cost sacrificed its source) is still the one that was named. Whether the object has
/// the ability now is for [`abilities_of`] to say.
pub fn ability(state: &GameState, address: AbilityRef) -> Option<Arc<Ability>> {
    let object = holder(state, address.object)?;
    match address.source {
        AbilitySource::Printed => object.def.rules.get(address.ability).cloned(),
        AbilitySource::Granted(_) if address.ability != 0 => None,
        AbilitySource::Granted(Grant::Effect(id)) => state
            .effects()
            .iter()
            .find(|effect| effect.id == id)
            .and_then(|effect| granted_by(&effect.modification)),
        AbilitySource::Granted(Grant::Static {
            source,
            rule,
            index,
        }) => {
            let giver = holder(state, source)?;
            match giver.def.rules.get(rule).map(|rules| &**rules) {
                Some(Ability::Static { mods, .. }) => {
                    mods.get(index).and_then(|wanted| match wanted {
                        mtg_cards::modify::Modification::GrantAbility(granted) => {
                            Some(Arc::clone(granted))
                        }
                        _ => None,
                    })
                }
                _ => None,
            }
        }
    }
}

/// The ability a modification gives, if it gives one.
fn granted_by(modification: &Modification) -> Option<Arc<Ability>> {
    match modification {
        Modification::GrantAbility(granted) => Some(Arc::clone(granted)),
        _ => None,
    }
}

/// The keyword abilities of `id` in the order of their indices, borrowed from the state: the keywords `keywords_of`
/// starts from, before the ones continuous effects grant and take away.
pub(crate) fn keyword_abilities(state: &GameState, id: ObjectId) -> Vec<&Keyword> {
    holder(state, id).map_or_else(Vec::new, |object| {
        printed(object)
            .filter_map(|(_, rules)| match &**rules {
                Ability::Keyword(keyword) => Some(keyword),
                _ => None,
            })
            .collect()
    })
}

#[cfg(test)]
mod tests {
    //! Goal: an object has the abilities printed on its card, in the order of its rules, each under the address that names it, wherever it is (battlefield, hand, stack, graveyard) and whether it is a card or a token; an address resolves to the very ability it was read with and to nothing when the object is not in the game, is an ability on the stack or has no such ability; and no module other than this one reads the rules of a card, or other than `characteristics` its plain mana abilities, directly.
    use std::fs;
    use std::path::{Path, PathBuf};

    use mtg_cards::CardDefinition;
    use mtg_cards::ability::Timing;
    use mtg_cards::cost::Cost;
    use mtg_cards::effect::Effect;
    use mtg_cards::refs::{Ref, Selector, ZoneKind};
    use mtg_cards::trigger::TriggerPattern;
    use mtg_cards::value::Amount;
    use mtg_core::card::CardType;
    use mtg_core::ids::PlayerId;
    use rstest::rstest;

    use super::*;
    use crate::abilities::PutOnStack;
    use crate::choose::Choices;
    use crate::events::{Event, ResolutionOutcome, ZoneCause};
    use crate::fixtures::{creature, state};
    use crate::fold::apply_event;
    use crate::snapshot::snapshot;
    use crate::tokens::TokenCreated;
    use crate::zones::Zone;

    const ME: PlayerId = PlayerId(0);

    fn gain() -> Vec<Effect> {
        vec![Effect::GainLife {
            who: Ref::You,
            amount: Amount::Fixed(1),
        }]
    }

    fn activated() -> Ability {
        Ability::Activated {
            cost: Cost(Vec::new()),
            timing: Timing::Instant,
            limit: None,
            from: ZoneKind::Battlefield,
            targets: Vec::new(),
            effects: gain(),
        }
    }

    fn triggered() -> Ability {
        Ability::Triggered {
            when: TriggerPattern::Attacks {
                what: Selector::this(),
            },
            intervening_if: None,
            limit: None,
            from: ZoneKind::Battlefield,
            targets: Vec::new(),
            effects: gain(),
        }
    }

    /// Flying, an activated ability and a triggered one, in that order: three abilities of three kinds.
    fn veteran() -> Arc<CardDefinition> {
        Arc::new(
            CardDefinition::new("Veteran")
                .with_type(CardType::Creature)
                .with_power_toughness(1, 1)
                .with_rules(Ability::Keyword(Keyword::Flying))
                .with_rules(activated())
                .with_rules(triggered()),
        )
    }

    /// An instant with the one ability a spell has.
    fn zap() -> Arc<CardDefinition> {
        Arc::new(
            CardDefinition::new("Zap")
                .with_type(CardType::Instant)
                .with_rules(Ability::Spell {
                    targets: Vec::new(),
                    effects: gain(),
                }),
        )
    }

    /// Flying and haste with an ability that is not a keyword between them.
    fn raider() -> Arc<CardDefinition> {
        Arc::new(
            CardDefinition::new("Raider")
                .with_type(CardType::Creature)
                .with_power_toughness(2, 1)
                .with_rules(Ability::Keyword(Keyword::Flying))
                .with_rules(activated())
                .with_rules(Ability::Keyword(Keyword::Haste)),
        )
    }

    /// What a test reads the abilities of.
    #[derive(Debug, Clone, Copy)]
    enum Place {
        Battlefield,
        Hand,
        Stack,
        Graveyard,
        Token,
        CeasedToken,
        Vanilla,
        Raider,
        AbilityOnStack,
        ResolvedAbility,
        Unknown,
    }

    /// Seat 0 holds a Zap and controls a Veteran, a Bear and a Raider.
    fn game() -> GameState {
        state(
            vec![vec![zap()], vec![]],
            vec![vec![veteran(), creature("Bear", 2, 2), raider()], vec![]],
        )
    }

    fn fold(game: &mut GameState, events: &[Event]) {
        for event in events {
            apply_event(game, event).unwrap_or_else(|error| panic!("{event:?}: {error}"));
        }
    }

    /// A token with one printed ability.
    fn make_token(game: &mut GameState) -> ObjectId {
        let object = game.next_object();
        let created = TokenCreated {
            object,
            controller: ME,
            name: "Gem".into(),
            tapped: false,
            attacking: None,
            definition: Arc::new(
                CardDefinition::new("Gem")
                    .with_type(CardType::Artifact)
                    .with_rules(activated()),
            ),
        };
        fold(game, &[Event::TokenCreated(created)]);
        object
    }

    fn put_on_stack(game: &mut GameState, source: ObjectId, ability: usize) -> ObjectId {
        let object = game.next_object();
        let put = PutOnStack {
            origin: crate::abilities::AbilitySource::default(),
            object,
            source: game.object(source).unwrap().reference(),
            snapshot: snapshot(game, source).unwrap(),
            ability,
            controller: ME,
            choices: Choices::default(),
            cause: None,
            delayed: None,
        };
        fold(game, &[Event::AbilityPutOnStack(Box::new(put))]);
        object
    }

    /// A game and the object of `place` in it.
    fn at(place: Place) -> (GameState, ObjectId) {
        let mut game = game();
        let battlefield = game.zone(Zone::Battlefield).to_vec();
        let (veteran, bear, raider) = (battlefield[0], battlefield[1], battlefield[2]);
        let zap = game.zone(Zone::Hand(ME))[0];
        let id = match place {
            Place::Battlefield => veteran,
            Place::Hand => zap,
            Place::Stack => {
                let cast = Event::SpellCast {
                    player: ME,
                    object: zap,
                    choices: Choices::default(),
                };
                fold(&mut game, &[cast]);
                zap
            }
            Place::Graveyard => {
                let died = Event::ZoneChanged {
                    object: veteran,
                    from: Zone::Battlefield,
                    to: Zone::Graveyard(ME),
                    cause: ZoneCause::Destroyed,
                };
                fold(&mut game, &[died]);
                veteran
            }
            Place::Token => make_token(&mut game),
            Place::CeasedToken => {
                let token = make_token(&mut game);
                let sacrificed = Event::ZoneChanged {
                    object: token,
                    from: Zone::Battlefield,
                    to: Zone::Graveyard(ME),
                    cause: ZoneCause::Sacrificed,
                };
                fold(&mut game, &[sacrificed]);
                assert!(game.has_ceased(token));
                token
            }
            Place::Vanilla => bear,
            Place::Raider => raider,
            Place::AbilityOnStack => put_on_stack(&mut game, veteran, 1),
            Place::ResolvedAbility => {
                let ability = put_on_stack(&mut game, veteran, 1);
                let ended = Event::ResolutionEnded {
                    object: ability,
                    outcome: ResolutionOutcome::Countered,
                };
                fold(&mut game, &[ended]);
                assert!(game.object(ability).is_none());
                ability
            }
            Place::Unknown => ObjectId(9_999),
        };
        (game, id)
    }

    #[rstest]
    #[case::on_the_battlefield(Place::Battlefield, 3)]
    #[case::in_hand(Place::Hand, 1)]
    #[case::as_a_spell_on_the_stack(Place::Stack, 1)]
    #[case::in_a_graveyard(Place::Graveyard, 3)]
    #[case::a_token(Place::Token, 1)]
    #[case::a_token_that_ceased_to_exist(Place::CeasedToken, 1)]
    #[case::a_card_with_no_rules(Place::Vanilla, 0)]
    #[case::an_ability_on_the_stack(Place::AbilityOnStack, 0)]
    #[case::an_object_that_is_not_in_the_game(Place::Unknown, 0)]
    fn an_object_has_the_abilities_printed_on_its_card_in_the_order_of_its_rules(
        #[case] place: Place,
        #[case] count: usize,
    ) {
        let (game, id) = at(place);
        let found = abilities_of(&game, id);
        assert_eq!(found.len(), count, "{place:?}");
        let card = game.source_object(id).map(GameObject::card);
        for (index, (address, rules)) in found.iter().enumerate() {
            assert_eq!(*address, AbilityRef::printed(id, index));
            let printed = card.and_then(|card| card.rules.get(index));
            assert!(
                printed.is_some_and(|printed| Arc::ptr_eq(rules, printed)),
                "the ability is the card's own, not a copy"
            );
        }
    }

    #[test]
    fn the_abilities_of_a_card_come_in_the_order_of_its_rules() {
        let (game, id) = at(Place::Battlefield);
        let kinds: Vec<&str> = abilities_of(&game, id)
            .iter()
            .map(|(_, rules)| match &**rules {
                Ability::Keyword(_) => "keyword",
                Ability::Activated { .. } => "activated",
                Ability::Triggered { .. } => "triggered",
                other => panic!("unexpected {other:?}"),
            })
            .collect();
        assert_eq!(kinds, ["keyword", "activated", "triggered"]);
    }

    #[rstest]
    #[case::the_first(Place::Battlefield, 0, true)]
    #[case::the_last(Place::Battlefield, 2, true)]
    #[case::one_past_the_end(Place::Battlefield, 3, false)]
    #[case::far_past_the_end(Place::Battlefield, usize::MAX, false)]
    #[case::in_hand(Place::Hand, 0, true)]
    #[case::as_a_spell_on_the_stack(Place::Stack, 0, true)]
    #[case::after_a_zone_change_the_printed_ability_is_the_same(Place::Graveyard, 1, true)]
    #[case::a_token(Place::Token, 0, true)]
    #[case::a_token_that_ceased_still_has_what_it_put_on_the_stack(Place::CeasedToken, 0, true)]
    #[case::a_card_with_no_rules(Place::Vanilla, 0, false)]
    #[case::an_ability_on_the_stack_has_none_of_its_own(Place::AbilityOnStack, 0, false)]
    #[case::an_ability_that_resolved_is_gone(Place::ResolvedAbility, 0, false)]
    #[case::an_id_never_used(Place::Unknown, 0, false)]
    fn an_address_names_the_ability_it_was_read_with_and_nothing_once_it_is_stale(
        #[case] place: Place,
        #[case] index: usize,
        #[case] resolves: bool,
    ) {
        let (game, id) = at(place);
        let address = AbilityRef::printed(id, index);
        let read = ability(&game, address);
        assert_eq!(read.is_some(), resolves, "{place:?} {index}");
        if let Some(read) = read {
            let listed = abilities_of(&game, id);
            let same = listed
                .iter()
                .find(|(listed, _)| *listed == address)
                .is_some_and(|(_, rules)| Arc::ptr_eq(&read, rules));
            assert!(
                same,
                "{place:?} {index}: the list holds what the address reads"
            );
        }
    }

    #[rstest]
    #[case::one_keyword(Place::Battlefield, &[Keyword::Flying])]
    #[case::two_among_other_abilities(Place::Raider, &[Keyword::Flying, Keyword::Haste])]
    #[case::none_among_other_rules(Place::Hand, &[])]
    #[case::a_card_with_no_rules(Place::Vanilla, &[])]
    #[case::an_ability_on_the_stack(Place::AbilityOnStack, &[])]
    #[case::an_object_that_is_not_in_the_game(Place::Unknown, &[])]
    fn the_keyword_abilities_are_the_keywords_among_the_abilities_in_order(
        #[case] place: Place,
        #[case] expected: &[Keyword],
    ) {
        let (game, id) = at(place);
        let found = keyword_abilities(&game, id);
        assert_eq!(found, expected.iter().collect::<Vec<_>>(), "{place:?}");
    }

    /// The receivers of the reads of the field `field` in `source`, with the comments taken out and a method chain
    /// split over several lines joined. A call (`.rules()`) and a longer name are not reads.
    fn field_readers(source: &str, field: &str) -> Vec<String> {
        let code = source
            .lines()
            .map(|line| line.split("//").next().unwrap_or_default())
            .collect::<Vec<_>>()
            .join(" ");
        let code = code
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
            .replace(" .", ".")
            .replace(". ", ".");
        let name_start = |text: &str| {
            text.char_indices()
                .rev()
                .take_while(|(_, c)| c.is_alphanumeric() || *c == '_')
                .last()
                .map_or(text.len(), |(at, _)| at)
        };
        let dotted = format!(".{field}");
        code.match_indices(&dotted)
            .filter(|(at, name)| {
                let next = code[at + name.len()..].chars().next();
                next.is_none_or(|c| c != '(' && c != '_' && !c.is_alphanumeric())
            })
            .map(|(at, _)| {
                let before = &code[..at];
                let (name, call) = match before.strip_suffix("()") {
                    Some(name) => (name, "()"),
                    None => (before, ""),
                };
                format!("{}{call}", &name[name_start(name)..])
            })
            .collect()
    }

    #[rstest]
    #[case::the_rules_of_an_object("let r = object.card().rules.get(0);", "rules", &["card()"])]
    #[case::a_definition("let r = object.def.rules.iter();", "rules", &["def"])]
    #[case::a_bound_card("for r in card.rules.iter() {}", "rules", &["card"])]
    #[case::a_chain_over_several_lines(
        "object\n    .card()\n    .rules\n    .iter()",
        "rules",
        &["card()"]
    )]
    #[case::an_index("made[0].definition.rules[0]", "rules", &["definition"])]
    #[case::several_reads("a.def.rules.len() + b.card().rules.len()", "rules", &["def", "card()"])]
    #[case::the_plain_mana_abilities_of_a_card("let a = card.abilities.clone();", "abilities", &["card"])]
    #[case::the_result_of_a_call("characteristics(s, id)?.abilities.len()", "abilities", &[""])]
    #[case::the_other_field_is_not_read("object.card().rules.len()", "abilities", &[])]
    #[case::a_call_is_not_a_read("ability.rules()", "rules", &[])]
    #[case::a_builder_is_not_a_read("CardDefinition::new(n).with_rules(r)", "rules", &[])]
    #[case::a_longer_name_is_not_a_read("x.rules_of(y)", "rules", &[])]
    #[case::a_comment_is_not_code(
        "// object.card().rules.len()\nlet x = 1; // y.def.rules",
        "rules",
        &[]
    )]
    fn the_search_for_reads_of_a_cards_printed_side_finds_them_however_the_code_is_laid_out(
        #[case] source: &str,
        #[case] field: &str,
        #[case] expected: &[&str],
    ) {
        assert_eq!(field_readers(source, field), expected);
    }

    fn rust_files(dir: &Path, found: &mut Vec<PathBuf>) {
        let entries = fs::read_dir(dir).unwrap_or_else(|error| panic!("{dir:?}: {error}"));
        for entry in entries {
            let path = entry
                .unwrap_or_else(|error| panic!("{dir:?}: {error}"))
                .path();
            if path.is_dir() {
                rust_files(&path, found);
            } else if path.extension().is_some_and(|extension| extension == "rs") {
                found.push(path);
            }
        }
    }

    /// The modules that read the `rules` of a card directly: the reader, and no other.
    const RULES_OWNERS: [&str; 1] = ["ability_reader.rs"];

    /// The modules that read the plain mana abilities (`abilities`) of a card directly: the reader, and the derivation of
    /// characteristics, through which every other module reads them.
    const ABILITIES_OWNERS: [&str; 2] = ["ability_reader.rs", "characteristics.rs"];

    /// The receivers of a `rules` field that is not a card's: a pending trigger, a delayed trigger and an ability on
    /// the stack each hold the one ability they are.
    const OTHER_RULES: [&str; 3] = ["self", "waiting", "delayed"];

    /// What a card is called where its `abilities` are read; `chars.abilities` and `options.abilities` are not a card's.
    const CARDS: [&str; 4] = ["card()", "card", "def", "definition"];

    #[test]
    fn no_module_but_the_owners_reads_the_rules_or_the_plain_mana_abilities_of_a_card() {
        let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut files = Vec::new();
        rust_files(&src, &mut files);
        assert!(files.len() > 40, "the sources were found: {}", files.len());
        let mut offenders = Vec::new();
        for file in files {
            let relative = file.strip_prefix(&src).unwrap_or(&file);
            let owns = |owners: &[&str]| owners.iter().any(|owner| relative == Path::new(owner));
            let text =
                fs::read_to_string(&file).unwrap_or_else(|error| panic!("{file:?}: {error}"));
            if !owns(&RULES_OWNERS) {
                for receiver in field_readers(&text, "rules") {
                    if !OTHER_RULES.contains(&receiver.as_str()) {
                        offenders.push(format!(
                            "{} reads `{receiver}.rules`: read abilities through `ability_reader`",
                            relative.display()
                        ));
                    }
                }
            }
            if !owns(&ABILITIES_OWNERS) {
                for receiver in field_readers(&text, "abilities") {
                    if CARDS.contains(&receiver.as_str()) {
                        offenders.push(format!(
                            "{} reads `{receiver}.abilities`: read plain mana abilities through `characteristics`",
                            relative.display()
                        ));
                    }
                }
            }
        }
        assert!(offenders.is_empty(), "{offenders:#?}");
    }
}
