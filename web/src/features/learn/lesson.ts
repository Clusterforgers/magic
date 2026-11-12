import type { TourStop } from '@/components/tour/TourCard'
import type { PlayState } from '@/play/store'
import type { CardType, ObjectId, Step } from '@/protocol/generated'

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

/** One step of a lesson: a tour stop that is done when the game shows the player did what it asks. */
export interface LessonStep extends Omit<TourStop, 'targets'> {
  /** Selectors of the part to light up, worked out from the facts (a card in hand by its id). */
  targets: (facts: LessonFacts) => readonly string[]
  /** Whether the player did what the step asks; a step without one is read and left with Next. */
  done?: (facts: LessonFacts) => boolean
  /** What the step waits for, said in place of Next. */
  waiting?: string
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

/** The guided game: the turns of the tutorial table, from the first land to the winning attack. */
export const GAME_LESSON: Lesson = {
  id: 'game',
  steps: [
    {
      id: 'welcome',
      title: 'Your first game',
      text: 'A real game against a practice bot that plays the same way every time. Each step says what to do and moves on when you have done it. Skip ends the lesson; the game stays yours.',
      targets: () => [],
    },
    {
      id: 'land',
      title: 'Play a land',
      text: 'Lands make the mana you pay spells with. You may play one land each turn: click the Forest in your hand.',
      targets: (facts) => [...card(facts.hand, 'Forest'), HAND],
      done: (facts) => facts.lands.length >= 3,
      waiting: 'Waiting for your land',
    },
    {
      id: 'cast',
      title: 'Cast a creature',
      text: 'Grizzly Bears costs {1}{G}: one mana of any kind and one green. Click it, then pay: click two Forests to tap them, or press Pay automatically.',
      targets: (facts) => [...card(facts.hand, 'Grizzly Bears'), SHARED, HAND],
      done: (facts) => facts.creatures.some((each) => each.name === 'Grizzly Bears'),
      waiting: 'Waiting for Grizzly Bears',
    },
    {
      id: 'pass',
      title: 'Pass priority',
      text: 'With nothing more to do, press the main button at the bottom right to pass. Passing with an empty stack moves the game on; here it ends your turn.',
      targets: () => [CONTROLS],
      done: (facts) => facts.turn >= 2,
      waiting: 'Waiting for you to pass',
    },
    {
      id: 'their-turn',
      title: 'The bot’s turn',
      text: 'The middle bar says whose turn it is. The bot plays a Mountain and casts Goblin Piker; your next turn comes by itself.',
      targets: () => [SHARED],
      done: (facts) => facts.turn > 3 || (facts.turn === 3 && facts.mine),
      waiting: 'Waiting for the bot',
    },
    {
      id: 'second-land',
      title: 'Another land',
      text: 'You drew a Plains. Play it: Hero in Training costs three, and one Forest stays open for later.',
      targets: (facts) => [...card(facts.hand, 'Plains'), HAND],
      done: (facts) => facts.lands.length >= 4,
      waiting: 'Waiting for your land',
    },
    {
      id: 'attack',
      title: 'Attack',
      text: 'Press the main button to go to combat, click Grizzly Bears to attack the bot, then confirm the attack in the middle bar. Your Bears are no longer new, so they may attack.',
      // The middle bar first, so the card never lies over the button that confirms the attack.
      targets: () => [SHARED, CONTROLS],
      done: (facts) => facts.attacking > 0 || (facts.theirLife ?? BOT_LIFE) < BOT_LIFE,
      waiting: 'Waiting for your attack',
    },
    {
      id: 'hero',
      title: 'Cast Hero in Training',
      text: 'After combat you are in your second main phase. Cast Hero in Training {2}{W}: Pay automatically taps the Plains and two Forests.',
      targets: (facts) => [...card(facts.hand, 'Hero in Training'), HAND, CONTROLS],
      done: (facts) =>
        facts.stack.some((each) => each.mine && each.name === 'Hero in Training') ||
        facts.creatures.some((each) => each.name === 'Hero in Training'),
      waiting: 'Waiting for Hero in Training',
    },
    {
      id: 'trigger',
      title: 'A trigger on the stack',
      text: 'When Hero in Training enters, its ability triggers: “draw a card” waits on the stack in the middle bar. Pass priority to let it resolve.',
      targets: () => [SHARED, CONTROLS],
      done: (facts) => facts.turn >= 4 || facts.hand.some((each) => each.name === 'Giant Growth'),
      waiting: 'Waiting for the trigger',
    },
    {
      id: 'end-turn',
      title: 'Keep mana open',
      text: 'You drew Giant Growth, an instant: you may cast it on any turn. Your last Forest stays untapped for it. Pass to end your turn.',
      targets: () => [CONTROLS],
      done: (facts) => facts.turn >= 4,
      waiting: 'Waiting for you to pass',
    },
    {
      id: 'block',
      title: 'Block',
      text: 'The bot attacks with Goblin Piker. Your Bears are tapped from attacking, so click Hero in Training, then the Piker, to block it; then confirm the blocks in the middle bar.',
      targets: () => [SHARED, THEIRS],
      done: (facts) => facts.blocking > 0 || facts.turn >= 5,
      waiting: 'Waiting for your block',
    },
    {
      id: 'instant',
      title: 'Respond with an instant',
      text: 'Before damage you get priority again. Cast Giant Growth on Hero in Training: +3/+3 until end of turn, so it survives the fight.',
      targets: (facts) => [...card(facts.hand, 'Giant Growth'), HAND],
      done: (facts) => !facts.hand.some((each) => each.name === 'Giant Growth') || facts.turn >= 5,
      waiting: 'Waiting for Giant Growth',
    },
    {
      id: 'resolve',
      title: 'Let it resolve',
      text: 'Your spell waits on the stack until both players pass. Pass priority: Giant Growth resolves, then combat damage is dealt.',
      targets: () => [SHARED, CONTROLS],
      done: (facts) => facts.turn >= 5,
      waiting: 'Waiting for the turn to end',
    },
    {
      id: 'commander',
      title: 'The command zone',
      text: 'Isamaru, Hound of Konda is your commander. It waits in the command zone, beside your hand, and you may cast it from there like a card in hand. Play your land, then cast Isamaru.',
      targets: (facts) => [
        ...card(facts.command, 'Isamaru, Hound of Konda'),
        '[data-slot="hand-command"]',
        `${OWN} [data-zone="command"]`,
      ],
      done: (facts) => !facts.command.some((each) => each.name.startsWith('Isamaru')),
      waiting: 'Waiting for Isamaru',
    },
    {
      id: 'win',
      title: 'Win the game',
      text: 'The bot has 4 life and no blockers. Go to combat and attack with Grizzly Bears and Hero in Training.',
      targets: (facts) => [...card(facts.creatures, 'Grizzly Bears'), OWN, CONTROLS],
      done: (facts) => facts.over,
      waiting: 'Waiting for your attack',
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
