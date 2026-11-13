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

The 5 s server deadline rules out deep full-game search per decision; the levels share one evaluation and one threat model, and add search:

- **Threat model** (`brain/threat.rs`): card traits read once from the rules (draw and token engines, tutors, counters, removal, wipes, burn, fogs, tricks, flash); a permanent's danger beyond its body (engines, combo pieces doubled when their partner is in play, commanders with the commander damage they dealt); a player's threat (board, engines, hand, mana, life), which picks whom attacks go at; the worth of a spell on the stack.
- **Evaluation** (`brain/eval.rs`): life, cards, permanents and mana per team, the threat model's engine values, and interaction in the seat's own hand counted at its hold value (4 plus 1.6 per card in the opponents' hands, up to five; wipes 1.4 of it, counters 1.0, removal 0.8, fogs 0.6, burn 0.5, tricks 0.35; each further piece half the one before). Spending a counter or removal therefore has to beat what keeping it is worth: the bomb, the engine, the combo piece, the wipe on the seat's board and lethal burn are answered; a mana dork, a bear or a shock at a high life total are not.
- **Interaction policy** (`brain/priority.rs`): a main-phase play that taps the seat below a held counterspell pays its hold value (a quarter of it for removal and tricks), unless the hand would overflow; instant-speed answers, flash creatures and other instants wait for the opponents' turn; colour choices name the colour of the opposing spell on the stack (protection).
- **Search** (Normal, Hard): plays, attacks, blocks and targets are simulated on engine copies with quick policies for every seat and scored by the shared evaluation. Responses settle the whole stack (counter wars), combat plays run to the end of the turn (pumps wear off before scoring). In rollouts a seat holding a counterspell counters an opposing spell that threatens more than the counter's hold value, so opponents keeping mana up are modelled as holding interaction when their redealt hands have it.
- **ISMCTS** (Very Hard): one tree over redealt worlds per decision with the heuristic answer first, the same evaluation at the cut-off, and the policy's tap-out costs charged to the root answers; the tree's favourite replaces the heuristic answer only when its mean reward is 0.05 higher.

## Decision-quality suite

`crates/mtg-testkit/src/brain/suite.rs` holds hand-made positions with their acceptable answers; `cargo run --release -p mtg-testkit --example bot_suite [level]` prints the score per level and per theme.

## Measured

Decision-quality suite (66 cases, untimed, seed 7; timed, the longest answer is 1.2 s):

| Level | Before | Now |
| --- | --- | --- |
| Easy | 38% | 39% |
| Normal | 76% | 97% |
| Hard | 71% | 97% |
| Very Hard | 73% | 92% |

Seeded games against the brains of main before this work (`bot_tournament <level> old-<level>`, the three duel decks, `DECKS=3`): Normal 48% in 1v1 (240 games) and 24% of 4-player pods (160 games, 25% even); Hard 52% in 1v1 (240) and 25% of pods (160); Very Hard 46% in 1v1 (24 games; 42% before the tree search kept its heuristic answer), pods not measured (a pod of four tree searches plays for hours on four cores). The duel decks are creature mirrors where the suite's skills (counters, engines, combo pieces, wipes) rarely decide a game, so the judgement gains do not show as wins there; the evaluation weights moved by half either way also stay at 50%.
