//! Goal: the r30-4b wordings (a rule of the attached permanent after its bonus, three rules of it in one sentence, the
//! counters a card enters or escapes with, Animate Dead's trigger entering tapped, the upkeep payment that untaps the
//! enchanted creature, the flip of a player's permanents, skipped untap steps and Forbidden Crypt's replacements) write
//! back as the cards print them; another quirk, count, zone, cost or player never does.
use super::*;

/// Whether one wording of the abilities is the text.
fn says(written: &[String], text: &str) -> bool {
    written.iter().any(|one| same_text(one, text))
}

const BONUS: &str =
    r#"{"Static":{"affected":"Attached","mods":[{"ModifyPT":{"power":3,"toughness":0}}]}}"#;

#[rstest]
#[case::attacks_alone(
    r#"{"Rule":{"rule":{"Combat":{"quirk":"AttackAlone","who":"Attached"}}}}"#,
    "Enchanted creature gets +3/+0 and can only attack alone.",
    true
)]
#[case::cant_become_suspected(
    r#"{"Rule":{"rule":{"Cant":{"action":"BecomeSuspected","who":"Attached"}}}}"#,
    "Enchanted creature gets +3/+0 and can't become suspected.",
    true
)]
#[case::another_quirk(
    r#"{"Rule":{"rule":{"Combat":{"quirk":"AttackAsThoughHaste","who":"Attached"}}}}"#,
    "Enchanted creature gets +3/+0 and can only attack alone.",
    false
)]
#[case::suspected_this(
    r#"{"Rule":{"rule":{"Cant":{"action":"BecomeSuspected","who":"This"}}}}"#,
    "Enchanted creature gets +3/+0 and can't become suspected.",
    false
)]
fn a_rule_follows_the_bonus(#[case] rule: &str, #[case] text: &str, #[case] reads: bool) {
    let written = static_pair_texts(&ability(BONUS), &ability(rule));
    assert_eq!(says(&written, text), reads, "{written:?}");
}

const RESTRICTED: &str = r#"{"Static":{"affected":"Attached","mods":["CantAttack","CantBlock"]}}"#;
const NO_CREW: &str = r#"{"Rule":{"rule":{"Cant":{"who":"Attached","action":"Crew"}}}}"#;
const FETTERS: &str =
    r#"{"Rule":{"rule":{"Cant":{"action":{"Activate":[]},"who":{"Each":[{"Is":"Attached"}]}}}}}"#;
const GOLD: &str = "Enchanted permanent can't attack, block, or crew Vehicles, and its activated abilities can't be activated unless they're mana abilities.";

#[rstest]
#[case::bound_in_gold(RESTRICTED, NO_CREW, FETTERS, true)]
#[case::no_third(RESTRICTED, NO_CREW, NO_CREW, false)]
#[case::no_crew(RESTRICTED, FETTERS, FETTERS, false)]
fn three_rules_of_the_attached_permanent(
    #[case] first: &str,
    #[case] second: &str,
    #[case] third: &str,
    #[case] reads: bool,
) {
    let written = r304b_triple_texts(&ability(first), &ability(second), &ability(third));
    assert_eq!(says(&written, GOLD), reads, "{written:?}");
}

fn entering(count: u32, condition: &str) -> String {
    format!(
        r#"{{"Replacement":{{"when":{{"Enters":{{"what":["This"]}}}},"by":{{"WithCounters":{{"counter":"+1/+1","count":{count}}}}},"condition":{condition}}}}}"#
    )
}

const ESCAPED: &str = r#"{"CastWith":"Escape"}"#;
const NOT_ESCAPED: &str = r#"{"Not":{"CastWith":"Escape"}}"#;
const POLUKRANOS: &str =
    "~ enters with six +1/+1 counters on it. It escapes with twelve +1/+1 counters on it instead.";

#[rstest]
#[case::polukranos(entering(6, NOT_ESCAPED), entering(12, ESCAPED), POLUKRANOS, true)]
#[case::other_counts(entering(6, NOT_ESCAPED), entering(10, ESCAPED), POLUKRANOS, false)]
#[case::swapped(entering(12, ESCAPED), entering(6, NOT_ESCAPED), POLUKRANOS, false)]
#[case::both_plain(entering(6, NOT_ESCAPED), entering(12, NOT_ESCAPED), POLUKRANOS, false)]
fn counters_it_enters_or_escapes_with(
    #[case] first: String,
    #[case] second: String,
    #[case] text: &str,
    #[case] reads: bool,
) {
    let written = static_pair_texts(&ability(&first), &ability(&second));
    assert_eq!(says(&written, text), reads, "{written:?}");
}

const DANCE: &str = "When ~ enters, if it's on the battlefield, it loses \"enchant creature card in a graveyard\" and gains \"enchant creature put onto the battlefield with ~.\" Put enchanted creature card onto the battlefield tapped under your control and attach ~ to it. When ~ leaves the battlefield, that creature's controller sacrifices it.";

#[test]
fn animate_dead_entering_tapped_reads_and_writes_back() {
    let read = clause(DANCE, ClauseKind::Triggered, false).expect("read");
    assert!(says(&written_variants(&read), DANCE));
    let untapped = DANCE.replace("onto the battlefield tapped", "onto the battlefield");
    assert!(
        clause(&untapped, ClauseKind::Triggered, false)
            .is_none_or(|other| !says(&written_variants(&other), DANCE))
    );
}

fn upkeep_pay(whose: &str, who: &str, paid: &str) -> String {
    format!(
        r#"{{"Triggered":{{"when":{{"StepBegins":{{"step":"Upkeep","whose":{whose}}}}},"effects":[{{"Pay":{{"who":{who},"cost":"{{1}}{{B}}","paid":{paid}}}}}]}}}}"#
    )
}

const ITS_CONTROLLER: &str = r#"{"ControllerOf":"Attached"}"#;
const UNTAP_IT: &str = r#"[{"Untap":{"what":"Attached"}}]"#;
const PAY_TEXT: &str = "At the beginning of the upkeep of enchanted creature's controller, that player may pay {1}{B}. If the player does, untap that creature.";

#[rstest]
#[case::dance(ITS_CONTROLLER, ITS_CONTROLLER, UNTAP_IT, true)]
#[case::you_pay(ITS_CONTROLLER, r#""You""#, UNTAP_IT, false)]
#[case::your_upkeep(r#""You""#, ITS_CONTROLLER, UNTAP_IT, false)]
#[case::tap_instead(
    ITS_CONTROLLER,
    ITS_CONTROLLER,
    r#"[{"Tap":{"what":"Attached"}}]"#,
    false
)]
fn the_controller_pays_to_untap(
    #[case] whose: &str,
    #[case] who: &str,
    #[case] paid: &str,
    #[case] reads: bool,
) {
    let written = written_variants(&ability(&upkeep_pay(whose, who, paid)));
    assert_eq!(says(&written, PAY_TEXT), reads, "{written:?}");
}

const SANDS_TEXT: &str = "At the beginning of each player's upkeep, that player simultaneously untaps each tapped artifact, creature, and land they control and taps each untapped artifact, creature, and land they control.";

fn sands(tapped: &str) -> String {
    let kinds = r#"{"AnyOf":[{"Type":"Artifact"},{"Type":"Creature"},{"Type":"Land"}]}"#;
    format!(
        r#"{{"Triggered":{{"when":{{"StepBegins":{{"step":"Upkeep"}}}},"effects":[{{"Bind":{{"what":{{"Each":[{kinds},{{"ControlledBy":"Triggering"}},"Untapped"]}},"as":"untapped"}}}},{{"Untap":{{"what":{{"Each":[{kinds},{{"ControlledBy":"Triggering"}},"{tapped}"]}}}}}},{{"Tap":{{"what":{{"Bound":"untapped"}}}}}}]}}}}"#
    )
}

#[rstest]
#[case::sands_of_time("Tapped", true)]
#[case::untaps_the_untapped("Untapped", false)]
fn a_players_permanents_flip(#[case] tapped: &str, #[case] reads: bool) {
    let written = written_variants(&ability(&sands(tapped)));
    assert_eq!(says(&written, SANDS_TEXT), reads, "{written:?}");
}

#[rstest]
#[case::each_player(r#""EachPlayer""#, true)]
#[case::opponents(r#""Opponents""#, false)]
fn untap_steps_are_skipped(#[case] who: &str, #[case] reads: bool) {
    let json = format!(
        r#"{{"Rule":{{"rule":{{"Cant":{{"who":{who},"action":{{"SkipStep":"Untap"}}}}}}}}}}"#
    );
    let written = written_variants(&ability(&json));
    assert_eq!(
        says(&written, "Each player skips their untap step."),
        reads,
        "{written:?}"
    );
}

const CRYPT_DRAW: &str = r#"{"Replacement":{"when":{"Draw":{}},"by":{"Instead":[{"If":{"condition":{"Compare":{"left":{"Count":[{"In":"Graveyard"},{"OwnedBy":"You"}]},"op":"Ge","right":1}},"then":[{"Choose":{"from":{"Each":[{"In":"Graveyard"},{"OwnedBy":"You"}]},"as":"card"}},{"Move":{"what":{"Bound":"card"},"to":"Hand"}}],"otherwise":[{"LoseGame":{}}]}}]}}}"#;
const CRYPT_DRAW_TEXT: &str = "If you would draw a card, return a card from your graveyard to your hand instead. If you can't, you lose the game.";
const CRYPT_EXILE_TEXT: &str =
    "If a card would be put into your graveyard from anywhere, exile that card instead.";

#[rstest]
#[case::crypt_draw(CRYPT_DRAW, CRYPT_DRAW_TEXT, true)]
#[case::draw_to_graveyard(&CRYPT_DRAW.replace(r#""to":"Hand""#, r#""to":"Graveyard""#), CRYPT_DRAW_TEXT, false)]
#[case::crypt_exile(
    r#"{"Replacement":{"when":{"ZoneChange":{"what":[{"OwnedBy":"You"},{"Not":"Token"}],"to":"Graveyard"}},"by":{"MoveTo":"Exile"}}}"#,
    CRYPT_EXILE_TEXT,
    true
)]
#[case::any_graveyard(
    r#"{"Replacement":{"when":{"ZoneChange":{"what":[{"Not":"Token"}],"to":"Graveyard"}},"by":{"MoveTo":"Exile"}}}"#,
    CRYPT_EXILE_TEXT,
    false
)]
fn forbidden_crypt_replacements(#[case] json: &str, #[case] text: &str, #[case] reads: bool) {
    let written = written_variants(&ability(json));
    assert_eq!(says(&written, text), reads, "{written:?}");
}
