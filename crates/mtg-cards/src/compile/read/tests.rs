//! Goal: every template family reads the oracle wordings it models into the DSL, writes each back as the same text, and leaves every near-miss to the table.
use rstest::rstest;

use super::tokens::{Token, canonical, tokenize, without_period};
use super::*;

fn spell(text: &str) -> Option<Ability> {
    clause(text, ClauseKind::Spell, false)
}

fn spell_x(text: &str) -> Option<Ability> {
    clause(text, ClauseKind::Spell, true)
}

fn ability(json: &str) -> Ability {
    serde_json::from_str(json).unwrap_or_else(|error| panic!("{json}: {error}"))
}

/// Reads the wording and checks that the renderer writes the same text back, in one of its wordings.
fn golden(text: &str, kind: ClauseKind, x_free: bool) -> Ability {
    let read = clause(text, kind, x_free).unwrap_or_else(|| panic!("not read: {text}"));
    let wordings = render_variants(&read);
    assert!(!wordings.is_empty(), "not written: {text}");
    assert_eq!(render_ability(&read).as_ref(), wordings.first(), "{text}");
    assert!(
        wordings.iter().any(|written| same_text(written, text)),
        "{text}: {wordings:?}"
    );
    read
}

fn reads_as(text: &str, kind: ClauseKind, x_free: bool, json: &str) {
    assert_eq!(golden(text, kind, x_free), ability(json), "{text}");
}

#[rstest]
#[case::words(
    "Draw a card.",
    &[Token::Word("draw".into()), Token::Word("a".into()), Token::Word("card".into()), Token::Punct('.')]
)]
#[case::symbols_and_numbers(
    "{T}: Add {G}{G}. Scry 12",
    &[Token::Symbol("T".into()), Token::Punct(':'), Token::Word("add".into()), Token::Symbol("G".into()),
      Token::Symbol("G".into()), Token::Punct('.'), Token::Word("scry".into()), Token::Number(12)]
)]
#[case::the_card_itself(
    "~ deals 3 damage",
    &[Token::Source, Token::Word("deals".into()), Token::Number(3), Token::Word("damage".into())]
)]
#[case::this_creature(
    "This creature can't attack",
    &[Token::Source, Token::Word("can't".into()), Token::Word("attack".into())]
)]
#[case::sign_and_slash(
    "+2/-3",
    &[Token::Punct('+'), Token::Number(2), Token::Punct('/'), Token::Punct('-'), Token::Number(3)]
)]
#[case::hyphen_inside_a_word("non-token", &[Token::Word("non-token".into())])]
#[case::curly_apostrophe("don\u{2019}t", &[Token::Word("don't".into())])]
#[case::a_name_inside_a_sentence(
    "create a Treasure token",
    &[Token::Word("create".into()), Token::Word("a".into()), Token::Name("Treasure".into()), Token::Word("token".into())]
)]
#[case::a_capital_opening_a_sentence(
    "Draw a card. Then scry 1.\nGain 2",
    &[Token::Word("draw".into()), Token::Word("a".into()), Token::Word("card".into()), Token::Punct('.'),
      Token::Word("then".into()), Token::Word("scry".into()), Token::Number(1), Token::Punct('.'),
      Token::Word("gain".into()), Token::Number(2)]
)]
#[case::a_capital_after_a_colon_or_a_dash(
    "{T}: Add {G}. \u{2014} Draw",
    &[Token::Symbol("T".into()), Token::Punct(':'), Token::Word("add".into()), Token::Symbol("G".into()),
      Token::Punct('.'), Token::Punct('\u{2014}'), Token::Word("draw".into())]
)]
#[case::the_variable_is_not_a_name("create X tokens", &[Token::Word("create".into()), Token::Word("x".into()), Token::Word("tokens".into())])]
#[case::this_aura("Destroy this Aura", &[Token::Word("destroy".into()), Token::Source])]
fn the_tokens_of_a_clause(#[case] text: &str, #[case] expected: &[Token]) {
    assert_eq!(tokenize(text), expected);
}

#[rstest]
#[case::spaces("Draw  a   card. ", "Draw a card.")]
#[case::this_creature("This creature deals 1 damage to you.", "~ deals 1 damage to you.")]
#[case::this_spell("Counter this spell, and draw.", "Counter ~, and draw.")]
#[case::then("Scry 1, then draw a card.", "Scry 1. Draw a card.")]
#[case::then_twice("Draw a card, then scry 1, then draw.", "Draw a card. Scry 1. Draw.")]
#[case::lines("Scry 1.\nDraw a card.", "Scry 1. Draw a card.")]
#[case::the_older_enters(
    "When ~ enters the battlefield, draw a card.",
    "When ~ enters, draw a card."
)]
#[case::the_older_enters_tapped("~ enters the battlefield tapped.", "~ enters tapped.")]
#[case::leaves_is_not_enters(
    "When ~ leaves the battlefield, draw a card.",
    "When ~ leaves the battlefield, draw a card."
)]
#[case::to_the_battlefield(
    "Put it onto the battlefield. Return it to the battlefield.",
    "Put it onto the battlefield. Return it to the battlefield."
)]
#[case::of_their_choice(
    "Each player sacrifices a creature of their choice.",
    "Each player sacrifices a creature."
)]
#[case::of_their_choice_in_a_list(
    "Target opponent sacrifices a creature of their choice, discards a card.",
    "Target opponent sacrifices a creature, discards a card."
)]
#[case::of_their_own("Pick a card of their own choice.", "Pick a card of their own choice.")]
#[case::this_alone("Do this.", "Do this.")]
#[case::this_other_noun("Do this thing.", "Do this thing.")]
fn canonical_text_collapses_spaces_and_names_the_card(#[case] text: &str, #[case] expected: &str) {
    assert_eq!(canonical(text), expected);
}

#[rstest]
#[case::another_word_order("You draw a card.")]
#[case::digits_for_a_count("Draw 2 cards.")]
#[case::lowercase_start("draw a card.")]
#[case::a_plural_count("Draw a cards.")]
#[case::trailing_words("Draw a card now.")]
fn a_reading_that_does_not_write_back_as_the_text_stays_unread(#[case] text: &str) {
    assert_eq!(spell(text), None, "{text}");
}

#[rstest]
#[case::spell_kind_only(ClauseKind::Static)]
#[case::keyword(ClauseKind::Keyword)]
#[case::modal(ClauseKind::Modal)]
#[case::loyalty(ClauseKind::Loyalty)]
fn only_spell_triggered_and_activated_clauses_are_read(#[case] kind: ClauseKind) {
    assert_eq!(clause("Draw a card.", kind, false), None);
    assert_eq!(
        clause("When ~ enters, draw a card.", kind, false),
        None,
        "{kind:?}"
    );
}

#[test]
fn a_card_calling_itself_this_creature_reads_as_the_source() {
    let read = golden(
        "This creature deals 1 damage to any target.",
        ClauseKind::Spell,
        false,
    );
    let tilde = golden("~ deals 1 damage to any target.", ClauseKind::Spell, false);
    assert_eq!(read, tilde);
}

mod actions;
mod activated;
mod additional;
mod among;
mod as_long_as;
mod attached_kinds;
mod attack_tax;
mod attacked_player;
mod audit;
mod aura_grants;
mod base_stats;
mod becomes;
mod block_limits;
mod cast;
mod cast_cost_paid;
mod cast_self;
mod cast_spell_kinds;
mod chapters;
mod chosen_color;
mod class;
mod combination_mana;
mod commander_statics;
mod compound;
mod conditions;
mod copies;
mod copy_supertypes;
mod copy_wordings;
mod cost_conditions;
mod cost_counts;
mod cost_wordings;
mod counted;
mod counter_actions;
mod counters;
mod create;
mod crew_events;
mod crew_record;
mod damage_wordings;
mod defining;
mod draws;
mod each_pump;
mod enchanted_player;
mod entered_this_turn;
mod enters;
mod enters_tapped;
mod evasion;
mod event_batches;
mod events;
mod exile_bounce_shrink;
mod exile_while;
mod flicker;
mod for_each;
mod goad;
mod granted_mana;
mod granted_triggers;
mod graveyard;
mod group_statics;
mod hideaway;
mod if_sentence;
mod impulse;
mod lands;
mod levels;
mod library;
mod life;
mod life_gain;
mod lords;
mod mana_spent;
mod mana_triggers;
mod mana_wordings;
mod manifest;
mod mass_moves;
mod may;
mod modes;
mod monarch;
mod mutations;
mod named_tokens;
mod number_of;
mod paid_reads;
mod pay;
mod payments;
mod player_keywords;
mod prevention;
mod pump_wordings;
mod put_back;
mod removal_and_return;
mod returns;
mod reveal;
mod reveal_until;
mod search_by_controller;
mod search_lists;
mod search_pairs;
mod spell_copies;
mod spell_keywords;
mod spell_x_costs;
mod spend_subtypes;
mod static_pairs;
mod statics;
mod stations;
mod strive;
mod tap_and_mill;
mod this_or_another;
mod token_keywords;
mod token_wordings;
mod top_cast;
mod transform;
mod trigger_limits;
mod triggered_mana;
mod triggers;
mod turned_face_up;
mod vocabulary;
mod where_x;
mod written_effects;
mod x_after;

mod mana {
    //! Goal: a plain mana ability reads in the shapes of the cards, and a cost, a choice or a count it does not model stays with the table.
    use super::*;

    #[rstest]
    #[case::green(
        "{T}: Add {G}.",
        r#"{"Activated":{"cost":["Tap"],"effects":[{"AddMana":{"mana":{"Fixed":["Green"]}}}]}}"#
    )]
    #[case::colorless_pair("{T}: Add {C}{C}.", r#"{"Activated":{"cost":["Tap"],"effects":[{"AddMana":{"mana":{"Fixed":["Colorless","Colorless"]}}}]}}"#)]
    #[case::two_colors("{T}: Add {W}{U}.", r#"{"Activated":{"cost":["Tap"],"effects":[{"AddMana":{"mana":{"Fixed":["White","Blue"]}}}]}}"#)]
    #[case::any_color(
        "{T}: Add one mana of any color.",
        r#"{"Activated":{"cost":["Tap"],"effects":[{"AddMana":{"mana":{"AnyColor":1}}}]}}"#
    )]
    fn a_plain_mana_ability_reads_in_the_shape_of_the_hand_written_files(
        #[case] text: &str,
        #[case] json: &str,
    ) {
        reads_as(text, ClauseKind::Activated, false, json);
    }

    #[rstest]
    #[case::the_commanders_identity(
        "{T}: Add one mana of any color in your commander's color identity.",
        r#"{"Activated":{"cost":["Tap"],"effects":[{"AddMana":{"mana":{"CommanderIdentity":1}}}]}}"#
    )]
    #[case::a_land_an_opponent_controls(
        "{T}: Add one mana of any color that a land an opponent controls could produce.",
        r#"{"Activated":{"cost":["Tap"],"effects":[{"AddMana":{"mana":{"ProducedBy":{"among":[{"Type":"Land"},{"ControlledBy":"Opponents"}],"count":1}}}}]}}"#
    )]
    #[case::a_land_you_control(
        "{T}: Add one mana of any color that a land you control could produce.",
        r#"{"Activated":{"cost":["Tap"],"effects":[{"AddMana":{"mana":{"ProducedBy":{"among":[{"Type":"Land"},{"ControlledBy":"You"}],"count":1}}}}]}}"#
    )]
    #[case::any_type(
        "{T}: Add one mana of any type that a land you control could produce.",
        r#"{"Activated":{"cost":["Tap"],"effects":[{"AddMana":{"mana":{"ProducedBy":{"among":[{"Type":"Land"},{"ControlledBy":"You"}],"count":1,"colorless":true}}}}]}}"#
    )]
    #[case::two_of_one_color(
        "{T}: Add two mana of any one color.",
        r#"{"Activated":{"cost":["Tap"],"effects":[{"AddMana":{"mana":{"AnyColor":2}}}]}}"#
    )]
    #[case::three_of_one_color_for_creature_spells(
        "{T}: Add three mana of any one color. Spend this mana only to cast creature spells.",
        r#"{"Activated":{"cost":["Tap"],"effects":[{"AddMana":{"mana":{"AnyColor":3},"only":[{"Spell":{"types":["Creature"]}}]}}]}}"#
    )]
    #[case::a_cost_of_life(
        "{T}, Pay 1 life: Add {G} or {W}.",
        r#"{"Activated":{"cost":["Tap",{"PayLife":1}],"effects":[{"AddMana":{"mana":{"OneOf":["Green","White"]}}}]}}"#
    )]
    #[case::a_cost_of_energy(
        "{T}, Pay {E}: Add one mana of any color.",
        r#"{"Activated":{"cost":["Tap",{"PayEnergy":1}],"effects":[{"AddMana":{"mana":{"AnyColor":1}}}]}}"#
    )]
    #[case::a_counter_removed(
        "{T}, Remove a charge counter from ~: Add one mana of any color.",
        r#"{"Activated":{"cost":["Tap",{"RemoveCounters":{"counter":"charge"}}],"effects":[{"AddMana":{"mana":{"AnyColor":1}}}]}}"#
    )]
    #[case::only_if_you_control_a_swamp(
        "{T}: Add {W} or {B}. Activate only if you control a Swamp.",
        r#"{"Activated":{"cost":["Tap"],"condition":{"Compare":{"left":{"Count":[{"Subtype":"Swamp"},{"ControlledBy":"You"}]},"op":"Ge","right":1}},"effects":[{"AddMana":{"mana":{"OneOf":["White","Black"]}}}]}}"#
    )]
    #[case::a_signet(
        "{1}, {T}: Add {W}{U}.",
        r#"{"Activated":{"cost":[{"Mana":"{1}"},"Tap"],"effects":[{"AddMana":{"mana":{"Fixed":["White","Blue"]}}}]}}"#
    )]
    #[case::a_prism(
        "{1}, {T}: Add one mana of any color.",
        r#"{"Activated":{"cost":[{"Mana":"{1}"},"Tap"],"effects":[{"AddMana":{"mana":{"AnyColor":1}}}]}}"#
    )]
    #[case::a_quarry(
        "{5}, {T}: Add {W}{U}{B}{R}{G}.",
        r#"{"Activated":{"cost":[{"Mana":"{5}"},"Tap"],"effects":[{"AddMana":{"mana":{"Fixed":["White","Blue","Black","Red","Green"]}}}]}}"#
    )]
    #[case::a_hybrid_cost(
        "{W/U}, {T}: Add {W}{U}.",
        r#"{"Activated":{"cost":[{"Mana":"{W/U}"},"Tap"],"effects":[{"AddMana":{"mana":{"Fixed":["White","Blue"]}}}]}}"#
    )]
    fn a_mana_ability_that_depends_on_the_game_reads_with_its_cost_and_condition(
        #[case] text: &str,
        #[case] json: &str,
    ) {
        reads_as(text, ClauseKind::Activated, false, json);
        golden(text, ClauseKind::Activated, false);
    }

    #[rstest]
    #[case("{T}: Add {W}.")]
    #[case("{T}: Add {U}.")]
    #[case("{T}: Add {B}.")]
    #[case("{T}: Add {R}.")]
    #[case("{T}: Add {C}.")]
    #[case("{T}: Add {G}{G}.")]
    #[case("{T}: Add {R}{G}{W}.")]
    fn every_colour_and_colourless_mana_reads(#[case] text: &str) {
        golden(text, ClauseKind::Activated, false);
    }

    #[rstest]
    #[case::a_creature_spell(
        "{T}: Add one mana of any color. Spend this mana only to cast a creature spell.",
        r#"{"Activated":{"cost":["Tap"],"effects":[{"AddMana":{"mana":{"AnyColor":1},"only":[{"Spell":{"types":["Creature"]}}]}}]}}"#
    )]
    #[case::artifact_spells(
        "{T}: Add {C}{C}{C}. Spend this mana only to cast artifact spells.",
        r#"{"Activated":{"cost":["Tap"],"effects":[{"AddMana":{"mana":{"Fixed":["Colorless","Colorless","Colorless"]},"only":[{"Spell":{"types":["Artifact"]}}]}}]}}"#
    )]
    #[case::instant_or_sorcery_spells(
        "{T}: Add {R}. Spend this mana only to cast instant or sorcery spells.",
        r#"{"Activated":{"cost":["Tap"],"effects":[{"AddMana":{"mana":{"Fixed":["Red"]},"only":[{"Spell":{"types":["Instant","Sorcery"]}}]}}]}}"#
    )]
    #[case::a_multicolored_spell(
        "{T}: Add one mana of any color. Spend this mana only to cast a multicolored spell.",
        r#"{"Activated":{"cost":["Tap"],"effects":[{"AddMana":{"mana":{"AnyColor":1},"only":[{"Spell":{"multicolored":true}}]}}]}}"#
    )]
    #[case::noncreature_spells(
        "{T}: Add {G}. Spend this mana only to cast noncreature spells.",
        r#"{"Activated":{"cost":["Tap"],"effects":[{"AddMana":{"mana":{"Fixed":["Green"]},"only":[{"Spell":{"without":["Creature"]}}]}}]}}"#
    )]
    #[case::any_spell(
        "{T}: Add {G}. Spend this mana only to cast spells.",
        r#"{"Activated":{"cost":["Tap"],"effects":[{"AddMana":{"mana":{"Fixed":["Green"]},"only":[{"Spell":{}}]}}]}}"#
    )]
    #[case::for_each(
        "{T}: Add {G} for each creature you control.",
        r#"{"Activated":{"cost":["Tap"],"effects":[{"AddMana":{"mana":{"Fixed":["Green"]},"times":{"Count":[{"Type":"Creature"},{"ControlledBy":"You"}]}}}]}}"#
    )]
    #[case::for_each_and_a_restriction(
        "{T}: Add {R} for each Goblin you control. Spend this mana only to cast creature spells.",
        r#"{"Activated":{"cost":["Tap"],"effects":[{"AddMana":{"mana":{"Fixed":["Red"]},"times":{"Count":[{"Subtype":"Goblin"},{"ControlledBy":"You"}]},"only":[{"Spell":{"types":["Creature"]}}]}}]}}"#
    )]
    #[case::abilities(
        "{T}: Add {C}{C}. Spend this mana only to activate abilities.",
        r#"{"Activated":{"cost":["Tap"],"effects":[{"AddMana":{"mana":{"Fixed":["Colorless","Colorless"]},"only":[{"Ability":{}}]}}]}}"#
    )]
    #[case::costs_with_x(
        "{T}: Add {C}{C}{C}{C}. Spend this mana only on costs that contain {X}.",
        r#"{"Activated":{"cost":["Tap"],"effects":[{"AddMana":{"mana":{"Fixed":["Colorless","Colorless","Colorless","Colorless"]},"only":["CostWithX"]}}]}}"#
    )]
    fn mana_that_may_be_spent_only_on_some_things_reads_with_its_restriction(
        #[case] text: &str,
        #[case] json: &str,
    ) {
        reads_as(text, ClauseKind::Activated, false, json);
    }

    #[rstest]
    #[case::two_uses(
        "{T}: Add {G}. Spend this mana only to cast artifact spells or activate abilities of artifacts."
    )]
    #[case::a_chosen_kind(
        "{T}: Add one mana of any color. Spend this mana only to cast a creature spell of the chosen kind."
    )]
    #[case::without_the_period_after_the_mana(
        "{T}: Add {G} Spend this mana only to cast creature spells."
    )]
    #[case::the_restriction_alone("Spend this mana only to cast creature spells.")]
    fn a_restriction_the_templates_do_not_write_stays_unread(#[case] text: &str) {
        assert_eq!(clause(text, ClauseKind::Activated, false), None, "{text}");
    }

    #[rstest]
    #[case::a_choice_and_a_cost("{T}: Add {R} or {G}. {T}: Add {B}.")]
    #[case::x("{T}: Add {X}.")]
    #[case::generic("{T}: Add {2}.")]
    #[case::snow("{T}: Add {S}.")]
    #[case::no_mana("{T}: Add")]
    #[case::not_a_cost("Draw a card: Add {G}.")]
    fn a_cost_a_choice_or_a_count_the_template_does_not_model_stays_unread(#[case] text: &str) {
        assert_eq!(clause(text, ClauseKind::Activated, false), None, "{text}");
    }
}

mod cards_and_life {
    //! Goal: drawing, discarding, gaining and losing life read for you, a target player and each opponent, and "for each", "unless", "if" and a second effect on another subject stay unread.
    use super::*;

    #[rstest]
    #[case::one("Draw a card.", false, r#"{"Spell":{"effects":[{"Draw":{}}]}}"#)]
    #[case::three(
        "Draw three cards.",
        false,
        r#"{"Spell":{"effects":[{"Draw":{"count":3}}]}}"#
    )]
    #[case::x(
        "Draw X cards.",
        true,
        r#"{"Spell":{"effects":[{"Draw":{"count":"X"}}]}}"#
    )]
    #[case::target_player("Target player draws two cards.", false, r#"{"Spell":{"targets":[{"what":"Player"}],"effects":[{"Draw":{"who":{"Target":0},"count":2}}]}}"#)]
    #[case::each_opponent(
        "Each opponent draws a card.",
        false,
        r#"{"Spell":{"effects":[{"Draw":{"who":"Opponents"}}]}}"#
    )]
    #[case::each_player(
        "Each player draws a card.",
        false,
        r#"{"Spell":{"effects":[{"Draw":{"who":"EachPlayer"}}]}}"#
    )]
    #[case::gain(
        "You gain 3 life.",
        false,
        r#"{"Spell":{"effects":[{"GainLife":{"amount":3}}]}}"#
    )]
    #[case::gain_x(
        "You gain X life.",
        true,
        r#"{"Spell":{"effects":[{"GainLife":{"amount":"X"}}]}}"#
    )]
    #[case::target_gains("Target player gains 5 life.", false, r#"{"Spell":{"targets":[{"what":"Player"}],"effects":[{"GainLife":{"who":{"Target":0},"amount":5}}]}}"#)]
    #[case::lose(
        "You lose 2 life.",
        false,
        r#"{"Spell":{"effects":[{"LoseLife":{"amount":2}}]}}"#
    )]
    #[case::target_opponent_loses("Target opponent loses 3 life.", false, r#"{"Spell":{"targets":[{"what":"Opponent"}],"effects":[{"LoseLife":{"who":{"Target":0},"amount":3}}]}}"#)]
    #[case::each_opponent_loses(
        "Each opponent loses 2 life.",
        false,
        r#"{"Spell":{"effects":[{"LoseLife":{"who":"Opponents","amount":2}}]}}"#
    )]
    #[case::discard(
        "Discard two cards.",
        false,
        r#"{"Spell":{"effects":[{"Discard":{"count":2}}]}}"#
    )]
    #[case::random_discard("Target player discards a card at random.", false, r#"{"Spell":{"targets":[{"what":"Player"}],"effects":[{"Discard":{"who":{"Target":0},"random":true}}]}}"#)]
    #[case::mill("Target player mills three cards.", false, r#"{"Spell":{"targets":[{"what":"Player"}],"effects":[{"Mill":{"who":{"Target":0},"count":3}}]}}"#)]
    #[case::loot(
        "Draw two cards, then discard two cards.",
        false,
        r#"{"Spell":{"effects":[{"Draw":{"count":2}},{"Discard":{"count":2}}]}}"#
    )]
    fn the_wordings_read_into_the_dsl(
        #[case] text: &str,
        #[case] x_free: bool,
        #[case] json: &str,
    ) {
        reads_as(text, ClauseKind::Spell, x_free, json);
    }

    #[rstest]
    #[case("Draw a card.")]
    #[case("Draw two cards.")]
    #[case("Draw four cards.")]
    #[case("Draw seven cards.")]
    #[case("Target player draws a card.")]
    #[case("Target player draws three cards.")]
    #[case("You gain 1 life.")]
    #[case("You gain 4 life.")]
    #[case("You gain 7 life.")]
    #[case("You lose 1 life.")]
    #[case("Each opponent loses 1 life.")]
    #[case("Each player loses 3 life.")]
    #[case("Each player gains 2 life.")]
    #[case("Target opponent loses 5 life.")]
    #[case("Target opponent discards a card.")]
    #[case("Each opponent discards a card.")]
    #[case("Draw a card, then discard a card.")]
    fn more_oracle_wordings_read_and_write_back(#[case] text: &str) {
        golden(text, ClauseKind::Spell, false);
    }

    #[rstest]
    #[case::equal_to_an_unread_count(
        "Draw cards equal to the number of cards in target opponent's hand."
    )]
    #[case::unless("Target player draws a card unless they discard a card.")]
    #[case::if_you_do("Draw a card. If you do, gain 1 life.")]
    #[case::if_first("If you control a creature, draw a card.")]
    #[case::may_for_a_player("Target player may draw a card.")]
    #[case::then_a_player("Each player draws a card, then discards a card.")]
    #[case::x_without_an_x_cost("Draw X cards.")]
    #[case::lose_half("You lose half your life, rounded up.")]
    #[case::a_second_target("Target player draws a card. Target player loses 1 life.")]
    #[case::zero("You gain 0 life.")]
    #[case::life_equal_to_a_creature("You gain life equal to its power.")]
    #[case::each_creature_draws("Each creature draws a card.")]
    #[case::its_controller_without_an_object("Its controller gains 3 life.")]
    fn near_misses_stay_unread(#[case] text: &str) {
        assert_eq!(spell(text), None, "{text}");
    }

    #[test]
    fn x_reads_only_when_the_mana_cost_has_one() {
        assert!(spell_x("Draw X cards.").is_some());
        assert!(spell("Draw X cards.").is_none());
    }
}

mod damage {
    //! Goal: "deals N damage" reads for any target, one creature, a player or planeswalker, each opponent and each creature, and divided damage, a second target, "for each" and "equal to" stay unread.
    use super::*;

    #[rstest]
    #[case::bolt("~ deals 3 damage to any target.", false, r#"{"Spell":{"targets":[{"what":"AnyTarget"}],"effects":[{"DealDamage":{"to":{"Target":0},"amount":3}}]}}"#)]
    #[case::creature("~ deals 4 damage to target creature.", false, r#"{"Spell":{"targets":[{"what":{"Object":[{"Type":"Creature"}]}}],"effects":[{"DealDamage":{"to":{"Target":0},"amount":4}}]}}"#)]
    #[case::creature_or_planeswalker("~ deals 4 damage to target creature or planeswalker.", false, r#"{"Spell":{"targets":[{"what":{"Object":[{"AnyOf":[{"Type":"Creature"},{"Type":"Planeswalker"}]}]}}],"effects":[{"DealDamage":{"to":{"Target":0},"amount":4}}]}}"#)]
    #[case::player_or_planeswalker("~ deals 3 damage to target player or planeswalker.", false, r#"{"Spell":{"targets":[{"what":{"Either":["Player",{"Object":[{"Type":"Planeswalker"}]}]}}],"effects":[{"DealDamage":{"to":{"Target":0},"amount":3}}]}}"#)]
    #[case::opponent("~ deals 1 damage to target opponent.", false, r#"{"Spell":{"targets":[{"what":"Opponent"}],"effects":[{"DealDamage":{"to":{"Target":0},"amount":1}}]}}"#)]
    #[case::each_opponent(
        "~ deals 1 damage to each opponent.",
        false,
        r#"{"Spell":{"effects":[{"DealDamage":{"to":"Opponents","amount":1}}]}}"#
    )]
    #[case::each_creature("~ deals 2 damage to each creature.", false, r#"{"Spell":{"effects":[{"DealDamage":{"to":{"Each":[{"Type":"Creature"}]},"amount":2}}]}}"#)]
    #[case::x("~ deals X damage to any target.", true, r#"{"Spell":{"targets":[{"what":"AnyTarget"}],"effects":[{"DealDamage":{"to":{"Target":0},"amount":"X"}}]}}"#)]
    fn the_wordings_read_into_the_dsl(
        #[case] text: &str,
        #[case] x_free: bool,
        #[case] json: &str,
    ) {
        reads_as(text, ClauseKind::Spell, x_free, json);
    }

    #[rstest]
    #[case("~ deals 1 damage to any target.")]
    #[case("~ deals 2 damage to any target.")]
    #[case("~ deals 5 damage to any target.")]
    #[case("~ deals 2 damage to target creature.")]
    #[case("~ deals 5 damage to target creature.")]
    #[case("~ deals 3 damage to target attacking creature.")]
    #[case("~ deals 3 damage to target creature you don't control.")]
    #[case("~ deals 2 damage to target creature an opponent controls.")]
    #[case("~ deals 3 damage to target creature with flying.")]
    #[case("~ deals 2 damage to each opponent.")]
    #[case("~ deals 3 damage to each creature your opponents control.")]
    #[case("~ deals 2 damage to you.")]
    #[case("~ deals 1 damage to each player.")]
    #[case("~ deals 2 damage to target creature or planeswalker.")]
    #[case("~ deals 6 damage to target creature with mana value 3 or less.")]
    #[case("~ deals 3 damage to target Goblin.")]
    fn more_oracle_wordings_read_and_write_back(#[case] text: &str) {
        golden(text, ClauseKind::Spell, false);
    }

    #[rstest]
    #[case::divided("~ deals 2 damage divided as you choose among one or two targets.")]
    #[case::two_targets("~ deals 2 damage to target creature and 1 damage to target player.")]
    #[case::for_each("~ deals 1 damage to any target for each creature you control.")]
    #[case::equal_to("~ deals damage equal to its power to target creature.")]
    #[case::equal_to_a_count_held_by_a_player(
        "~ deals damage to any target equal to the number of cards in their hand."
    )]
    #[case::each_of_up_to("~ deals 2 damage to each of up to two target creatures.")]
    #[case::if_clause(
        "~ deals 3 damage to any target. If you control a Goblin, ~ deals 5 damage instead."
    )]
    #[case::unless("~ deals 3 damage to target creature unless its controller pays {2}.")]
    #[case::two_subtypes("~ deals 3 damage to each Goblin and each Elf.")]
    #[case::any_number("~ deals 1 damage to any number of targets.")]
    #[case::x_without_an_x_cost("~ deals X damage to any target.")]
    #[case::no_amount("~ deals damage to any target.")]
    #[case::not_the_card("Target creature deals 3 damage to any target.")]
    #[case::it("It deals 3 damage to any target.")]
    #[case::fights(
        "Target creature you control deals damage equal to its power to target creature you don't control."
    )]
    fn near_misses_stay_unread(#[case] text: &str) {
        assert_eq!(spell(text), None, "{text}");
    }
}

mod removal {
    //! Goal: destroy, exile and counter read with the common targets, "all" and the regeneration sentence, and "unless", "if", a second target, a subtype and a zone stay unread.
    use super::*;

    #[rstest]
    #[case::doom_blade("Destroy target nonblack creature.", r#"{"Spell":{"targets":[{"what":{"Object":[{"Type":"Creature"},{"Not":{"Color":"Black"}}]}}],"effects":[{"Destroy":{"what":{"Target":0}}}]}}"#)]
    #[case::counterspell("Counter target spell.", r#"{"Spell":{"targets":[{"what":{"Object":["Spell"]}}],"effects":[{"Counter":{"what":{"Target":0}}}]}}"#)]
    #[case::wrath("Destroy all creatures. They can't be regenerated.", r#"{"Spell":{"effects":[{"Destroy":{"what":{"Each":[{"Type":"Creature"}]},"regenerate":false}}]}}"#)]
    #[case::swords("Exile target creature. Its controller gains life equal to its power.", r#"{"Spell":{"targets":[{"what":{"Object":[{"Type":"Creature"}]}}],"effects":[{"Exile":{"what":{"Target":0}}},{"GainLife":{"who":{"ControllerOf":{"Target":0}},"amount":{"Stat":{"of":{"Target":0},"stat":"Power"}}}}]}}"#)]
    #[case::no_regeneration("Destroy target creature. It can't be regenerated.", r#"{"Spell":{"targets":[{"what":{"Object":[{"Type":"Creature"}]}}],"effects":[{"Destroy":{"what":{"Target":0},"regenerate":false}}]}}"#)]
    #[case::any_permanent("Destroy target permanent.", r#"{"Spell":{"targets":[{"what":{"Object":[]}}],"effects":[{"Destroy":{"what":{"Target":0}}}]}}"#)]
    #[case::artifact_or_enchantment("Destroy target artifact or enchantment.", r#"{"Spell":{"targets":[{"what":{"Object":[{"AnyOf":[{"Type":"Artifact"},{"Type":"Enchantment"}]}]}}],"effects":[{"Destroy":{"what":{"Target":0}}}]}}"#)]
    #[case::an_opponent_controls("Destroy target creature an opponent controls.", r#"{"Spell":{"targets":[{"what":{"Object":[{"Type":"Creature"},{"ControlledBy":"Opponents"}]}}],"effects":[{"Destroy":{"what":{"Target":0}}}]}}"#)]
    #[case::up_to_two("Exile up to two target creatures.", r#"{"Spell":{"targets":[{"what":{"Object":[{"Type":"Creature"}]},"bounds":{"min":0,"max":2}}],"effects":[{"Exile":{"what":{"Target":0}}}]}}"#)]
    #[case::noncreature_spell("Counter target noncreature spell.", r#"{"Spell":{"targets":[{"what":{"Object":["Spell",{"Not":{"Type":"Creature"}}]}}],"effects":[{"Counter":{"what":{"Target":0}}}]}}"#)]
    #[case::creature_spell("Counter target creature spell.", r#"{"Spell":{"targets":[{"what":{"Object":["Spell",{"Type":"Creature"}]}}],"effects":[{"Counter":{"what":{"Target":0}}}]}}"#)]
    #[case::all_you_control("Exile all creatures you control.", r#"{"Spell":{"effects":[{"Exile":{"what":{"Each":[{"Type":"Creature"},{"ControlledBy":"You"}]}}}]}}"#)]
    fn the_wordings_read_into_the_dsl(#[case] text: &str, #[case] json: &str) {
        reads_as(text, ClauseKind::Spell, false, json);
    }

    #[rstest]
    #[case("Destroy target creature.")]
    #[case("Destroy target artifact.")]
    #[case("Destroy target enchantment.")]
    #[case("Destroy target land.")]
    #[case("Destroy target planeswalker.")]
    #[case("Destroy target nonland permanent.")]
    #[case("Destroy target nonartifact creature.")]
    #[case("Destroy target creature or planeswalker.")]
    #[case("Destroy target tapped creature.")]
    #[case("Destroy target untapped creature.")]
    #[case("Destroy target attacking creature.")]
    #[case("Destroy target creature with flying.")]
    #[case("Destroy target creature you don't control.")]
    #[case("Destroy target red creature.")]
    #[case("Destroy target legendary creature.")]
    #[case("Destroy target creature with power 4 or greater.")]
    #[case("Destroy target creature with mana value 3 or less.")]
    #[case("Destroy all creatures.")]
    #[case("Destroy all lands.")]
    #[case("Destroy all artifacts.")]
    #[case("Destroy all nonland permanents.")]
    #[case("Destroy all artifacts and enchantments.")]
    #[case("Destroy all creatures your opponents control.")]
    #[case("Destroy all creatures. They can't be regenerated.")]
    #[case("Destroy all lands. They can't be regenerated.")]
    #[case("Destroy target land. It can't be regenerated.")]
    #[case("Exile target creature.")]
    #[case("Exile target nonland permanent.")]
    #[case("Exile target artifact or enchantment.")]
    #[case("Exile target creature or planeswalker.")]
    #[case("Destroy target Goblin.")]
    #[case("Exile target Clue.")]
    #[case("Exile all creatures.")]
    #[case("Exile all nonland permanents.")]
    #[case("Counter target creature spell.")]
    #[case("Counter target instant or sorcery spell.")]
    #[case("Counter target red spell.")]
    #[case("Counter target spell with mana value 3 or less.")]
    #[case("Counter target artifact spell.")]
    fn more_oracle_wordings_read_and_write_back(#[case] text: &str) {
        golden(text, ClauseKind::Spell, false);
    }

    #[rstest]
    #[case::unless_destroy("Destroy target creature unless its controller pays {3}.")]
    #[case::if_clause("Destroy target creature if it's attacking.")]
    #[case::a_second_target("Destroy target creature and target land.")]
    #[case::each("Destroy each creature.")]
    #[case::that("Destroy target creature that was dealt damage this turn.")]
    #[case::a_zone("Exile target card from a graveyard.")]
    #[case::an_ability("Counter target activated ability.")]
    #[case::a_player("Destroy target player.")]
    #[case::any_target("Destroy any target.")]
    #[case::counter_a_creature("Counter target creature.")]
    #[case::counter_all("Counter all spells.")]
    #[case::regeneration_without_destroy("Exile target creature. It can't be regenerated.")]
    #[case::regeneration_twice(
        "Destroy target creature. It can't be regenerated. It can't be regenerated."
    )]
    #[case::wrong_pronoun("Destroy all creatures. It can't be regenerated.")]
    #[case::controller_of_all("Destroy all creatures. Its controller gains 3 life.")]
    #[case::exile_all_gain("Exile all creatures. Its controller gains life equal to its power.")]
    #[case::life_for_spell("Counter target spell. Its controller gains life equal to its power.")]
    #[case::a_second_sentence(
        "Destroy target creature. Draw a card, then discard two cards at will."
    )]
    #[case::and_gain("Destroy target creature and gain 3 life.")]
    #[case::mixed_connector("Destroy all artifacts or enchantments.")]
    #[case::two_types_adjective("Destroy target artifact creature.")]
    fn near_misses_stay_unread(#[case] text: &str) {
        assert_eq!(spell(text), None, "{text}");
    }
}

mod scry {
    //! Goal: "Scry N" reads alone and chained with "then" to the steps of "you", and a scry for another player, a second sentence or a following choice stays unread.
    use super::*;

    #[rstest]
    #[case::opt(
        "Scry 1, then draw a card.",
        false,
        r#"{"Spell":{"effects":[{"Scry":{}},{"Draw":{}}]}}"#
    )]
    #[case::two("Scry 2.", false, r#"{"Spell":{"effects":[{"Scry":{"count":2}}]}}"#)]
    #[case::preordain(
        "Scry 2, then draw a card.",
        false,
        r#"{"Spell":{"effects":[{"Scry":{"count":2}},{"Draw":{}}]}}"#
    )]
    #[case::x("Scry X.", true, r#"{"Spell":{"effects":[{"Scry":{"count":"X"}}]}}"#)]
    fn the_wordings_read_into_the_dsl(
        #[case] text: &str,
        #[case] x_free: bool,
        #[case] json: &str,
    ) {
        reads_as(text, ClauseKind::Spell, x_free, json);
    }

    #[rstest]
    #[case("Scry 1.")]
    #[case("Scry 3.")]
    #[case("Scry 4.")]
    #[case("Scry 2, then draw two cards.")]
    #[case("Draw a card, then scry 1.")]
    #[case("Scry 1, then scry 1.")]
    #[case("Scry 1. Draw a card.")]
    #[case("Scry 1.\nDraw a card.")]
    #[case("Draw two cards.\nDiscard a card.")]
    #[case("Scry 1, then draw a card, then discard a card.")]
    #[case("Draw a card, then discard a card.")]
    fn more_oracle_wordings_read_and_write_back(#[case] text: &str) {
        golden(text, ClauseKind::Spell, false);
    }

    #[rstest]
    #[case::zero("Scry 0.")]
    #[case::another_player("Target player scries 2.")]
    #[case::each_player("Each player scries 1.")]
    #[case::then_a_choice("Scry 2, then you may sacrifice a creature.")]
    #[case::a_second_sentence(
        "Scry 2. You may put a card from your hand on the bottom of your library."
    )]
    #[case::a_comma("Scry 2, draw a card.")]
    #[case::then_alone("Scry 2, then.")]
    #[case::x_without_an_x_cost("Scry X.")]
    #[case::no_number("Scry.")]
    fn near_misses_stay_unread(#[case] text: &str) {
        assert_eq!(spell(text), None, "{text}");
    }
}

mod pump {
    //! Goal: "gets +N/+N" and "gains a keyword until end of turn" read for a target creature, the card itself and the creatures you control, and "for each", a different duration, a second target and a conditional stay unread.
    use super::*;

    #[rstest]
    #[case::giant_growth("Target creature gets +3/+3 until end of turn.", false, r#"{"Spell":{"targets":[{"what":{"Object":[{"Type":"Creature"}]}}],"effects":[{"Continuous":{"affected":{"Target":0},"mods":[{"ModifyPT":{"power":3,"toughness":3}}],"duration":"UntilEndOfTurn"}}]}}"#)]
    #[case::negative("Target creature gets -2/-2 until end of turn.", false, r#"{"Spell":{"targets":[{"what":{"Object":[{"Type":"Creature"}]}}],"effects":[{"Continuous":{"affected":{"Target":0},"mods":[{"ModifyPT":{"power":-2,"toughness":-2}}],"duration":"UntilEndOfTurn"}}]}}"#)]
    #[case::mixed("Target creature gets +3/-3 until end of turn.", false, r#"{"Spell":{"targets":[{"what":{"Object":[{"Type":"Creature"}]}}],"effects":[{"Continuous":{"affected":{"Target":0},"mods":[{"ModifyPT":{"power":3,"toughness":-3}}],"duration":"UntilEndOfTurn"}}]}}"#)]
    #[case::x("Target creature gets +X/+X until end of turn.", true, r#"{"Spell":{"targets":[{"what":{"Object":[{"Type":"Creature"}]}}],"effects":[{"Continuous":{"affected":{"Target":0},"mods":[{"ModifyPT":{"power":"X","toughness":"X"}}],"duration":"UntilEndOfTurn"}}]}}"#)]
    #[case::flying("Target creature gains flying until end of turn.", false, r#"{"Spell":{"targets":[{"what":{"Object":[{"Type":"Creature"}]}}],"effects":[{"Continuous":{"affected":{"Target":0},"mods":[{"AddKeyword":"Flying"}],"duration":"UntilEndOfTurn"}}]}}"#)]
    #[case::both("Target creature gets +1/+1 and gains trample until end of turn.", false, r#"{"Spell":{"targets":[{"what":{"Object":[{"Type":"Creature"}]}}],"effects":[{"Continuous":{"affected":{"Target":0},"mods":[{"ModifyPT":{"power":1,"toughness":1}},{"AddKeyword":"Trample"}],"duration":"UntilEndOfTurn"}}]}}"#)]
    #[case::your_creatures("Creatures you control get +1/+1 until end of turn.", false, r#"{"Spell":{"effects":[{"Continuous":{"affected":{"Each":[{"Type":"Creature"},{"ControlledBy":"You"}]},"mods":[{"ModifyPT":{"power":1,"toughness":1}}],"duration":"UntilEndOfTurn"}}]}}"#)]
    #[case::the_card("~ gets +2/+2 until end of turn.", false, r#"{"Spell":{"effects":[{"Continuous":{"affected":"This","mods":[{"ModifyPT":{"power":2,"toughness":2}}],"duration":"UntilEndOfTurn"}}]}}"#)]
    #[case::unblockable_this_turn("Target creature can't be blocked this turn.", false, r#"{"Spell":{"targets":[{"what":{"Object":[{"Type":"Creature"}]}}],"effects":[{"Continuous":{"affected":{"Target":0},"mods":["CantBeBlocked"],"duration":"UntilEndOfTurn"}}]}}"#)]
    #[case::your_creatures_unblockable("Creatures you control can't be blocked this turn.", false, r#"{"Spell":{"effects":[{"Continuous":{"affected":{"Each":[{"Type":"Creature"},{"ControlledBy":"You"}]},"mods":["CantBeBlocked"],"duration":"UntilEndOfTurn"}}]}}"#)]
    fn the_wordings_read_into_the_dsl(
        #[case] text: &str,
        #[case] x_free: bool,
        #[case] json: &str,
    ) {
        reads_as(text, ClauseKind::Spell, x_free, json);
    }

    #[rstest]
    #[case("Target creature gets +2/+2 until end of turn.")]
    #[case("Target creature gets +4/+4 until end of turn.")]
    #[case("Target creature gets +2/+0 until end of turn.")]
    #[case("Target creature gets +0/+3 until end of turn.")]
    #[case("Target creature gets -1/-1 until end of turn.")]
    #[case("Target creature gets +1/-1 until end of turn.")]
    #[case("Target creature gains first strike until end of turn.")]
    #[case("Target creature gains double strike until end of turn.")]
    #[case("Target creature gains lifelink until end of turn.")]
    #[case("Target creature gains indestructible until end of turn.")]
    #[case("Target creature gains flying and haste until end of turn.")]
    #[case("Target creature gains flying, trample, and haste until end of turn.")]
    #[case("Target creature you control gets +2/+2 until end of turn.")]
    #[case("Target attacking creature gets +1/+1 until end of turn.")]
    #[case("Target creature gets +2/+2 and gains flying until end of turn.")]
    #[case("Target creature gets +1/+1 and gains first strike until end of turn.")]
    #[case("Creatures you control get +2/+0 until end of turn.")]
    #[case("Creatures you control gain trample until end of turn.")]
    #[case("Creatures you control get +1/+1 and gain vigilance until end of turn.")]
    #[case("Other creatures you control get +1/+1 until end of turn.")]
    #[case("Attacking creatures get +1/+0 until end of turn.")]
    #[case("~ gains flying until end of turn.")]
    #[case("Target creature can't be blocked this turn.")]
    #[case("Target creature you control can't be blocked this turn.")]
    #[case("Creatures you control can't be blocked this turn.")]
    #[case("~ can't be blocked this turn.")]
    #[case("You may draw a card. If you do, discard a card.")]
    fn more_oracle_wordings_read_and_write_back(#[case] text: &str) {
        golden(text, ClauseKind::Spell, false);
    }

    #[rstest]
    #[case::for_each("Target creature gets +1/+1 for each creature you control until end of turn.")]
    #[case::no_duration("Target creature gets +2/+2.")]
    #[case::next_turn("Target creature gets +2/+2 until your next turn.")]
    #[case::end_of_combat("Target creature gets +2/+2 until end of combat.")]
    #[case::then_untap("Target creature gets +2/+2 until end of turn. Untap it.")]
    #[case::second_target(
        "Target creature gets +2/+2 until end of turn and target creature gets -2/-2 until end of turn."
    )]
    #[case::second_target_sentence(
        "Target creature gets +2/+2 until end of turn. Target creature gets -2/-2 until end of turn."
    )]
    #[case::conditional("If you control a Goblin, target creature gets +2/+2 until end of turn.")]
    #[case::x_without_an_x_cost("Target creature gets +X/+X until end of turn.")]
    #[case::negative_x("Target creature gets -X/-X until end of turn.")]
    #[case::not_a_number("Target creature gets +a/+b until end of turn.")]
    #[case::one_number("Target creature gets +2 until end of turn.")]
    #[case::a_player("Target player gets +2/+2 until end of turn.")]
    #[case::nothing("Target creature gets until end of turn.")]
    #[case::with_an_ability("Target creature gains \"{T}: Add {G}.\" until end of turn.")]
    fn near_misses_stay_unread(#[case] text: &str) {
        assert_eq!(spell(text), None, "{text}");
    }
}

mod sentences {
    //! Goal: a clause is one or more sentences read in order, with one target and the pronouns that refer to it, and a clause with an unread sentence is not read at all.
    use super::*;

    #[rstest]
    #[case::two_templates("Destroy target creature. You gain 3 life.")]
    #[case::pump_then_draw("Target creature gets +2/+2 until end of turn. Draw a card.")]
    #[case::damage_then_life("~ deals 2 damage to any target. You gain 2 life.")]
    #[case::three_sentences("Destroy all creatures. They can't be regenerated. You gain 3 life.")]
    fn sentences_read_in_order_when_each_one_reads(#[case] text: &str) {
        let read = golden(text, ClauseKind::Spell, false);
        assert!(read.effects().len() >= 2, "{read:?}");
    }

    #[rstest]
    #[case::a_second_target("Exile target creature. Target player draws a card.")]
    #[case::unread_second("Destroy target creature. Flip two coins.")]
    #[case::unread_first("Flip a coin. Draw a card.")]
    #[case::garbled("Destroy target creature. Draw")]
    #[case::empty("")]
    #[case::only_a_period(".")]
    #[case::a_conditional("Destroy target creature. If you do, draw a card.")]
    fn one_unread_sentence_leaves_the_whole_clause_unread(#[case] text: &str) {
        assert_eq!(spell(text), None, "{text:?}");
    }

    #[test]
    fn a_missing_closing_period_is_accepted() {
        assert!(spell("Draw a card").is_some());
        assert!(spell("Destroy target creature. Draw a card").is_some());
    }

    #[test]
    fn the_target_is_the_clauses_first_and_only_target() {
        let Some(Ability::Spell { targets, effects }) =
            spell("Exile target creature. Its controller gains life equal to its power.")
        else {
            panic!("not read");
        };
        assert_eq!(targets.len(), 1);
        assert_eq!(effects.len(), 2);
    }
}
