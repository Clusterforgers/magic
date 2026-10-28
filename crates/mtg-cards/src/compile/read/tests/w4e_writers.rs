//! Goal: the writers of zone moves, counters, modes and intervening conditions (r3w5) give the printed wording of each shape from the data, and a changed number, kind, player, zone, counter or condition never reads back.
use super::*;

fn written(json: &str, clause: &str) -> bool {
    written_variants(&ability(json))
        .iter()
        .any(|wording| same_text(wording, clause))
}

fn returned(most: u32, counter: &str) -> String {
    format!(
        r#"{{"Spell":{{"effects":[{{"Move":{{"to":{{"Battlefield":{{"counters":[["{counter}",1]],"under":"You"}}}},"what":{{"Target":0}}}}}}],"targets":[{{"bounds":{{"max":{most},"min":1}},"what":{{"Object":[{{"Type":"Creature"}},{{"In":"Graveyard"}}]}}}}]}}}}"#
    )
}

#[rstest]
#[case::three(
    3,
    "-1/-1",
    "Put one, two, or three target creature cards from graveyards onto the battlefield under your control. Each of them enters with an additional -1/-1 counter on it.",
    true
)]
#[case::two(
    2,
    "+1/+1",
    "Put one or two target creature cards from graveyards onto the battlefield under your control. Each of them enters with an additional +1/+1 counter on it.",
    true
)]
#[case::other_number(
    2,
    "-1/-1",
    "Put one, two, or three target creature cards from graveyards onto the battlefield under your control. Each of them enters with an additional -1/-1 counter on it.",
    false
)]
#[case::other_kind(
    3,
    "+1/+1",
    "Put one, two, or three target creature cards from graveyards onto the battlefield under your control. Each of them enters with an additional -1/-1 counter on it.",
    false
)]
fn several_targets_returned_with_a_counter(
    #[case] most: u32,
    #[case] counter: &str,
    #[case] clause: &str,
    #[case] reads: bool,
) {
    assert_eq!(written(&returned(most, counter), clause), reads, "{clause}");
}

const SNAP: &str = r#"{"Spell":{"effects":[{"RemoveCounters":{"count":{"AnyCounters":{"Each":[]}},"from":{"Each":[]}}},{"Exile":{"what":{"Each":["Token"]}}}]}}"#;
const HEXMAGE: &str = r#"{"Activated":{"cost":["SacrificeSelf"],"effects":[{"RemoveCounters":{"count":{"AnyCounters":{"Target":0}},"from":{"Target":0}}}],"targets":[{"what":{"Object":[]}}]}}"#;
const ONE_KIND: &str = r#"{"Activated":{"cost":["SacrificeSelf"],"effects":[{"RemoveCounters":{"counter":"+1/+1","count":{"AnyCounters":{"Target":0}},"from":{"Target":0}}}],"targets":[{"what":{"Object":[]}}]}}"#;

#[rstest]
#[case::sweep(
    SNAP,
    "Remove all counters from all permanents and exile all tokens.",
    true
)]
#[case::sweep_destroys(
    SNAP,
    "Remove all counters from all permanents and destroy all tokens.",
    false
)]
#[case::target(
    HEXMAGE,
    "Sacrifice ~: Remove all counters from target permanent.",
    true
)]
#[case::target_creature(
    HEXMAGE,
    "Sacrifice ~: Remove all counters from target creature.",
    false
)]
#[case::one_kind(
    ONE_KIND,
    "Sacrifice ~: Remove all counters from target permanent.",
    false
)]
fn all_counters_removed(#[case] json: &str, #[case] clause: &str, #[case] reads: bool) {
    assert_eq!(written(json, clause), reads, "{clause}");
}

fn archfiend(most: u32, who: &str) -> String {
    format!(
        r#"{{"Triggered":{{"effects":[{{"Choose":{{"as":"kept","count":{{"max":{most},"min":0}},"from":{{"Each":[{{"Type":"Creature"}},{{"ControlledBy":"{who}"}}]}},"rest_as":"rest","who":"{who}"}}}},{{"Sacrifice":{{"count":{{"Count":[{{"Is":{{"Bound":"rest"}}}}]}},"filter":[{{"Is":{{"Bound":"rest"}}}}],"who":"{who}"}}}}],"when":{{"StepBegins":{{"step":"End","whose":"Opponents"}}}}}}}}"#
    )
}

const ARCHFIEND: &str = "At the beginning of each opponent's end step, that player chooses up to two creatures they control, then sacrifices the rest.";

#[rstest]
#[case::two(2, "Triggering", true)]
#[case::three(3, "Triggering", false)]
#[case::you(2, "You", false)]
fn the_rest_is_sacrificed(#[case] most: u32, #[case] who: &str, #[case] reads: bool) {
    assert_eq!(written(&archfiend(most, who), ARCHFIEND), reads);
}

fn gamble(random: bool) -> String {
    format!(
        r#"{{"Spell":{{"effects":[{{"Search":{{"as":"card","filter":[]}}}},{{"Move":{{"to":"Hand","what":{{"Bound":"card"}}}}}},{{"Discard":{{"random":{random}}}}},{{"Shuffle":{{}}}}]}}}}"#
    )
}

#[rstest]
#[case::that_card(
    true,
    "Search your library for a card, put that card into your hand, discard a card at random, then shuffle.",
    true
)]
#[case::chosen(
    false,
    "Search your library for a card, put that card into your hand, discard a card at random, then shuffle.",
    false
)]
#[case::chosen_said(
    false,
    "Search your library for a card, put that card into your hand, discard a card, then shuffle.",
    true
)]
fn a_search_discards_before_the_shuffle(
    #[case] random: bool,
    #[case] clause: &str,
    #[case] reads: bool,
) {
    assert_eq!(written(&gamble(random), clause), reads, "{clause}");
}

fn guiltfeeder(zone: &str, owner: &str) -> String {
    format!(
        r#"{{"Triggered":{{"effects":[{{"LoseLife":{{"amount":{{"Count":[{{"In":"{zone}"}},{{"OwnedBy":"{owner}"}}]}},"who":"Defending"}}}}],"when":{{"Unblocked":{{"what":["This"]}}}}}}}}"#
    )
}

const GUILTFEEDER: &str = "Whenever ~ attacks and isn't blocked, defending player loses 1 life for each card in their graveyard.";

#[rstest]
#[case::graveyard("Graveyard", "Defending", true)]
#[case::hand("Hand", "Defending", false)]
#[case::yours("Graveyard", "You", false)]
fn life_lost_for_each_card(#[case] zone: &str, #[case] owner: &str, #[case] reads: bool) {
    assert_eq!(written(&guiltfeeder(zone, owner), GUILTFEEDER), reads);
}

fn parting(on: &str) -> String {
    format!(
        r#"{{"Spell":{{"effects":[{{"Destroy":{{"what":{{"Target":0}}}}}},{{"Draw":{{"count":{{"AnyCounters":{on}}},"who":"You"}}}},{{"LoseLife":{{"amount":{{"AnyCounters":{on}}},"who":"You"}}}}],"targets":[{{"what":{{"Object":[{{"Type":"Creature"}}]}}}}]}}}}"#
    )
}

const PARTING: &str = "Destroy target creature. You draw X cards and you lose X life, where X is the number of counters on that creature.";

#[rstest]
#[case::that_creature(r#"{"Target":0}"#, true)]
#[case::this(r#""This""#, false)]
fn x_is_the_counters_on_the_target(#[case] on: &str, #[case] reads: bool) {
    assert_eq!(written(&parting(on), PARTING), reads);
}

fn vault(counter: &str, on: &str) -> String {
    format!(
        r#"{{"Triggered":{{"effects":[{{"CreateToken":{{"count":{{"Counters":{{"counter":{counter},"on":"{on}"}}}},"token":{{"Custom":{{"colors":["Black"],"name":"Zombie Mutant","power":2,"subtypes":["Zombie","Mutant"],"toughness":2,"types":["Creature"]}}}}}}}}],"when":{{"Chapter":{{"number":2}}}}}}}}"#
    )
}

const VAULT: &str = "II — Create X 2/2 black Zombie Mutant creature tokens, where X is the total number of rad counters among players.";

#[rstest]
#[case::rad(r#"{"Other":"rad"}"#, "EachPlayer", true)]
#[case::poison(r#""poison""#, "EachPlayer", false)]
#[case::on_this(r#"{"Other":"rad"}"#, "This", false)]
fn x_is_the_counters_among_players(#[case] counter: &str, #[case] on: &str, #[case] reads: bool) {
    assert_eq!(written(&vault(counter, on), VAULT), reads);
}

fn paradox(counter: &str) -> String {
    format!(
        r#"{{"Triggered":{{"effects":[{{"MultiplyCounters":{{"counter":{{"Other":"growth"}},"on":"This"}}}},{{"CreateToken":{{"as":"token","token":{{"Custom":{{"colors":["Green","Blue"],"name":"Fractal","power":0,"subtypes":["Fractal"],"toughness":0,"types":["Creature"]}}}}}}}},{{"PutCounters":{{"count":{{"Counters":{{"counter":{{"Other":"growth"}},"on":"This"}}}},"counter":{counter},"on":{{"Bound":"token"}}}}}}],"when":{{"StepBegins":{{"step":"End","whose":"You"}}}}}}}}"#
    )
}

const PARADOX: &str = "At the beginning of your end step, double the number of growth counters on ~. Then create a 0/0 green and blue Fractal creature token. Put X +1/+1 counters on it, where X is the number of growth counters on ~.";

#[rstest]
#[case::plus(r#""+1/+1""#, true)]
#[case::minus(r#""-1/-1""#, false)]
fn a_token_gets_x_counters(#[case] counter: &str, #[case] reads: bool) {
    assert_eq!(written(&paradox(counter), PARADOX), reads);
}

fn soldiers(most: u32, subtype: &str, all_if: &str) -> String {
    format!(
        r#"{{"Triggered":{{"effects":[{{"Modal":{{{all_if}"modes":[{{"effects":[{{"CreateToken":{{"token":{{"Custom":{{"colors":["White"],"name":"Soldier","power":1,"subtypes":["Soldier"],"toughness":1,"types":["Creature"]}}}}}}}}]}},{{"effects":[{{"Choose":{{"as":"soldiers","count":{{"max":{most},"min":0}},"from":{{"Each":[{{"Subtype":"{subtype}"}},{{"ControlledBy":"You"}}]}}}}}},{{"PutCounters":{{"counter":"+1/+1","on":{{"Bound":"soldiers"}}}}}}]}}]}}}}],"when":{{"StepBegins":{{"step":"BeginCombat","whose":"You"}}}}}}}}"#
    )
}

const COMMANDER: &str = r#""all_if":{"Compare":{"left":{"Count":["Commander",{"ControlledBy":"You"}]},"op":"Ge","right":1}},"#;
const SOLDIERS: &str = "At the beginning of combat on your turn, choose one. If you control a commander, you may choose both instead.\n• Create a 1/1 white Soldier creature token.\n• Put a +1/+1 counter on each of up to two Soldiers you control.";

#[rstest]
#[case::printed(2, "Soldier", COMMANDER, true)]
#[case::three(3, "Soldier", COMMANDER, false)]
#[case::knights(2, "Knight", COMMANDER, false)]
#[case::no_condition(2, "Soldier", "", false)]
fn modes_with_a_condition_for_both(
    #[case] most: u32,
    #[case] subtype: &str,
    #[case] all_if: &str,
    #[case] reads: bool,
) {
    assert_eq!(written(&soldiers(most, subtype, all_if), SOLDIERS), reads);
}

fn charm(divide: u32, most: u32) -> String {
    format!(
        r#"{{"Spell":{{"effects":[{{"Modal":{{"modes":[{{"effects":[{{"Exile":{{"what":{{"Target":0}}}}}}],"targets":[{{"what":{{"Object":[{{"Type":"Creature"}},{{"Compare":{{"op":"Ge","stat":"Power","to":3}}}}]}}}}]}},{{"effects":[{{"Draw":{{"count":2}}}},{{"LoseLife":{{"amount":2}}}}]}},{{"effects":[{{"PutCounters":{{"count":"Divided","counter":"+1/+1","on":{{"Target":0}}}}}}],"targets":[{{"bounds":{{"max":{most},"min":1}},"divide":{divide},"what":{{"Object":[{{"Type":"Creature"}}]}}}}]}}]}}}}]}}}}"#
    )
}

const CHARM: &str = "Choose one —\n• Exile target creature with power 3 or greater.\n• You draw two cards and you lose 2 life.\n• Distribute two +1/+1 counters among one or two target creatures.";

#[rstest]
#[case::printed(2, 2, true)]
#[case::three_counters(3, 2, false)]
#[case::three_targets(2, 3, false)]
fn counters_distributed_in_a_mode(#[case] divide: u32, #[case] most: u32, #[case] reads: bool) {
    assert_eq!(written(&charm(divide, most), CHARM), reads);
}

fn coalition(effect: &str) -> String {
    format!(
        r#"{{"Spell":{{"effects":[{{"Modal":{{"choose":{{"max":5,"min":5}},"repeat":true,"modes":[{{"effects":[{{"{effect}":{{"what":{{"Target":0}}}}}}],"targets":[{{"what":{{"Object":[]}}}}]}},{{"effects":[{{"Draw":{{"who":{{"Target":0}}}}}}],"targets":[{{"what":"Player"}}]}}]}}}}]}}}}"#
    )
}

const COALITION: &str = "Choose five. You may choose the same mode more than once.\n• Target permanent phases out.\n• Target player draws a card.";

#[rstest]
#[case::phases("PhaseOut", true)]
#[case::destroyed("Destroy", false)]
fn a_target_phases_out_in_a_mode(#[case] effect: &str, #[case] reads: bool) {
    assert_eq!(written(&coalition(effect), COALITION), reads);
}

fn brawler(last: &str) -> String {
    format!(
        r#"{{"Activated":{{"cost":[{{"Mana":"{{5}}"}},"ExileSelf"],"effects":[{{"PutCounters":{{"count":4,"counter":"+1/+1","on":{{"Target":0}}}}}},{{"PutCounters":{{"counter":{{"Other":"flying"}},"on":{{"Target":0}}}}}},{{"PutCounters":{{"counter":{{"Other":"{last}"}},"on":{{"Target":0}}}}}}],"from":"Graveyard","targets":[{{"what":{{"Object":[{{"Type":"Creature"}}]}}}}],"timing":"Sorcery"}}}}"#
    )
}

const BRAWLER: &str = "{5}, Exile ~ from your graveyard: Choose target creature. Put four +1/+1 counters, a flying counter, and a lifelink counter on that creature. Activate only as a sorcery.";

#[rstest]
#[case::lifelink("lifelink", true)]
#[case::trample("trample", false)]
fn kinds_of_counters_listed_on_one_target(#[case] last: &str, #[case] reads: bool) {
    assert_eq!(written(&brawler(last), BRAWLER), reads);
}

fn vault_three(second: &str) -> String {
    format!(
        r#"{{"Triggered":{{"effects":[{{"PutCounters":{{"count":2,"counter":"+1/+1","on":{{"Each":[{{"Type":"Creature"}},{{"ControlledBy":"You"}},{{"AnyOf":[{{"Subtype":"Zombie"}},{{"Subtype":"{second}"}}]}}]}}}}}}],"when":{{"Chapter":{{"number":3}}}}}}}}"#
    )
}

const VAULT_THREE: &str =
    "III — Put two +1/+1 counters on each creature you control that's a Zombie or Mutant.";

#[rstest]
#[case::mutant("Mutant", true)]
#[case::human("Human", false)]
fn counters_on_a_choice_of_subtypes(#[case] second: &str, #[case] reads: bool) {
    assert_eq!(written(&vault_three(second), VAULT_THREE), reads);
}

fn trigger(when: &str, condition: &str, effects: &str, targets: &str) -> String {
    format!(
        r#"{{"Triggered":{{"effects":[{effects}],"if":{condition},"targets":[{targets}],"when":{when}}}}}"#
    )
}

const DIES: &str =
    r#"{"ZoneChange":{"from":"Battlefield","to":"Graveyard","what":[{"Type":"Creature"}]}}"#;
const UPKEEP: &str = r#"{"StepBegins":{"step":"Upkeep","whose":"You"}}"#;
const END: &str = r#"{"StepBegins":{"step":"End"}}"#;
const MINUS: &str = r#"{"PutCounters":{"counter":"-1/-1","on":{"Target":0}}}"#;
const CREATURE: &str = r#"{"what":{"Object":[{"Type":"Creature"}]}}"#;
const HAD_MINUS: &str =
    r#"{"Matches":{"selector":[{"HasCounter":"-1/-1"}],"subject":"Triggering"}}"#;
const HAD_PLUS: &str =
    r#"{"Matches":{"selector":[{"HasCounter":"+1/+1"}],"subject":"Triggering"}}"#;
const LUCK: &str = r#"{"Compare":{"left":{"Counters":{"counter":{"Other":"luck"},"on":"This"}},"op":"Ge","right":10}}"#;
const LOST: &str = r#"{"Compare":{"left":{"Tallied":{"what":"LifeLost","who":"EachPlayer"}},"op":"Ge","right":4}}"#;
const GAINED: &str = r#"{"Compare":{"left":{"Tallied":{"what":"LifeGained","who":"EachPlayer"}},"op":"Ge","right":4}}"#;

#[rstest]
#[case::had_a_counter(
    DIES,
    HAD_MINUS,
    MINUS,
    CREATURE,
    "Whenever a creature dies, if it had a -1/-1 counter on it, put a -1/-1 counter on target creature.",
    true
)]
#[case::had_another_counter(
    DIES,
    HAD_PLUS,
    MINUS,
    CREATURE,
    "Whenever a creature dies, if it had a -1/-1 counter on it, put a -1/-1 counter on target creature.",
    false
)]
#[case::luck(
    UPKEEP,
    LUCK,
    r#"{"WinGame":{}}"#,
    "",
    "At the beginning of your upkeep, if ~ has ten or more luck counters on it, you win the game.",
    true
)]
#[case::nine_luck(
    UPKEEP,
    LUCK,
    r#"{"WinGame":{}}"#,
    "",
    "At the beginning of your upkeep, if ~ has nine or more luck counters on it, you win the game.",
    false
)]
#[case::life_lost(
    END,
    LOST,
    r#"{"Draw":{}}"#,
    "",
    "At the beginning of each end step, if a player lost 4 or more life this turn, you draw a card.",
    true
)]
#[case::life_gained(
    END,
    GAINED,
    r#"{"Draw":{}}"#,
    "",
    "At the beginning of each end step, if a player lost 4 or more life this turn, you draw a card.",
    false
)]
fn an_intervening_condition_follows_the_event(
    #[case] when: &str,
    #[case] condition: &str,
    #[case] effects: &str,
    #[case] targets: &str,
    #[case] clause: &str,
    #[case] reads: bool,
) {
    assert_eq!(
        written(&trigger(when, condition, effects, targets), clause),
        reads,
        "{clause}"
    );
}

const NONTOKEN_DIES: &str = r#"{"ZoneChange":{"from":"Battlefield","to":"Graveyard","what":[{"Type":"Creature"},{"Not":"Token"},{"ControlledBy":"You"}]}}"#;
const INKLINGS: &str = r#"{"CreateToken":{"count":{"AnyCounters":"Triggering"},"tapped":true,"token":{"Custom":{"abilities":[{"Keyword":"Flying"}],"colors":["White","Black"],"name":"Inkling","power":2,"subtypes":["Inkling"],"toughness":1,"types":["Creature"]}}}}"#;
const FELISA: &str = "Whenever a nontoken creature you control dies, if it had counters on it, create X tapped 2/1 white and black Inkling creature tokens with flying, where X is the number of counters it had on it.";

#[rstest]
#[case::any_counter(
    r#"{"Matches":{"selector":["HasAnyCounter"],"subject":"Triggering"}}"#,
    true
)]
#[case::one_kind(HAD_PLUS, false)]
fn x_and_the_condition_are_the_counters_of_the_dead_creature(
    #[case] condition: &str,
    #[case] reads: bool,
) {
    assert_eq!(
        written(&trigger(NONTOKEN_DIES, condition, INKLINGS, ""), FELISA),
        reads
    );
}

const KALAMAX: &str = r#"{"Triggered":{"effects":[{"CopySpell":{"new_targets":true,"what":"Triggering"}}],"if":{"Matches":{"selector":["Tapped"],"subject":"This"}},"when":{"Nth":{"event":{"SpellCast":{"by":"You","what":[{"Type":"Instant"}]}},"nth":1}}}}"#;

#[rstest]
#[case::tapped(
    "Whenever you cast your first instant spell each turn, if ~ is tapped, copy that spell. You may choose new targets for the copy.",
    true
)]
#[case::untapped(
    "Whenever you cast your first instant spell each turn, if ~ is untapped, copy that spell. You may choose new targets for the copy.",
    false
)]
fn the_state_of_the_source_follows_the_event(#[case] clause: &str, #[case] reads: bool) {
    assert_eq!(written(KALAMAX, clause), reads);
}
