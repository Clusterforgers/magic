# Bots

How the bot brains (`crates/mtg-testkit/src/brain`) decide, what the design draws on, and how strong each level measures.

## What works for Magic and card-game AI

- **Forge** (open-source MTG engine, `forge-ai`): hand-written policy per effect (`*Ai` classes such as `CounterAi`, `DestroyAi`, `PumpAi`) gated by `AiPlayDecision`; card value from `ComputerUtilCard.evaluateCreature` (a base for a non-token, power and toughness weighted, keyword bonuses, minus drawbacks); `AiAttackController` sorts attackers and declares only attacks whose blocks it can survive, with an aggression level from the race (turns to kill each way); `AiBlockController` blocks to survive first, then good trades, then chumps only when lethal is coming. Counter decisions read AI-profile properties (`MIN_SPELL_CMC_TO_COUNTER`, `ALWAYS_COUNTER_REMOVAL_SPELLS`, `ALWAYS_COUNTER_OTHER_COUNTERSPELLS`, chances to hold combat tricks and reserve mana for the second main phase). Strength comes from many specific rules; weakness is that nothing looks ahead, so it is beaten by sequencing and bluffs.
- **Monte Carlo search on Magic**: Ward and Cowling, "Monte Carlo search applied to card selection in Magic: The Gathering" (IEEE CIG 2009) show rollouts over a rule-based player beat the rules alone for card choice; Cowling, Ward and Powley, "Ensemble determinization in Monte Carlo tree search for the imperfect information card game Magic: The Gathering" (IEEE TCIAIG 2012) show many shallow determinized trees beat one deep tree and that a rollout policy with domain knowledge matters more than iterations; Cowling, Powley and Whitehouse, "Information Set Monte Carlo Tree Search" (IEEE TCIAIG 2012) share one tree across determinizations and avoid strategy fusion.
- **Hearthstone and other CCGs**: Santos, Santos and Melo, "Monte Carlo tree search experiments in Hearthstone" (IEEE CIG 2017) and Zhang and Buro, "Improving Hearthstone AI by learning high-level rollout policies and bucketing chance node events" (CIG 2017) find the evaluation at the rollout cut-off and a sensible rollout policy dominate; Świechowski, Tajmajer and Janusz, "Improving Hearthstone AI by combining MCTS and supervised learning algorithms" (CIG 2018) learn the cut-off evaluation from game records; Kowalski and Miernik, "Evolutionary approach to collectible card game arena deckbuilding" and the Legends of Code and Magic contests (2018 to 2020) show linear feature evaluations tuned by self-play are strong under tight time limits.
- **Learned evaluation**: Silver et al., "A general reinforcement learning algorithm that masters chess, shogi, and Go through self-play" (Science 2018) learn value and policy by self-play; under a 5 s budget on a CPU without a network runtime, the useful part is a small linear evaluation over hand-picked features tuned by self-play (as in TD-Gammon, Tesauro 1995), kept deterministic.
- **Multiplayer search and politics**: Sturtevant and Korf, "On pruning techniques for multi-player games" (AAAI 2000) contrast max^n with the paranoid assumption; Schadd and Winands, "Best reply search for multiplayer games" (IEEE TCIAIG 2011) let only the strongest opponent reply. For Commander the practical rules are archenemy targeting (pressure the leader, not the weakest), not overextending into a single wipe, and avoiding kingmaking (do not cripple the player who is the only check on the leader).
- **Strong Commander players**: hold interaction with mana up and spend it at the end of an opponent's turn when unused; counter or remove by threat, not by whatever came first (engines, commanders, combo pieces and bombs over mana dorks); judge threat by board power, evasion, card-advantage engines, commander and commander damage, mana available, cards in hand and life; sequence so mana is used fully; play around open mana and known tricks; chump only to survive; save wipes for when the opponents' boards are worth more than their own.

## What transfers from chess engines

Chess engines are strong through search under strict time control, a tuned evaluation, opening and endgame knowledge, and statistically sound testing (Stockfish's fishtest: SPRT on paired games, SPSA tuning). Magic hides hands and libraries, shuffles, and seats up to six players, so each idea is taken in the form that survives determinization:

| Technique | In chess | Here |
| --- | --- | --- |
| Iterative deepening, anytime answer | deepen until the clock runs out, keep the last best move | the tree search is anytime: every iteration deepens the tree, the leader is always ready (`brain/deep.rs`) |
| Time management | soft and hard limits, more time when the best move is unstable | `brain/clock.rs`: a budget scaled by the decision's importance (lethal in reach, a big opposing spell on the stack, the seat's big plays), a soft limit that stops once the leader is stable, a hard limit within 4.5 s of the question |
| Alpha-beta, minimax | prune lines the opponent refutes | rejected: hidden cards and shuffles make exact minimax over redealt worlds strategy-fused and slow; the opponent's replies are the policy's in each world |
| Late move reductions | search unlikely moves shallower | root answers whose mean reward is clearly below the best (two standard errors and 0.03) get no further iterations |
| Move ordering, killer and history heuristics | try good moves first | untried answers are tried in the order of their mean advantage in earlier searches this game (`deep::History`), the heuristic answer first |
| Transposition table | merge positions reached by different move orders | root answers that do the same thing (two copies of one card) are merged; positions in redealt worlds rarely repeat, so no table across iterations |
| Quiescence search | never evaluate mid-capture | a search never stops with a spell on the stack or inside combat (`sim::reached`); counter wars settle first |
| Mate search, tablebases | exact search near the end | `brain/endgame.rs`: with an opponent in reach, each answer is played to the end of the turn in three redealt worlds; one that wins in all is played at once |
| Evaluation tuning (Texel) | fit weights to game results | `examples/eval_tune.rs` fits the evaluation's linear terms (`eval::Terms`) to results of self-play positions |
| Parameter tuning (SPSA) | tune search and evaluation parameters by match play | `examples/spsa_tune.rs` tunes the policy constants (`brain/knobs.rs`) and evaluation weights by paired games between perturbed brains |
| SPRT match testing | paired games until significant | `examples/bot_tournament.rs` with `SPRT=elo0,elo1`, Elo with a 95% interval, the longest answer |
| Opening book | prepared openings | mulligan and land rules (`choices::keep`), ramp first (`knobs.ramp`) |

## Approach

The 5 s server deadline rules out deep full-game search per decision; the levels share one evaluation and one threat model, and add search. Every decision ends within 4.5 s of being asked (`clock::DECISION_CAP`): the tree search's hard limit, a clock check every 16 steps inside each simulation and rollout, and a 0.4 s cap on the endgame probe.

- **Threat model** (`brain/threat.rs`): card traits read once from the rules (draw and token engines, tutors, counters, removal, wipes, burn, fogs, tricks, flash); a permanent's danger beyond its body (engines, combo pieces doubled when their partner is in play, commanders with the commander damage they dealt); a player's threat (board, engines, hand, mana, life), which picks whom attacks go at; the worth of a spell on the stack.
- **Evaluation** (`brain/eval.rs`): life, cards, permanents and mana per team, the threat model's engine values, and interaction in the seat's own hand counted at its hold value (4 plus 1.6 per card in the opponents' hands, up to five; wipes 1.4 of it, counters 1.0, removal 0.8, fogs 0.6, burn 0.5, tricks 0.35; each further piece half the one before). Spending a counter or removal therefore has to beat what keeping it is worth: the bomb, the engine, the combo piece, the wipe on the seat's board and lethal burn are answered; a mana dork, a bear or a shock at a high life total are not.
- **Interaction policy** (`brain/priority.rs`): a main-phase play that taps the seat below a held counterspell pays its hold value (a quarter of it for removal and tricks), unless the hand would overflow; instant-speed answers, flash creatures and other instants wait for the opponents' turn; colour choices name the colour of the opposing spell on the stack (protection).
- **Opening book** (`brain/choices.rs`): seven-card hands keep three to five lands, or two with a spell of mana value two or less; smaller hands keep two to their size less two.
- **Search** (Normal, Hard): plays, attacks, blocks and targets are simulated on engine copies with quick policies for every seat and scored by the shared evaluation. Responses settle the whole stack (counter wars), combat plays run to the end of the turn (pumps wear off before scoring). In rollouts a seat holding a counterspell counters an opposing spell that threatens more than the counter's hold value, so opponents keeping mana up are modelled as holding interaction when their redealt hands have it.
- **Endgame** (Hard, Very Hard; `brain/endgame.rs`): with an opponent within reach of the seat's creatures and burn, each answer worth trying is played to the end of the turn in three redealt worlds, and one that wins in all of them is played at once.
- **ISMCTS** (Hard with `Deep::LIGHT`, Very Hard with `Deep::FULL`): one tree over redealt worlds per decision with the heuristic answer first, the same evaluation at the cut-off, and the policy's tap-out costs charged to the root answers. Iterations cycle through 32 fixed worlds (common random numbers, so answers are compared in the same deals); an opponent's blocks, and an opponent's counterspell reply to the seat's spell when its redealt hand holds one, are minimizing nodes of the tree (the paranoid reply of max^n: opponents are assumed to block well and counter with open mana); a node belongs to the seat that first chose there, and another seat's choice passes it by; at a table of more than two a rollout plays once round the table, so every crack back is seen. Rollouts judge each seat's main-phase casts (`Search::judged`): each of the four costliest castable spells is cast on a copy, the stack settled with the opponents' counters, and the best that beats keeping the cards is played, for the seat and its opponents alike. Root answers that do the same thing are merged, answers clearly below the best are pruned, untried answers come in the history's order, and the search budget follows the decision's importance (`brain/clock.rs`: up to three times the level's budget with lethal in reach, 2.5 times over an opposing spell worth answering, 1.5 times for combat and big spells, 0.5 to 0.6 times for small plays and end steps). The search has authority: its favourite replaces the heuristic answer when its mean reward is one standard error of the difference higher (`Deep::trust` 0, `Deep::doubt` 1), which it does in about a fifth of its searches. Where rollouts see least the margin grows by `Deep::guard` (0.05): spending interaction the heuristics keep (a counter, a fog, removal in a response, combat or end step) and attacks at a table of more than two. An attack that dies to the crack back is vetoed when the heuristic's does not, and the ramp bonus (`priority::ramp_bonus`) is charged to the root answers with the tap-out costs.
- **Tuning** (`brain/knobs.rs`, `eval::Terms`): the policy constants and evaluation weights are parameters; Texel tuning (`examples/eval_tune.rs`) and SPSA (`examples/spsa_tune.rs`) fit them, and a fit is kept only when it wins matches without losing suite cases.

## Decision-quality suite

`crates/mtg-testkit/src/brain/suite.rs` holds hand-made positions with their acceptable answers; `cargo run --release -p mtg-testkit --example bot_suite [level]` prints the score per level and per theme.

## Testing

- **Match testing** (`examples/bot_tournament.rs`): each deal is played twice with the sides swapped, so identical brains score exactly 50%; `DECKS=interaction` plays every pairing of an interaction deck (`Duel::INTERACTION`: Izzet, Dimir, Azorius) with every duel deck; `SPRT=elo0,elo1` stops at 5% error rates; the summary gives the Elo difference with its 95% interval and the longest answer. `old-<level>` plays main's brains, frozen in `examples/legacy_brain`.
- **Benchmark**: the decision-quality suite (`bot_suite`), as a fixed position set.
- **Pods**: mode `pod` seats four players, two of each side alternating, so equal brains score 50%; `cpod` deals random Commander decks (`commander_cards::commander_fuzz_setup`).
- **Overrides**: the tournament reports the tree searches, how many replaced the heuristic answer (`deep::tally`) and the score of the games with an override.
- **Clock**: `tests/game/brain_clock.rs` plays timed Hard and Very Hard games on the interaction decks and asserts every answer ends within the 4.5 s cap (`clock::DECISION_CAP`); a search keeps 0.3 s of it in hand.

## Measured

Decision-quality suite (66 cases, untimed, seed 7):

| Level | main | Now |
| --- | --- | --- |
| Easy | 39% | 39% |
| Normal | 97% | 97% |
| Hard | 97% | 97% |
| Very Hard | 92% | 97% |

Seeded games against main's brains of the same level on the interaction pairings, untimed, each deal played with the sides swapped (Elo with its 95% interval; SPRT with elo0 0, elo1 40 at 5% errors):

| Level | Mode | Games | Score | Elo | SPRT |
| --- | --- | --- | --- | --- | --- |
| Normal | 1v1 | 960 | 50.1% | +1 [-21, +23] | |
| Hard | 1v1 | 232 | 59.1% | +64 [+19, +110] | H1 accepted |
| Very Hard | 1v1 | 121 | 49.6% | -3 [-66, +60] | undecided |
| Hard | pod | 82 | 56.1% | +43 [-32, +122] | undecided |

A pod seats four players, two of each side. The build without the guard and the crack-back veto scored 58.4% over 219 games for Hard and 54.0% over 150 (+28 [-28, +85]) for Very Hard in 1v1, and 54.5% over 99 games for Hard in pods.

Search authority on Hard, 256 games each against main's Hard on the same deals: the former margin (0.05 plus a standard error) overrides 4.7% of the searches and scores 51.6%; a standard error alone overrides 19% and scores 53.9%; with the importance budget and counter replies in the tree 55.9%; most of the deals (75 to 80%) are split, each side winning from the same seat. Judged rollouts score the same as cast-the-costliest (53.9% both) at the same cost and are kept as the opponent model. 400 iterations for Hard instead of 150 scored 51% over 82 games at three times the cost.

Normal with Hard's former searches scores 59.6% against main's Normal (480 games, +67 Elo) but aims a Pacifism at its own team in Two-Headed Giant, so Normal keeps main's settings.

The longest timed answer stays under the 4.5 s cap, which the clock test asserts (`brain_clock`); the suite's longest Very Hard answer is 3.8 s.

Tuning: `eval_tune` with `DECKS=interaction` fits the evaluation's terms on 21,218 positions from 600 Normal self-play games and lowers the held-out loss from 0.229 to 0.188 at K = 60, with weights far from the hand-set ones (lethal_for 39, early_source 37, lethal_against 36, card 27); they wait for a match test, so the hand-set weights stay. `PODS=<n>` adds Commander pods; a pod of seed 51405 loops a choice question at turn 44. Earlier, Texel fitting of the evaluation's terms on 47,000 positions from 1,000 Hard games lowers the held-out loss from 0.231 to 0.184 but plays even (50.4% in 240 games); SPSA over the 32 policy constants and weights (430 rounds of 16 games) gains +4 Elo on the interaction pairings and +17 on all pairings, within noise, and loses five suite cases, so the hand-set values stay. The suite's judgement (counters held for threats, blocks against the crack back) shows little in bot-against-bot win rates: both sides share the same rollout policy, so most games turn on the same plays.
