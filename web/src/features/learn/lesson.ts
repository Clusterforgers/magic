import type { TourStop } from '@/components/tour/TourCard'
import type { PlayState } from '@/play/store'
import type { CardType, ObjectId, Step } from '@/protocol/generated'
import { primaryLabel } from '@/features/table/prompt/passWords'
import { decisionName } from '@/features/table/turnText'
import type { DecisionName } from '@/features/table/turnText'

/** A card of the player's, by name and id. */
export interface Named {
  name: string
  id: ObjectId
}

/** What a lesson step can ask of the table: the facts of the game as this seat sees it. */
export interface LessonFacts {
  /** The turn number, 0 before the first view. */
  turn: number
  /** Whether it is the player's turn. */
  mine: boolean
  /** The step of the turn, null before the first view. */
  step: Step | null
  /** The kind of the decision waiting for the player's answer, null while none waits. */
  decision: DecisionName | 'Unknown' | null
  hand: readonly Named[]
  /** The player's lands and creatures on the battlefield. */
  lands: readonly Named[]
  creatures: readonly Named[]
  /** The bot's creatures on the battlefield. */
  theirCreatures: readonly Named[]
  /** The cards in the player's command zone. */
  command: readonly Named[]
  /** The names on the stack, top last, and whether the player controls each. */
  stack: readonly (Named & { mine: boolean })[]
  /** The player's creatures that attack, and those that block. */
  attacking: number
  blocking: number
  /** Whether a creature attacks the player. */
  attacked: boolean
  /** The bot's life, null before the first view. */
  theirLife: number | null
  /** Manual actions the player took, from the log. */
  byHand: number
  /** Whether the game is over, and whether the player won it. */
  over: boolean
  won: boolean
}

/** The facts of `state`, read for the lesson. */
export function lessonFacts(state: PlayState): LessonFacts {
  const view = state.view
  const seat = state.seat
  const empty: LessonFacts = {
    turn: 0,
    mine: false,
    step: null,
    decision: null,
    hand: [],
    lands: [],
    creatures: [],
    theirCreatures: [],
    command: [],
    stack: [],
    attacking: 0,
    blocking: 0,
    attacked: false,
    theirLife: null,
    byHand: 0,
    over: state.result !== null,
    won:
      seat !== null &&
      state.result !== null &&
      state.result !== 'Draw' &&
      'Winner' in state.result &&
      state.result.Winner === seat,
  }
  if (view === null || seat === null) return empty
  const table = view
  const named = (id: ObjectId): Named => ({ id, name: state.objectsById[id]?.name ?? '' })
  const own = view.objects.filter((object) => object.controller === seat)
  const onField = (type: CardType, mine: boolean): Named[] =>
    view.objects
      .filter(
        (object) =>
          object.zone === 'Battlefield' &&
          (object.controller === seat) === mine &&
          object.types.includes(type),
      )
      .map((object) => named(object.id))
  const inZone = (zone: 'Hand' | 'Command'): Named[] =>
    own
      .filter((object) => typeof object.zone === 'object' && zone in object.zone && object.owner === seat)
      .map((object) => named(object.id))
  const attackers = table.combat.attackers
  const ownIds = new Set(own.map((object) => object.id))
  const byHand = state.log.filter((entry) => {
    const event = entry.event
    if (!('Public' in event)) return false
    const inner = event.Public
    return typeof inner === 'object' && 'ManualAction' in inner && inner.ManualAction.player === seat
  }).length
  return {
    ...empty,
    turn: table.turn.number,
    mine: table.turn.active === seat,
    step: table.turn.step,
    decision: state.pending === null ? null : decisionName(state.pending.decision.kind),
    hand: inZone('Hand'),
    lands: onField('Land', true),
    creatures: onField('Creature', true),
    theirCreatures: onField('Creature', false),
    command: inZone('Command'),
    stack: (table.stack.cards ?? []).map((id) => ({
      ...named(id),
      mine: state.objectsById[id]?.controller === seat,
    })),
    attacking: attackers.filter((attacker) => ownIds.has(attacker.object)).length,
    blocking: attackers.reduce(
      (count, attacker) => count + attacker.blockers.filter((blocker) => ownIds.has(blocker)).length,
      0,
    ),
    attacked: attackers.some((attacker) => !ownIds.has(attacker.object)),
    theirLife: table.players.find((_, index) => index !== seat)?.life ?? null,
    byHand,
  }
}

/** Where a step's text names the primary button: its words as the dock draws them now. */
export const PASS = '{pass}'

/**
 * The text of `step` with the primary button's words in place of `PASS`: `shown`, the words the dock draws (short on a
 * narrow dock), else the whole words for `facts`.
 */
export function stepText(
  step: Pick<LessonStep, 'text'>,
  facts: LessonFacts,
  shown: string | null = null,
): string {
  if (!step.text.includes(PASS)) return step.text
  const top = facts.stack.at(-1)?.name ?? null
  const words =
    shown ?? (facts.step === null ? null : primaryLabel({ step: facts.step, yourTurn: facts.mine, top }))
  return step.text.replaceAll(PASS, words ?? 'End turn')
}

/** One step of a lesson: a tour stop that is done when the game shows the player did what it asks. */
export interface LessonStep extends Omit<TourStop, 'targets'> {
  /** Selectors of the part to light up, worked out from the facts (a card in hand by its id). */
  targets: (facts: LessonFacts) => readonly string[]
  /** Whether the player did what the step asks; a step without one is read and left with Next. */
  done?: (facts: LessonFacts) => boolean
  /** What the step waits for, said in place of Next. */
  waiting?: string
  /** The decisions of the player's the step is shown at; empty for a step the game plays by itself. */
  decisions?: readonly DecisionName[]
}

/** A lesson: its id and its steps in order. */
export interface Lesson {
  id: LessonId
  steps: readonly LessonStep[]
}

/** The lessons the tutorial table plays. */
export type LessonId = 'game' | 'manual'

/**
 * The step to show at `index` given `facts`: past every step already done, so a step the player finished before it
 * came up (or several at once, as one pass can) never waits. A read step stops the walk; past the last step is the
 * lesson's length.
 */
export function advance(steps: readonly LessonStep[], index: number, facts: LessonFacts): number {
  let at = index
  while (at < steps.length) {
    const done = steps[at]?.done
    if (done === undefined || !done(facts)) break
    at += 1
  }
  return at
}

const card = (cards: readonly Named[], name: string): string[] =>
  cards.filter((each) => each.name === name).map((each) => `[data-object-id="${each.id}"]`)

const HAND = '[data-region="hand"]'
const CONTROLS = '[data-region="controls"]'
const SHARED = '[data-region="shared"]'
const OWN = '[data-area="own"]'
const THEIRS = '[data-area="opponents"]'

/** The bot's starting life, as the server sets it (`tutorial::BOT_LIFE`). */
export const BOT_LIFE = 6

/** The steps of a turn in order, to tell whether the game is past a moment. */
const STEPS: readonly Step[] = [
  'Untap',
  'Upkeep',
  'Draw',
  'Main1',
  'BeginCombat',
  'DeclareAttackers',
  'DeclareBlockers',
  'FirstStrikeDamage',
  'CombatDamage',
  'EndCombat',
  'Main2',
  'End',
  'Cleanup',
]

/** Whether the game is past `step` of turn `turn`. */
export function past(facts: LessonFacts, turn: number, step: Step): boolean {
  if (facts.turn !== turn) return facts.turn > turn
  return facts.step !== null && STEPS.indexOf(facts.step) > STEPS.indexOf(step)
}

const has = (cards: readonly Named[], name: string): boolean => cards.some((each) => each.name === name)

/** Whether the player has finished casting `name`: it is on the stack or the battlefield, or out of the zone it came from. */
const cast = (facts: LessonFacts, name: string, from: readonly Named[]): boolean =>
  !has(from, name) || has(facts.stack, name) || has(facts.creatures, name)

/** Whether the player is past choosing `name` to cast: paying for it, or done. */
const chosen = (facts: LessonFacts, name: string, from: readonly Named[]): boolean =>
  facts.decision === 'PayMana' || facts.decision === 'ChooseTargets' || cast(facts, name, from)

const GIADA = 'Giada, Font of Hope'

/**
 * The guided game: the turns of the tutorial table, from the first land to the winning attack. Each step that waits on
 * the player is shown at the decisions it lists and names the button to press; `tutorialTrace.json` (written by the
 * server's tutorial test) holds every decision the table sends, and `lesson.test.ts` walks it.
 */
export const GAME_LESSON: Lesson = {
  id: 'game',
  steps: [
    {
      id: 'welcome',
      title: 'Your first game',
      text: 'A real game against a practice bot that plays the same way every time. Each step says what to do and which button to press, and moves on when the game does. Skip ends the lesson; the game stays yours.',
      targets: () => [],
    },
    {
      id: 'land',
      title: 'Play a land',
      text: 'Lands make the mana you pay spells with. You may play one land each turn: click the Forest in your hand.',
      targets: (facts) => [...card(facts.hand, 'Forest'), HAND],
      done: (facts) => facts.lands.length >= 3,
      waiting: 'Waiting for your land',
      decisions: ['Priority'],
    },
    {
      id: 'cast',
      title: 'Cast a creature',
      text: 'Grizzly Bears costs {1}{G}: one mana of any kind and one green, and your three Forests make three. Click Grizzly Bears in your hand.',
      targets: (facts) => [...card(facts.hand, 'Grizzly Bears'), HAND],
      done: (facts) => chosen(facts, 'Grizzly Bears', facts.hand),
      waiting: 'Waiting for Grizzly Bears',
      decisions: ['Priority'],
    },
    {
      id: 'pay',
      title: 'Pay the cost',
      text: 'Now pay {1}{G}: press Pay automatically in the middle bar, or click two Forests to tap them.',
      targets: () => [SHARED],
      done: (facts) => cast(facts, 'Grizzly Bears', facts.hand) && facts.decision !== 'PayMana',
      waiting: 'Waiting for the payment',
      decisions: ['PayMana'],
    },
    {
      id: 'pass',
      title: 'End your turn',
      text: 'The Bears resolved and the game moved on to your second main phase: new creatures cannot attack yet. Press {pass} at the bottom right to end your turn.',
      targets: () => [CONTROLS],
      done: (facts) => facts.turn >= 2,
      waiting: 'Waiting for you to pass',
      decisions: ['Priority'],
    },
    {
      id: 'their-turn',
      title: 'The bot’s turn',
      text: 'The middle bar says whose turn it is. The bot plays a Swamp and casts Dregscape Zombie; nothing asks you anything, and your next turn comes by itself.',
      targets: () => [SHARED],
      done: (facts) => past(facts, 2, 'Cleanup'),
      waiting: 'Waiting for the bot',
      decisions: [],
    },
    {
      id: 'second-land',
      title: 'Another land',
      text: 'You drew a Plains, your first white mana. Click it to play it.',
      targets: (facts) => [...card(facts.hand, 'Plains'), HAND],
      done: (facts) => facts.lands.length >= 4,
      waiting: 'Waiting for your land',
      decisions: ['Priority'],
    },
    {
      id: 'to-combat',
      title: 'Go to combat',
      text: 'Your Bears are no longer new, so they may attack. Press {pass} at the bottom right.',
      targets: () => [CONTROLS],
      done: (facts) => past(facts, 3, 'Main1'),
      waiting: 'Waiting for you to pass',
      decisions: ['Priority'],
    },
    {
      id: 'attack',
      title: 'Attack',
      text: 'Click Grizzly Bears to attack the bot, then press Attack in the middle bar.',
      targets: (facts) => [...card(facts.creatures, 'Grizzly Bears'), SHARED],
      done: (facts) => facts.attacking > 0 || past(facts, 3, 'DeclareAttackers'),
      waiting: 'Waiting for your attack',
      decisions: ['DeclareAttackers'],
    },
    {
      id: 'attacking',
      title: 'Let combat go on',
      text: 'Your Bears attack. Now you may cast instants before blocks, but you have none: press {pass}. The bot does not block, and the Bears deal 2 damage.',
      targets: () => [CONTROLS],
      done: (facts) => past(facts, 3, 'DeclareAttackers'),
      waiting: 'Waiting for you to pass',
      decisions: ['Priority'],
    },
    {
      id: 'hero',
      title: 'Cast Hero in Training',
      text: 'After combat you are in your second main phase, with all four lands untapped. Click Hero in Training {2}{W} in your hand.',
      targets: (facts) => [...card(facts.hand, 'Hero in Training'), HAND],
      done: (facts) => chosen(facts, 'Hero in Training', facts.hand),
      waiting: 'Waiting for Hero in Training',
      decisions: ['Priority'],
    },
    {
      id: 'hero-pay',
      title: 'Pay with the Plains',
      text: 'Press Pay automatically: it taps the Plains and two Forests, and one Forest stays open.',
      targets: () => [SHARED],
      done: (facts) => cast(facts, 'Hero in Training', facts.hand) && facts.decision !== 'PayMana',
      waiting: 'Waiting for the payment',
      decisions: ['PayMana'],
    },
    {
      id: 'resolve',
      title: 'The stack',
      text: 'A spell waits on the stack until both players pass. Press {pass} at the bottom right.',
      targets: () => [SHARED, CONTROLS],
      done: (facts) => has(facts.creatures, 'Hero in Training'),
      waiting: 'Waiting for you to pass',
      decisions: ['Priority'],
    },
    {
      id: 'trigger',
      title: 'A trigger on the stack',
      text: 'Hero in Training entered, so its ability triggers: “draw a card” waits on the stack too. Press {pass} again.',
      targets: () => [SHARED, CONTROLS],
      done: (facts) => has(facts.hand, 'Giant Growth') || facts.turn >= 4,
      waiting: 'Waiting for the trigger',
      decisions: ['Priority'],
    },
    {
      id: 'end-turn',
      title: 'Keep mana open',
      text: 'You drew Giant Growth, an instant: you may cast it on any turn, and your last Forest stays untapped for it. Press {pass}.',
      targets: () => [CONTROLS],
      done: (facts) => facts.turn >= 4,
      waiting: 'Waiting for you to pass',
      decisions: ['Priority'],
    },
    {
      id: 'attacked',
      title: 'The bot attacks',
      text: 'Dregscape Zombie attacks you. You could cast Giant Growth now, but wait for the blocks: press {pass}.',
      targets: () => [THEIRS, CONTROLS],
      done: (facts) => past(facts, 4, 'DeclareAttackers'),
      waiting: 'Waiting for you to pass',
      decisions: ['Priority'],
    },
    {
      id: 'block',
      title: 'Block',
      text: 'Your Bears are tapped from attacking. Click Hero in Training: one click blocks the only attacker, Dregscape Zombie. Then press Block in the middle bar.',
      targets: (facts) => [...card(facts.creatures, 'Hero in Training'), SHARED],
      done: (facts) => facts.blocking > 0 || past(facts, 4, 'DeclareBlockers'),
      waiting: 'Waiting for your block',
      decisions: ['DeclareBlockers'],
    },
    {
      id: 'instant',
      title: 'Respond with an instant',
      text: 'The Zombie would kill your 2/2 Hero. Before damage you get priority again: click Giant Growth in your hand.',
      targets: (facts) => [...card(facts.hand, 'Giant Growth'), HAND],
      done: (facts) => chosen(facts, 'Giant Growth', facts.hand) || facts.turn >= 5,
      waiting: 'Waiting for Giant Growth',
      decisions: ['Priority'],
    },
    {
      id: 'target',
      title: 'Choose the target',
      text: 'Giant Growth gives a creature +3/+3 until end of turn. Click Hero in Training, then press Confirm targets.',
      targets: (facts) => [...card(facts.creatures, 'Hero in Training'), SHARED],
      done: (facts) => facts.decision === 'PayMana' || !has(facts.hand, 'Giant Growth') || facts.turn >= 5,
      waiting: 'Waiting for the target',
      decisions: ['ChooseTargets'],
    },
    {
      id: 'growth-pay',
      title: 'Pay with the open Forest',
      text: 'Press Pay automatically: the Forest you kept open pays {G}.',
      targets: () => [SHARED],
      done: (facts) => !has(facts.hand, 'Giant Growth') || facts.turn >= 5,
      waiting: 'Waiting for the payment',
      decisions: ['PayMana'],
    },
    {
      id: 'fight',
      title: 'Combat damage',
      text: 'You have no mana left, so the game passes for you: Giant Growth resolves, your 5/5 Hero survives and the Zombie dies. Your turn comes by itself.',
      targets: () => [OWN],
      done: (facts) => past(facts, 4, 'Cleanup'),
      waiting: 'Waiting for the bot',
      decisions: [],
    },
    {
      id: 'third-land',
      title: 'Your land for the turn',
      text: 'You drew a Forest: click it to play it.',
      targets: (facts) => [...card(facts.hand, 'Forest'), HAND],
      done: (facts) => facts.lands.length >= 5,
      waiting: 'Waiting for your land',
      decisions: ['Priority'],
    },
    {
      id: 'commander',
      title: 'The command zone',
      text: 'Giada, Font of Hope is your commander. It waits in the command zone, beside your hand, and you may cast it from there like a card in hand: click Giada.',
      targets: (facts) => [
        ...card(facts.command, GIADA),
        '[data-slot="hand-command"]',
        `${OWN} [data-zone="command"]`,
      ],
      done: (facts) => chosen(facts, GIADA, facts.command),
      waiting: 'Waiting for Giada',
      decisions: ['Priority'],
    },
    {
      id: 'commander-pay',
      title: 'Pay for your commander',
      text: 'Press Pay automatically: the Plains pays {W}. Giada is new, so it stays home; once it resolves the game moves on to combat by itself.',
      targets: () => [SHARED],
      done: (facts) => cast(facts, GIADA, facts.command) && facts.decision !== 'PayMana',
      waiting: 'Waiting for the payment',
      decisions: ['PayMana'],
    },
    {
      id: 'win',
      title: 'Win the game',
      text: 'The bot has 4 life and no blockers. Click Grizzly Bears and Hero in Training, then press Attack.',
      targets: (facts) => [
        ...card(facts.creatures, 'Grizzly Bears'),
        ...card(facts.creatures, 'Hero in Training'),
        SHARED,
      ],
      done: (facts) => facts.attacking > 0 || facts.over,
      waiting: 'Waiting for your attack',
      decisions: ['DeclareAttackers'],
    },
    {
      id: 'finish',
      title: 'Deal the damage',
      text: 'Press {pass}. The bot has no creature to block with, and 4 damage ends the game.',
      targets: () => [CONTROLS],
      done: (facts) => facts.over,
      waiting: 'Waiting for you to pass',
      decisions: ['Priority'],
    },
    {
      id: 'won',
      title: 'You won',
      text: 'That is a whole game: lands, spells, combat, the stack and your commander. The table tour shows the rest of the table, and the Learn page has more tutorials.',
      // Under the result's buttons, so the card covers none of them.
      targets: () => ['[data-slot="review-open"]'],
    },
  ],
}

/** Playing a card by hand: the Commands panel and the card menu, on the tutorial table. */
export const MANUAL_LESSON: Lesson = {
  id: 'manual',
  steps: [
    {
      id: 'why',
      title: 'Playing by hand',
      text: 'Some cards are not automatic yet. For them you act by hand: move cards, tap, add counters or change life. Everything you do by hand is in the log for everyone to see.',
      targets: () => [],
    },
    {
      id: 'open',
      title: 'The Commands panel',
      text: 'Open the Commands tab of the side panel. It holds every manual command: drawing, life, tokens, counters and the picked card.',
      targets: () => ['[data-slot="commands-panel"]', '[data-area="side"]', '[data-slot="side-tabs"]'],
      done: () => document.querySelector('[data-slot="commands-panel"]') !== null,
      waiting: 'Waiting for the Commands tab',
    },
    {
      id: 'move',
      title: 'Move a card by hand',
      text: 'Pick a card in the panel’s Card group, then click Grizzly Bears in your hand and move it to the battlefield. The game does not check costs for what you do by hand.',
      targets: () => ['[data-slot="commands-panel"]', HAND],
      done: (facts) => facts.byHand >= 1,
      waiting: 'Waiting for a move by hand',
    },
    {
      id: 'menu',
      title: 'The card menu',
      text: 'A shortcut for one card: right-click it, long-press it on a touch screen, or focus it and press M. Tap your Forest from its menu.',
      targets: (facts) => [...card(facts.lands, 'Forest'), OWN],
      done: (facts) => facts.byHand >= 2,
      waiting: 'Waiting for an action from the menu',
    },
    {
      id: 'done',
      title: 'That is all',
      text: 'The log shows each action with “(by hand)”. A card marked By hand tells you which of its rules you play yourself.',
      targets: () => ['[aria-label="Game log"]'],
    },
  ],
}

/** The lesson called `id`. */
export function lessonOf(id: LessonId): Lesson {
  return id === 'manual' ? MANUAL_LESSON : GAME_LESSON
}
