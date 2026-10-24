//! Goal: the writers of the sample of the names A to F give the oracle wording of energy gained by an amount, of a spell cast from your graveyard, of any number of exiled creatures that return, of the ability a token copy is given, of a search that fills the graveyard, of a counter of any spell or ability, of a step skipped for an attack, of a discarded card exiled and played, of creatures that attack as one event, of a card of yours put into your graveyard, of a bigger step for a big X, of the top card of every library cast for free, of a group that attacks, and of a choice to exile and return the source; a wrong count, kind, group, player, duration or event never reads back.
use super::*;
use crate::compile::read::static_pair_texts;

/// Whether some wording the checker accepts for the rules is the clause.
fn written(json: &str, clause: &str) -> bool {
    written_variants(&ability(json))
        .iter()
        .any(|wording| same_text(wording, clause))
}

/// "Counter target spell. You get an amount of {E} equal to its mana value."
fn siphon(stat: &str, target: &str) -> String {
    format!(
        r#"{{"Spell":{{"targets":[{{"what":{{"Object":[{target}]}}}}],"effects":[{{"Counter":{{"what":{{"Target":0}}}}}},{{"PutCounters":{{"on":"You","counter":{{"Other":"energy"}},"count":{{"Stat":{{"of":{{"Target":0}},"stat":"{stat}"}}}}}}}}]}}}}"#
    )
}

#[rstest]
#[case::mana_value(
    "ManaValue",
    r#""Spell""#,
    "Counter target spell. You get an amount of {E} equal to its mana value."
)]
#[case::the_where_clause(
    "ManaValue",
    r#""Spell""#,
    "Counter target spell. You get X {E}, where X is its mana value."
)]
fn energy_gained_by_an_amount_reads_both_ways(
    #[case] stat: &str,
    #[case] target: &str,
    #[case] clause: &str,
) {
    assert!(written(&siphon(stat, target), clause), "{clause}");
}

#[rstest]
#[case::another_stat(
    "ManaValue",
    "Counter target spell. You get an amount of {E} equal to its power."
)]
#[case::another_stat_in_the_data(
    "Power",
    "Counter target spell. You get an amount of {E} equal to its mana value."
)]
#[case::a_target_that_differs(
    "ManaValue",
    "Counter target creature spell. You get an amount of {E} equal to its mana value."
)]
#[case::a_fixed_amount("ManaValue", "Counter target spell. You get {E}{E}.")]
#[case::the_steps_swapped(
    "ManaValue",
    "You get an amount of {E} equal to its mana value. Counter target spell."
)]
fn energy_gained_by_an_amount_does_not_read_as_another_stat_target_or_order(
    #[case] stat: &str,
    #[case] clause: &str,
) {
    assert!(!written(&siphon(stat, r#""Spell""#), clause), "{clause}");
}

/// "Whenever you cast a spell from your graveyard, ~ deals 2 damage to any target."
fn vengeance(what: &str, by: &str) -> String {
    format!(
        r#"{{"Triggered":{{"when":{{"SpellCast":{{"what":[{what}],"by":"{by}"}}}},"targets":[{{"what":"AnyTarget"}}],"effects":[{{"DealDamage":{{"to":{{"Target":0}},"amount":2}}}}]}}}}"#
    )
}

const OWN_GRAVEYARD: &str = r#"{"CastFrom":"Graveyard"},{"OwnedBy":"You"}"#;

#[test]
fn a_spell_you_own_cast_from_a_graveyard_is_cast_from_your_graveyard() {
    let clause = "Whenever you cast a spell from your graveyard, ~ deals 2 damage to any target.";
    assert!(written(&vengeance(OWN_GRAVEYARD, "You"), clause));
}

#[rstest]
#[case::any_graveyard(
    r#"{"CastFrom":"Graveyard"}"#,
    "You",
    "Whenever you cast a spell from your graveyard, ~ deals 2 damage to any target."
)]
#[case::another_zone(
    OWN_GRAVEYARD,
    "You",
    "Whenever you cast a spell from your hand, ~ deals 2 damage to any target."
)]
#[case::any_player(
    OWN_GRAVEYARD,
    "EachPlayer",
    "Whenever a player casts a spell from your graveyard, ~ deals 2 damage to any target."
)]
#[case::another_amount(
    OWN_GRAVEYARD,
    "You",
    "Whenever you cast a spell from your graveyard, ~ deals 3 damage to any target."
)]
fn a_cast_from_a_graveyard_does_not_read_as_another_zone_player_or_amount(
    #[case] what: &str,
    #[case] by: &str,
    #[case] clause: &str,
) {
    assert!(!written(&vengeance(what, by), clause), "{clause}");
}

/// "Exile [target creature you control]. Return [that card] to the battlefield under its owner's control at the
/// beginning of the next end step."
fn interlude(bounds: &str) -> String {
    format!(
        r#"{{"Spell":{{"targets":[{{"what":{{"Object":[{{"Type":"Creature"}},{{"ControlledBy":"You"}}]}}{bounds}}}],"effects":[{{"Exile":{{"what":{{"Target":0}},"as":"exiled"}}}},{{"DelayedTrigger":{{"when":{{"StepBegins":{{"step":"End"}}}},"effects":[{{"Move":{{"what":{{"Bound":"exiled"}},"to":{{"Battlefield":{{}}}}}}}}]}}}}]}}}}"#
    )
}

const ANY_NUMBER: &str = r#","bounds":{"min":0,"max":null}"#;

#[rstest]
#[case::any_number(
    ANY_NUMBER,
    "Exile any number of target creatures you control. Return those cards to the battlefield under their owner's control at the beginning of the next end step."
)]
#[case::one(
    "",
    "Exile target creature you control. Return that card to the battlefield under its owner's control at the beginning of the next end step."
)]
fn the_cards_exiled_come_back_as_one_card_or_as_those_cards(
    #[case] bounds: &str,
    #[case] clause: &str,
) {
    assert!(written(&interlude(bounds), clause), "{clause}");
}

#[rstest]
#[case::that_card_for_any_number(
    ANY_NUMBER,
    "Exile any number of target creatures you control. Return that card to the battlefield under its owner's control at the beginning of the next end step."
)]
#[case::those_cards_for_one(
    "",
    "Exile target creature you control. Return those cards to the battlefield under their owner's control at the beginning of the next end step."
)]
#[case::their_owners_for_one(
    "",
    "Exile target creature you control. Return that card to the battlefield under their owner's control at the beginning of the next end step."
)]
#[case::tapped(
    ANY_NUMBER,
    "Exile any number of target creatures you control. Return those cards to the battlefield tapped under their owner's control at the beginning of the next end step."
)]
#[case::another_step(
    ANY_NUMBER,
    "Exile any number of target creatures you control. Return those cards to the battlefield under their owner's control at the beginning of your next upkeep."
)]
fn the_cards_exiled_do_not_come_back_with_another_pronoun_step_or_state(
    #[case] bounds: &str,
    #[case] clause: &str,
) {
    assert!(!written(&interlude(bounds), clause), "{clause}");
}

/// "Create X tokens that are copies of target creature you control, except they have "When this token enters, it
/// fights up to one target creature you don't control.""
fn biomancy(trigger: &str) -> String {
    format!(
        r#"{{"Spell":{{"targets":[{{"what":{{"Object":[{{"Type":"Creature"}},{{"ControlledBy":"You"}}]}}}}],"effects":[{{"CreateToken":{{"token":{{"CopyOf":{{"of":{{"Target":0}},"except":[{{"GrantAbility":{trigger}}}]}}}},"count":"X"}}}}]}}}}"#
    )
}

const FIGHTS: &str = r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"targets":[{"what":{"Object":[{"Type":"Creature"},{"Not":{"ControlledBy":"You"}}]},"bounds":{"min":0,"max":1}}],"effects":[{"Fight":{"what":"This","with":{"Target":0}}}]}}"#;

#[test]
fn an_ability_given_to_a_token_copy_names_the_token() {
    let clause = "Create X tokens that are copies of target creature you control, except they have \"When this token enters, it fights up to one target creature you don't control.\"";
    assert!(written(&biomancy(FIGHTS), clause));
}

#[rstest]
#[case::a_target_that_is_required(
    "Create X tokens that are copies of target creature you control, except they have \"When this token enters, it fights target creature you don't control.\""
)]
#[case::any_creature(
    "Create X tokens that are copies of target creature you control, except they have \"When this token enters, it fights up to one target creature.\""
)]
#[case::two_targets(
    "Create X tokens that are copies of target creature you control, except they have \"When this token enters, it fights up to two target creatures you don't control.\""
)]
#[case::one_token(
    "Create a token that's a copy of target creature you control, except it has \"When this token enters, it fights up to one target creature you don't control.\""
)]
fn an_ability_given_to_a_token_copy_does_not_read_with_other_words(#[case] clause: &str) {
    assert!(!written(&biomancy(FIGHTS), clause), "{clause}");
}

#[rstest]
#[case::up_to_three(
    "Search your library for up to three creature cards, put them into your graveyard, then shuffle."
)]
#[case::one_card(
    "Search your library for a creature card, put it into your graveyard, then shuffle."
)]
fn a_search_may_put_the_cards_into_the_graveyard(#[case] text: &str) {
    golden(text, ClauseKind::Spell, false);
}

#[rstest]
#[case::their_graveyard(
    "Search your library for up to three creature cards, put them into their graveyard, then shuffle."
)]
#[case::the_library(
    "Search your library for up to three creature cards, put them into your library, then shuffle."
)]
#[case::no_shuffle(
    "Search your library for up to three creature cards, put them into your graveyard."
)]
fn a_search_that_fills_a_graveyard_stays_with_the_table_when_it_names_another_place(
    #[case] text: &str,
) {
    assert!(spell(text).is_none(), "{text}");
}

fn counter_of(targets: &str) -> String {
    format!(
        r#"{{"Spell":{{"targets":[{{"what":{{"Object":[{targets}]}}}}],"effects":[{{"Counter":{{"what":{{"Target":0}}}}}}]}}}}"#
    )
}

const ANYTHING: &str = r#"{"AnyOf":["Spell",{"Ability":"Activated"},{"Ability":"Triggered"}]}"#;

#[test]
fn a_spell_or_an_ability_is_countered_by_one_spell() {
    let clause = "Counter target spell, activated ability, or triggered ability.";
    assert!(written(&counter_of(ANYTHING), clause));
}

#[rstest]
#[case::only_a_spell(r#""Spell""#)]
#[case::only_activated_abilities(r#"{"Ability":"Activated"}"#)]
#[case::a_spell_or_an_activated_ability(r#"{"AnyOf":["Spell",{"Ability":"Activated"}]}"#)]
fn a_counter_of_less_than_everything_is_not_worded_as_all_three(#[case] targets: &str) {
    let clause = "Counter target spell, activated ability, or triggered ability.";
    assert!(!written(&counter_of(targets), clause), "{targets}");
}

/// "Draw two cards. Then discard a card unless you attacked this turn."
fn course(draw: u32, discard: u32, op: &str, attackers: u32) -> String {
    format!(
        r#"{{"Spell":{{"effects":[{{"Draw":{{"count":{draw}}}}},{{"If":{{"condition":{{"Compare":{{"left":{{"Tallied":{{"who":"You","what":"Attackers"}}}},"op":"{op}","right":{attackers}}}}},"then":[{{"Discard":{{"count":{discard}}}}}]}}}}]}}}}"#
    )
}

#[rstest]
#[case::then_discard("Draw two cards. Then discard a card unless you attacked this turn.")]
#[case::discard("Draw two cards. Discard a card unless you attacked this turn.")]
fn a_discard_is_skipped_when_you_attacked(#[case] clause: &str) {
    assert!(written(&course(2, 1, "Lt", 1), clause), "{clause}");
}

#[rstest]
#[case::two_draws(
    course(3, 1, "Lt", 1),
    "Draw two cards. Then discard a card unless you attacked this turn."
)]
#[case::two_discards(
    course(2, 2, "Lt", 1),
    "Draw two cards. Then discard a card unless you attacked this turn."
)]
#[case::attacked_twice(
    course(2, 1, "Lt", 2),
    "Draw two cards. Then discard a card unless you attacked this turn."
)]
#[case::if_instead(
    course(2, 1, "Ge", 1),
    "Draw two cards. Then discard a card unless you attacked this turn."
)]
#[case::with_a_creature(
    course(2, 1, "Lt", 1),
    "Draw two cards. Then discard a card unless you attacked with a creature this turn."
)]
#[case::an_opponent(
    course(2, 1, "Lt", 1),
    "Draw two cards. Then discard a card unless an opponent attacked this turn."
)]
fn a_skipped_discard_does_not_read_with_another_count_or_fact(
    #[case] json: String,
    #[case] clause: &str,
) {
    assert!(!written(&json, clause), "{clause}");
}

/// "Whenever you discard a card, you may exile that card from your graveyard. If you do, you may play that card this
/// turn."
fn construct(who: &str, duration: &str, play: bool) -> String {
    format!(
        r#"{{"Triggered":{{"when":{{"Discarded":{{"what":[],"by":"{who}"}}}},"effects":[{{"May":{{"effects":[{{"Exile":{{"what":"Triggering","as":"card"}}}},{{"Rule":{{"rule":{{"MayCast":{{"from":"Exile","only":{{"Bound":"card"}},"play":{play}}}}},"duration":"{duration}"}}}}]}}}}]}}}}"#
    )
}

#[test]
fn a_discarded_card_may_be_exiled_and_played_for_the_turn() {
    let clause = "Whenever you discard a card, you may exile that card from your graveyard. If you do, you may play that card this turn.";
    assert!(written(&construct("You", "UntilEndOfTurn", true), clause));
}

#[rstest]
#[case::a_longer_permission("You", "UntilEndOfYourNextTurn", true)]
#[case::a_cast_only("You", "UntilEndOfTurn", false)]
#[case::an_opponents_discard("Opponents", "UntilEndOfTurn", true)]
fn a_discarded_card_exiled_and_played_needs_the_same_player_duration_and_permission(
    #[case] who: &str,
    #[case] duration: &str,
    #[case] play: bool,
) {
    let clause = "Whenever you discard a card, you may exile that card from your graveyard. If you do, you may play that card this turn.";
    assert!(!written(&construct(who, duration, play), clause));
}

/// "Whenever one or more creatures attack, you may have target attacking creature gain double strike until end of
/// turn."
fn heritage(what: &str) -> String {
    format!(
        r#"{{"Triggered":{{"when":{{"Batch":{{"Attacks":{{"what":[{what}]}}}}}},"targets":[{{"what":{{"Object":["Attacking",{{"Type":"Creature"}}]}}}}],"effects":[{{"May":{{"effects":[{{"Continuous":{{"affected":{{"Target":0}},"mods":[{{"AddKeyword":"DoubleStrike"}}],"duration":"UntilEndOfTurn"}}}}]}}}}]}}}}"#
    )
}

#[test]
fn creatures_that_attack_are_one_event() {
    let clause = "Whenever one or more creatures attack, you may have target attacking creature gain double strike until end of turn.";
    assert!(written(&heritage(r#"{"Type":"Creature"}"#), clause));
}

#[rstest]
#[case::you_control(
    r#"{"Type":"Creature"}"#,
    "Whenever one or more creatures you control attack, you may have target attacking creature gain double strike until end of turn."
)]
#[case::a_creature(
    r#"{"Type":"Creature"}"#,
    "Whenever a creature attacks, you may have target attacking creature gain double strike until end of turn."
)]
#[case::a_subtype(
    r#"{"Subtype":"Elf"}"#,
    "Whenever one or more creatures attack, you may have target attacking creature gain double strike until end of turn."
)]
fn creatures_that_attack_are_not_other_groups(#[case] what: &str, #[case] clause: &str) {
    assert!(!written(&heritage(what), clause), "{clause}");
}

/// "Whenever a nontoken creature is put into your graveyard from the battlefield, create a 1/1 white Spirit creature
/// token with flying."
fn souls(what: &str, to: &str) -> String {
    format!(
        r#"{{"Triggered":{{"when":{{"ZoneChange":{{"what":[{what}],"from":"Battlefield","to":"{to}"}}}},"effects":[{{"CreateToken":{{"token":{{"Custom":{{"name":"Spirit","colors":["White"],"types":["Creature"],"subtypes":["Spirit"],"power":1,"toughness":1,"abilities":[{{"Keyword":"Flying"}}]}}}}}}}}]}}}}"#
    )
}

const OWNED: &str = r#"{"Type":"Creature"},{"Not":"Token"},{"OwnedBy":"You"}"#;
const CONTROLLED: &str = r#"{"Type":"Creature"},{"Not":"Token"},{"ControlledBy":"You"}"#;

#[rstest]
#[case::put_into_your_graveyard(
    OWNED,
    "Whenever a nontoken creature is put into your graveyard from the battlefield, create a 1/1 white Spirit creature token with flying."
)]
#[case::dies_for_the_creatures_you_control(
    CONTROLLED,
    "Whenever a nontoken creature you control dies, create a 1/1 white Spirit creature token with flying."
)]
fn a_card_you_own_that_goes_to_your_graveyard_is_put_there_from_the_battlefield(
    #[case] what: &str,
    #[case] clause: &str,
) {
    assert!(written(&souls(what, "Graveyard"), clause), "{clause}");
}

#[rstest]
#[case::owned_dies(
    OWNED,
    "Whenever a nontoken creature you control dies, create a 1/1 white Spirit creature token with flying."
)]
#[case::a_creature(
    OWNED,
    "Whenever a creature is put into your graveyard from the battlefield, create a 1/1 white Spirit creature token with flying."
)]
#[case::a_graveyard(
    OWNED,
    "Whenever a nontoken creature is put into a graveyard from the battlefield, create a 1/1 white Spirit creature token with flying."
)]
#[case::controlled_put_into(
    CONTROLLED,
    "Whenever a nontoken creature is put into your graveyard from the battlefield, create a 1/1 white Spirit creature token with flying."
)]
#[case::another_token(
    OWNED,
    "Whenever a nontoken creature is put into your graveyard from the battlefield, create a 2/2 white Spirit creature token with flying."
)]
fn a_card_put_into_your_graveyard_is_not_one_you_control_or_a_creature_of_any_kind(
    #[case] what: &str,
    #[case] clause: &str,
) {
    assert!(!written(&souls(what, "Graveyard"), clause), "{clause}");
}

/// "Create X 2/2 white Soldier creature tokens with vigilance. If X is 10 or more, also create X 4/4 white Angel
/// creature tokens with flying and vigilance."
fn finale(op: &str, least: u32, angel: u32) -> String {
    format!(
        r#"{{"Spell":{{"effects":[{{"CreateToken":{{"token":{{"Custom":{{"name":"Soldier","colors":["White"],"types":["Creature"],"subtypes":["Soldier"],"power":2,"toughness":2,"abilities":[{{"Keyword":"Vigilance"}}]}}}},"count":"X"}}}},{{"If":{{"condition":{{"Compare":{{"left":"X","op":"{op}","right":{least}}}}},"then":[{{"CreateToken":{{"token":{{"Custom":{{"name":"Angel","colors":["White"],"types":["Creature"],"subtypes":["Angel"],"power":{angel},"toughness":{angel},"abilities":[{{"Keyword":"Flying"}},{{"Keyword":"Vigilance"}}]}}}},"count":"X"}}}}]}}}}]}}}}"#
    )
}

#[rstest]
#[case::digits(
    10,
    "Create X 2/2 white Soldier creature tokens with vigilance. If X is 10 or more, also create X 4/4 white Angel creature tokens with flying and vigilance."
)]
#[case::three(
    3,
    "Create X 2/2 white Soldier creature tokens with vigilance. If X is 3 or more, also create X 4/4 white Angel creature tokens with flying and vigilance."
)]
#[case::in_words(
    3,
    "Create X 2/2 white Soldier creature tokens with vigilance. If X is three or more, also create X 4/4 white Angel creature tokens with flying and vigilance."
)]
fn a_bigger_x_makes_more_tokens(#[case] least: u32, #[case] clause: &str) {
    assert!(written(&finale("Ge", least, 4), clause), "{clause}");
}

#[rstest]
#[case::another_least(
    10,
    "Ge",
    4,
    "Create X 2/2 white Soldier creature tokens with vigilance. If X is 5 or more, also create X 4/4 white Angel creature tokens with flying and vigilance."
)]
#[case::at_most(
    10,
    "Le",
    4,
    "Create X 2/2 white Soldier creature tokens with vigilance. If X is 10 or more, also create X 4/4 white Angel creature tokens with flying and vigilance."
)]
#[case::another_size(
    10,
    "Ge",
    3,
    "Create X 2/2 white Soldier creature tokens with vigilance. If X is 10 or more, also create X 4/4 white Angel creature tokens with flying and vigilance."
)]
fn a_bigger_x_does_not_read_with_another_limit_or_token(
    #[case] least: u32,
    #[case] op: &str,
    #[case] angel: u32,
    #[case] clause: &str,
) {
    assert!(!written(&finale(op, least, angel), clause), "{clause}");
}

/// "Whenever ~ attacks, exile the top card of each player's library, then you may cast any number of spells from among
/// those cards without paying their mana costs."
fn etali(who: &str, count: u32, free: bool) -> String {
    format!(
        r#"{{"Triggered":{{"when":{{"Attacks":{{"what":["This"]}}}},"effects":[{{"ExileTop":{{"who":"{who}","count":{count},"as":"exiled"}}}},{{"CastFromZone":{{"what":{{"Bound":"exiled"}},"free":{free}}}}}]}}}}"#
    )
}

#[test]
fn the_top_card_of_every_library_may_be_cast_for_free() {
    let clause = "Whenever ~ attacks, exile the top card of each player's library, then you may cast any number of spells from among those cards without paying their mana costs.";
    assert!(written(&etali("EachPlayer", 1, true), clause));
}

#[rstest]
#[case::each_opponent("Opponents", 1, true)]
#[case::two_cards("EachPlayer", 2, true)]
#[case::not_free("EachPlayer", 1, false)]
fn the_top_cards_cast_for_free_need_every_library_one_card_and_no_cost(
    #[case] who: &str,
    #[case] count: u32,
    #[case] free: bool,
) {
    let clause = "Whenever ~ attacks, exile the top card of each player's library, then you may cast any number of spells from among those cards without paying their mana costs.";
    assert!(!written(&etali(who, count, free), clause));
}

fn creatures(group: &str) -> String {
    format!(r#"{{"Each":[{group}]}}"#)
}

fn avatar(statics: &str, attackers: &str) -> (Ability, Ability) {
    let first = format!(
        r#"{{"Static":{{"affected":{statics},"mods":[{{"AddKeyword":"DoubleStrike"}}]}}}}"#
    );
    let second = format!(r#"{{"Rule":{{"rule":{{"MustAttack":{attackers}}}}}}}"#);
    (ability(&first), ability(&second))
}

#[test]
fn a_group_that_has_a_keyword_and_must_attack_is_one_sentence() {
    let group = creatures(r#"{"Type":"Creature"}"#);
    let (first, second) = avatar(&group, &group);
    assert!(
        static_pair_texts(&first, &second)
            .iter()
            .any(|text| same_text(
                text,
                "All creatures have double strike and attack each combat if able."
            ))
    );
}

#[rstest]
#[case::another_group(
    creatures(r#"{"Type":"Creature"}"#),
    creatures(r#"{"Type":"Creature"},{"ControlledBy":"You"}"#)
)]
#[case::the_card_itself(creatures(r#"{"Type":"Creature"}"#), r#""This""#.to_owned())]
fn a_group_and_a_rule_for_another_group_are_not_one_sentence(
    #[case] statics: String,
    #[case] attackers: String,
) {
    let (first, second) = avatar(&statics, &attackers);
    assert_eq!(static_pair_texts(&first, &second), Vec::<String>::new());
}

#[test]
fn a_group_that_must_attack_with_a_condition_is_not_joined() {
    let group = creatures(r#"{"Type":"Creature"}"#);
    let first = ability(&format!(
        r#"{{"Static":{{"affected":{group},"mods":[{{"AddKeyword":"DoubleStrike"}}],"condition":"YourTurn"}}}}"#
    ));
    let second = ability(&format!(
        r#"{{"Rule":{{"rule":{{"MustAttack":{group}}}}}}}"#
    ));
    assert_eq!(static_pair_texts(&first, &second), Vec::<String>::new());
}

/// "At the beginning of your upkeep, you may exile ~. If you do, return it to the battlefield under its owner's control."
fn returning(under: &str, tapped: bool) -> String {
    format!(
        r#"{{"Triggered":{{"when":{{"StepBegins":{{"step":"Upkeep","whose":"You"}}}},"effects":[{{"May":{{"effects":[{{"Exile":{{"what":"This","as":"self"}}}},{{"Move":{{"what":{{"Bound":"self"}},"to":{{"Battlefield":{{{under}"tapped":{tapped}}}}}}}}}]}}}}]}}}}"#
    )
}

#[rstest]
#[case::its_owners_control("", "its owner's")]
#[case::your_control(r#""under":"You","#, "your")]
fn a_permanent_may_exile_itself_and_return(#[case] under: &str, #[case] who: &str) {
    let clause = format!(
        "At the beginning of your upkeep, you may exile ~. If you do, return it to the battlefield under {who} control."
    );
    assert!(written(&returning(under, false), &clause), "{clause}");
}

#[rstest]
#[case::your_control_for_the_owner("", false, "your")]
#[case::tapped("", true, "its owner's")]
fn a_permanent_that_returns_does_not_read_with_another_controller_or_state(
    #[case] under: &str,
    #[case] tapped: bool,
    #[case] who: &str,
) {
    let clause = format!(
        "At the beginning of your upkeep, you may exile ~. If you do, return it to the battlefield under {who} control."
    );
    assert!(!written(&returning(under, tapped), &clause), "{clause}");
}

/// "Target player draws X cards plus an additional card for each time they've cast a commander from the command zone
/// this game."
fn insight(who: u8, amount: &str) -> String {
    format!(
        r#"{{"Spell":{{"targets":[{{"what":"Player"}}],"effects":[{{"Draw":{{"who":{{"Target":{who}}},"count":{amount}}}}}]}}}}"#
    )
}

const INSIGHT: &str = "Target player draws X cards plus an additional card for each time they've cast a commander from the command zone this game.";

#[test]
fn x_cards_and_one_for_each_cast_of_a_commander_are_one_draw() {
    let amount = r#"{"Sum":["X",{"CommanderCasts":{"Target":0}}]}"#;
    assert!(written(&insight(0, amount), INSIGHT));
}

#[rstest]
#[case::the_casts_of_another_player(0, r#"{"Sum":["X",{"CommanderCasts":"You"}]}"#)]
#[case::only_x(0, r#""X""#)]
#[case::two_for_each(
    0,
    r#"{"Sum":["X",{"Times":{"by":2,"of":{"CommanderCasts":{"Target":0}}}}]}"#
)]
#[case::the_controller_draws(1, r#"{"Sum":["X",{"CommanderCasts":{"Target":0}}]}"#)]
fn a_draw_for_the_casts_of_a_commander_needs_x_and_the_casts_of_the_same_player(
    #[case] who: u8,
    #[case] amount: &str,
) {
    assert!(!written(&insight(who, amount), INSIGHT));
}

/// "Whenever you gain life, you may pay {1}{W}. If you do, put a +1/+1 counter on target creature for each 1 life you
/// gained."
fn cradle(counter: &str, trigger: &str) -> String {
    format!(
        r#"{{"Triggered":{{"when":{trigger},"targets":[{{"what":{{"Object":[{{"Type":"Creature"}}]}}}}],"effects":[{{"Pay":{{"cost":"{{1}}{{W}}","paid":[{{"PutCounters":{{"on":{{"Target":0}},"counter":{counter},"count":"Triggering"}}}}]}}}}]}}}}"#
    )
}

const GAIN: &str = r#"{"LifeGained":{"who":"You"}}"#;
const CRADLE: &str = "Whenever you gain life, you may pay {1}{W}. If you do, put a +1/+1 counter on target creature for each 1 life you gained.";

#[test]
fn counters_for_each_life_gained_are_counted_by_the_life() {
    assert!(written(&cradle(r#""+1/+1""#, GAIN), CRADLE));
    let lost = CRADLE.replace("+1/+1", "-1/-1");
    assert!(written(&cradle(r#""-1/-1""#, GAIN), &lost));
}

#[rstest]
#[case::another_kind(r#""-1/-1""#, GAIN, CRADLE)]
#[case::a_named_kind(
    r#"{"Other":"rally"}"#,
    GAIN,
    "Whenever you gain life, you may pay {1}{W}. If you do, put a rally counter on target creature for each 1 life you gained."
)]
#[case::an_opponents_gain(
    r#""+1/+1""#,
    r#"{"LifeGained":{"who":"Opponents"}}"#,
    "Whenever an opponent gains life, you may pay {1}{W}. If you do, put a +1/+1 counter on target creature for each 1 life you gained."
)]
#[case::for_each_2_life(
    r#""+1/+1""#,
    GAIN,
    "Whenever you gain life, you may pay {1}{W}. If you do, put a +1/+1 counter on target creature for each 2 life you gained."
)]
fn counters_for_each_life_do_not_read_with_another_kind_player_or_amount(
    #[case] counter: &str,
    #[case] trigger: &str,
    #[case] clause: &str,
) {
    assert!(!written(&cradle(counter, trigger), clause), "{clause}");
}

/// "Whenever a creature deals combat damage to one of your opponents, its controller may draw a card."
fn edric(chooser: &str, to: &str) -> String {
    format!(
        r#"{{"Triggered":{{"when":{{"DealsDamage":{{"source":[{{"Type":"Creature"}}],"to":"{to}","combat":true}}}},"effects":[{{"May":{{"who":{chooser},"effects":[{{"Draw":{{"who":{{"ControllerOf":"Triggering"}}}}}}]}}}}]}}}}"#
    )
}

const CONTROLLER: &str = r#"{"ControllerOf":"Triggering"}"#;

#[rstest]
#[case::one_of_your_opponents(
    "Whenever a creature deals combat damage to one of your opponents, its controller may draw a card."
)]
#[case::an_opponent(
    "Whenever a creature deals combat damage to an opponent, its controller may draw a card."
)]
fn the_controller_of_a_creature_may_draw_for_its_damage(#[case] clause: &str) {
    assert!(written(&edric(CONTROLLER, "Opponents"), clause), "{clause}");
}

#[rstest]
#[case::you_may_draw(
    r#""You""#,
    "Whenever a creature deals combat damage to one of your opponents, you may draw a card."
)]
#[case::a_draw_without_the_choice(
    CONTROLLER,
    "Whenever a creature deals combat damage to one of your opponents, its controller draws a card."
)]
#[case::damage_to_you(
    CONTROLLER,
    "Whenever a creature deals combat damage to you, its controller may draw a card."
)]
fn a_draw_by_the_controller_needs_the_choice_and_the_same_player(
    #[case] chooser: &str,
    #[case] clause: &str,
) {
    assert!(!written(&edric(chooser, "Opponents"), clause), "{clause}");
}

/// "Whenever a permanent deals damage to you, return it to its owner's hand."
fn field(to: &str, destination: &str) -> String {
    format!(
        r#"{{"Triggered":{{"when":{{"DealsDamage":{{"source":[],"to":"{to}"}}}},"effects":[{{"Move":{{"what":"Triggering","to":{destination}}}}}]}}}}"#
    )
}

#[test]
fn the_permanent_that_dealt_damage_returns_to_its_owners_hand() {
    let clause = "Whenever a permanent deals damage to you, return it to its owner's hand.";
    assert!(written(&field("You", r#""Hand""#), clause));
}

#[rstest]
#[case::the_graveyard("You", r#""Graveyard""#)]
#[case::damage_to_an_opponent("Opponents", r#""Hand""#)]
fn the_permanent_that_dealt_damage_returns_only_to_the_hand_for_damage_to_you(
    #[case] to: &str,
    #[case] destination: &str,
) {
    let clause = "Whenever a permanent deals damage to you, return it to its owner's hand.";
    assert!(!written(&field(to, destination), clause));
}

/// "When this creature enters, double the number of each kind of counter on any number of target permanents."
fn skate(bounds: &str, counter: &str) -> String {
    format!(
        r#"{{"Triggered":{{"when":{{"ZoneChange":{{"what":["This"],"to":"Battlefield"}}}},"targets":[{{"what":{{"Object":[]}}{bounds}}}],"effects":[{{"MultiplyCounters":{{"on":{{"Target":0}}{counter}}}}}]}}}}"#
    )
}

const SKATE: &str = "When this creature enters, double the number of each kind of counter on any number of target permanents.";

#[test]
fn the_counters_of_any_number_of_permanents_are_doubled() {
    assert!(written(
        &skate(r#","bounds":{"min":0,"max":null}"#, ""),
        SKATE
    ));
}

#[rstest]
#[case::one_target("", "")]
#[case::up_to_two(r#","bounds":{"min":0,"max":2}"#, "")]
#[case::one_kind(r#","bounds":{"min":0,"max":null}"#, r#","counter":"+1/+1""#)]
#[case::triple(r#","bounds":{"min":0,"max":null}"#, r#","factor":3"#)]
fn doubling_counters_needs_any_number_of_targets_and_every_kind(
    #[case] bounds: &str,
    #[case] counter: &str,
) {
    assert!(!written(&skate(bounds, counter), SKATE));
}
