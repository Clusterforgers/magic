/** Pure: the commands of the Commands panel, what each one says it does, the preview of its change and the action it sends. */
import type {
  Color,
  CounterKind,
  ManaType,
  ManualAction,
  ObjectId,
  PlayerId,
  TokenOfferView,
} from '@/protocol/generated'
import { counterKind, heldOf, LIMITS, MANA_KINDS } from '../offers'
import { START_TOKEN, tokenPreview } from './customToken'
import type { TokenDraft } from './customToken'
import { rollPreview, SIDES } from './roll'
import type { RollPick } from './roll'

/** The groups of the panel, in order. */
export type PanelGroupId = 'you' | 'library' | 'make'

/** The commands of the panel. */
export type CommandId =
  | 'life'
  | 'poison'
  | 'energy'
  | 'experience'
  | 'rad'
  | 'draw'
  | 'look'
  | 'tutor'
  | 'mill'
  | 'exileTop'
  | 'bottomTop'
  | 'shuffle'
  | 'revealTop'
  | 'token'
  | 'mana'
  | 'roll'

/** How a command takes its amount: a signed change of a number the seat has, or a count of 1 or more. */
export interface AmountSpec {
  signed: boolean
  min: number
  max: number
  /** The quick amounts beside the field; a signed one adds to the pending change, a count sets it. */
  presets: readonly number[]
  /** The amount a row opens with. */
  start: number
}

/** One command of the panel. */
export interface Command {
  id: CommandId
  group: PanelGroupId
  label: string
  /** One line of what the command does, in plain words. */
  effect: string
  /** Left out for a command that takes no amount. */
  amount?: AmountSpec
  /** The choice it also takes: a token (or a custom creature), a kind of mana, or what to roll. */
  choice?: 'token' | 'mana' | 'roll'
  /** Why it cannot be used now; the row is listed, inert. */
  unavailable?: string
}

/** A titled run of commands. */
export interface PanelGroup {
  id: PanelGroupId
  label: string
  commands: Command[]
}

/** A seat of the table and the numbers the panel changes on it. */
export interface PanelSeat {
  seat: PlayerId
  /** The seat's name, as the table shows it. */
  name: string
  life: number
  library: number
  counters: readonly (readonly [CounterKind, number])[]
}

/** What the panel is made from: the seat and the numbers its commands change. */
export interface PanelInput {
  seat: PlayerId
  /** The seat's name, as the table shows it. */
  name: string
  life: number
  library: number
  hand: number
  counters: readonly (readonly [CounterKind, number])[]
  /** The seats still in the game, the seat itself first, to pick whose numbers the You group and Mill change. */
  seats?: readonly PanelSeat[]
  /** The other seat picked for those; left out while they change the seat itself. */
  subject?: PanelSeat
  /** The top cards the seat looked at and has not put back yet, top first; none otherwise. */
  looked: readonly ObjectId[]
  /** The whole library, top first, while the seat searches it by hand; none otherwise. */
  searched?: readonly ObjectId[]
  /** The colours of the seat's commander identity in WUBRG order (empty: colourless); left out outside Commander. */
  identity?: readonly Color[]
  /** The tokens the seat's cards make (`TableView.tokens`); none when left out. */
  tokens?: readonly TokenOfferView[]
  /** Why nothing can be done by hand (a seat that lost, a finished game); left out while the seat plays. */
  off?: string
}

/** What a row has chosen so far. */
export interface PanelDraft extends TokenDraft {
  amount: number
  mana: ManaType
  /** What the Roll row rolls and, for a die of any size, its faces. */
  roll: RollPick
  sides: number
}

/** What a row shows before Apply: the preview line and the words of the button, or why it cannot be applied. */
export type Preview = { preview: string; apply: string; action: ManualAction } | { problem: string }

const LIFE_PRESETS = [-5, -1, 1, 5] as const
const COUNTER_PRESETS = [-1, 1] as const
const CARD_PRESETS = [1, 2, 3] as const

const signed = (max: number, presets: readonly number[]): AmountSpec => ({
  signed: true,
  min: -max,
  max,
  presets,
  start: 0,
})
const count = (max: number, presets: readonly number[] = CARD_PRESETS): AmountSpec => ({
  signed: false,
  min: 1,
  max,
  presets,
  start: 1,
})

/** The seat of the panel and its numbers. */
function selfOf(input: PanelInput): PanelSeat {
  return {
    seat: input.seat,
    name: input.name,
    life: input.life,
    library: input.library,
    counters: input.counters,
  }
}

/** The seat whose life, counters and library top the commands change: the picked seat, else the seat itself. */
export function subjectOf(input: PanelInput): PanelSeat {
  return input.subject ?? selfOf(input)
}

const capital = (text: string): string => text.charAt(0).toUpperCase() + text.slice(1)

/** The player counters the panel lists by name, in order, with what each does to `on` ("you" or a seat's name). */
const PLAYER_COUNTERS = [
  [
    'poison',
    'Poison',
    (on: string) => `Poison counters on ${on}; ten of them and ${on === 'you' ? 'you lose' : `${on} loses`}.`,
  ],
  ['energy', 'Energy', (on: string) => `Energy counters on ${on}, to pay energy costs.`],
  ['experience', 'Experience', (on: string) => `Experience counters on ${on}.`],
  [
    'rad',
    'Rad',
    (on: string) =>
      on === 'you'
        ? 'Rad counters on you; each upkeep you mill that many and lose 1 life and a rad per nonland card milled.'
        : `Rad counters on ${on}; each upkeep they mill that many and lose 1 life and a rad per nonland card milled.`,
  ],
] as const

/** "your" for the seat itself, "Bo's" for another seat. */
const possessive = (input: PanelInput, who: PanelSeat): string =>
  who.seat === input.seat ? 'your' : `${who.name}’s`

/** The groups and commands of the panel for `input`. */
export function panelGroups(input: PanelInput): PanelGroup[] {
  const off = input.off
  const who = subjectOf(input)
  const on = who.seat === input.seat ? 'you' : who.name
  const noCards = off ?? (input.library === 0 ? 'Your library is empty.' : undefined)
  const noCardsFor =
    off ?? (who.library === 0 ? `${capital(possessive(input, who))} library is empty.` : undefined)
  const you: Command[] = [
    {
      id: 'life',
      group: 'you',
      label: 'Life',
      effect: 'Gain or lose life.',
      amount: signed(LIMITS.life, LIFE_PRESETS),
      unavailable: off,
    },
    ...PLAYER_COUNTERS.map(([id, label, effect]) => ({
      id,
      group: 'you' as const,
      label,
      effect: effect(on),
      amount: signed(LIMITS.counters, COUNTER_PRESETS),
      unavailable: off,
    })),
  ]
  const library: Command[] = [
    {
      id: 'draw',
      group: 'library',
      label: 'Draw',
      effect: 'Put the top cards of your library into your hand.',
      amount: count(LIMITS.cards),
      unavailable: noCards,
    },
    lookCommand(input, noCards),
    {
      id: 'tutor',
      group: 'library',
      label: 'Search for a card',
      effect:
        (input.searched?.length ?? 0) > 0
          ? 'You are searching your library: choose a card, or shuffle without one.'
          : 'Look through your library, put a card into your hand, then shuffle.',
      unavailable: noCards,
    },
    {
      id: 'mill',
      group: 'library',
      label: 'Mill',
      effect: `Put the top cards of ${possessive(input, who)} library into ${possessive(input, who)} graveyard.`,
      amount: count(LIMITS.cards),
      unavailable: noCardsFor,
    },
    {
      id: 'exileTop',
      group: 'library',
      label: 'Exile from the top',
      effect: `Exile the top cards of ${possessive(input, who)} library face up.`,
      amount: count(LIMITS.cards),
      unavailable: noCardsFor,
    },
    {
      id: 'bottomTop',
      group: 'library',
      label: 'Top to the bottom',
      effect: 'Put the top cards of your library on the bottom, in the same order.',
      amount: count(LIMITS.cards),
      unavailable: noCards,
    },
    {
      id: 'shuffle',
      group: 'library',
      label: 'Shuffle',
      effect: 'Shuffle your library into a random order.',
      unavailable: off,
    },
    revealTopCommand(noCards),
  ]
  const make: Command[] = [
    {
      id: 'token',
      group: 'make',
      label: 'Tokens',
      effect: 'Create tokens under your control.',
      amount: count(LIMITS.tokens),
      choice: 'token',
      unavailable: off,
    },
    {
      id: 'mana',
      group: 'make',
      label: 'Mana',
      effect: 'Add mana to your pool until the step ends.',
      amount: count(LIMITS.mana),
      choice: 'mana',
      unavailable: off,
    },
    rollCommand(off),
  ]
  return [
    { id: 'you', label: on === 'you' ? 'You' : who.name, commands: you },
    { id: 'library', label: 'Library', commands: library },
    { id: 'make', label: 'Make', commands: make },
  ]
}

/** Look at the top: scry, surveil or reorder the cards looked at; pending cards say so on the row. */
function lookCommand(input: PanelInput, unavailable: string | undefined): Command {
  const waiting = input.looked.length
  return {
    id: 'look',
    group: 'library',
    label: 'Look at the top',
    effect:
      waiting > 0
        ? `You looked at the top ${plural(waiting, 'card')}: arrange them.`
        : 'Look at the top cards of your library, then put them back, on the bottom or in the graveyard.',
    amount: count(LIMITS.cards),
    unavailable,
  }
}

/** Reveal the top card: every seat reads it until it leaves the top or the library is shuffled. */
function revealTopCommand(unavailable: string | undefined): Command {
  return {
    id: 'revealTop',
    group: 'library',
    label: 'Reveal the top card',
    effect: 'Show the top card of your library to every seat.',
    unavailable,
  }
}

/** Roll a die or flip a coin: the engine draws the result from the game's random stream and every seat reads it. */
function rollCommand(unavailable: string | undefined): Command {
  return {
    id: 'roll',
    group: 'make',
    label: 'Roll a die / Flip a coin',
    effect: 'A coin, a d6, a d20 or a die of any size; everyone sees the result.',
    choice: 'roll',
    unavailable,
  }
}

/** The kind of mana a Mana row opens on: the seat's first identity colour, colourless for a colourless identity, Green without one. */
export function startMana(identity: readonly Color[] | undefined): ManaType {
  return identity === undefined ? 'Green' : (identity[0] ?? 'Colorless')
}

/** The draft a row of `command` opens with; its Mana row starts on the seat's commander identity. */
export function startDraft(command: Command, input?: Pick<PanelInput, 'identity'>): PanelDraft {
  return {
    ...START_TOKEN,
    amount: command.amount?.start ?? 0,
    mana: startMana(input?.identity),
    roll: 'd20',
    sides: SIDES.start,
  }
}

/** The amount after a press of a preset: a signed command adds it to the pending change, a count takes it. */
export function pressPreset(spec: AmountSpec, amount: number, preset: number): number {
  const next = spec.signed ? amount + preset : preset
  return Math.min(spec.max, Math.max(spec.min, next))
}

const plural = (n: number, one: string, many = `${one}s`): string => `${n} ${n === 1 ? one : many}`
const symbolOf = (mana: ManaType): string => MANA_KINDS.find(([kind]) => kind === mana)?.[1] ?? '?'

/** What applying `command` with `draft` changes, in the seat's words, and the action it sends. */
export function previewOf(command: Command, draft: PanelDraft, input: PanelInput): Preview {
  if (command.unavailable !== undefined) return { problem: command.unavailable }
  const { amount } = draft
  const spec = command.amount
  if (spec !== undefined && (!Number.isInteger(amount) || amount < spec.min || amount > spec.max)) {
    return {
      problem: spec.signed ? `Choose a change from ${spec.min} to ${spec.max}.` : `Choose 1 to ${spec.max}.`,
    }
  }
  if (spec?.signed === true && amount === 0)
    return { problem: 'Choose an amount with the buttons or the field.' }
  switch (command.id) {
    case 'life': {
      const who = subjectOf(input)
      const gain = amount > 0
      const life = `${Math.abs(amount)} life`
      return {
        preview: `${who.name}: ${who.life} → ${who.life + amount} life`,
        apply:
          who.seat === input.seat
            ? `${gain ? 'Gain' : 'Lose'} ${life}`
            : `${who.name} ${gain ? 'gains' : 'loses'} ${life}`,
        action: { Life: { player: who.seat, delta: amount } },
      }
    }
    case 'poison':
    case 'energy':
    case 'experience':
    case 'rad':
      return playerCounter(command, amount, input)
    case 'draw':
    case 'mill':
    case 'exileTop':
    case 'bottomTop':
      return fromTop(command.id, amount, input)
    case 'shuffle':
      return {
        preview: `${input.name}: ${plural(input.library, 'card')} in a new random order`,
        apply: 'Shuffle your library',
        action: { Shuffle: {} },
      }
    case 'look': {
      const seen = Math.min(amount, input.library)
      return {
        preview: `${input.name}: you see the top ${plural(seen, 'card')}; the other seats are told how many`,
        apply: `Look at the top ${plural(seen, 'card')}`,
        action: { LookTop: { count: amount } },
      }
    }
    case 'revealTop':
      return {
        preview: `${input.name}: the top card of your library is shown to every seat`,
        apply: 'Reveal the top card',
        action: { Reveal: { what: { Top: {} } } },
      }
    case 'tutor':
      return {
        preview: `${input.name}: you look through all ${plural(input.library, 'card')} of your library; the other seats are told you searched`,
        apply: 'Search your library',
        action: { Search: {} },
      }
    case 'token':
      return tokenPreview(draft, amount, input.tokens)
    case 'roll':
      return rollPreview(draft.roll, draft.sides)
    case 'mana': {
      const symbols = `{${symbolOf(draft.mana)}}`.repeat(amount)
      return {
        preview: `${input.name}: ${symbols} into your mana pool until the step ends`,
        apply: `Add ${symbols}`,
        action: { AddMana: { mana: draft.mana, count: amount } },
      }
    }
  }
}

function playerCounter(command: Command, amount: number, input: PanelInput): Preview {
  const who = subjectOf(input)
  const mine = who.seat === input.seat
  const kind = counterKind(command.id)
  const now = heldOf(who.counters, kind)
  const word = command.label.toLowerCase()
  if (now + amount < 0)
    return { problem: `${mine ? 'You have' : `${who.name} has`} ${plural(now, `${word} counter`)}.` }
  const counters = plural(Math.abs(amount), `${word} counter`)
  const to = mine ? '' : ` to ${who.name}`
  const from = mine ? '' : ` from ${who.name}`
  return {
    preview: `${who.name}: ${now} → ${now + amount} ${word}`,
    apply: amount > 0 ? `Add ${counters}${to}` : `Remove ${counters}${from}`,
    action: { PlayerCounters: { player: who.seat, counter: kind, delta: amount } },
  }
}

function fromTop(id: 'draw' | 'mill' | 'exileTop' | 'bottomTop', amount: number, input: PanelInput): Preview {
  // Mill and exile act on the picked seat's library; the others on the seat's own.
  const who = id === 'mill' || id === 'exileTop' ? subjectOf(input) : selfOf(input)
  const mine = who.seat === input.seat
  if (amount > who.library) {
    return { problem: `${mine ? 'Your' : `${who.name}’s`} library has ${plural(who.library, 'card')}.` }
  }
  const cards = plural(amount, 'card')
  const theTop = amount === 1 ? 'the top card goes' : `the top ${cards} go`
  const library = `library ${who.library} → ${who.library - amount}`
  const their = mine ? 'your' : `${who.name}’s`
  const of = mine ? {} : { library: who.seat }
  const from = mine ? '' : ` from ${who.name}`
  switch (id) {
    case 'draw':
      return {
        preview: `${input.name}: ${library}, hand ${input.hand} → ${input.hand + amount}`,
        apply: `Draw ${cards}`,
        action: { Draw: { count: amount } },
      }
    case 'mill':
      return {
        preview: `${who.name}: ${theTop} to ${their} graveyard (${library})`,
        apply: `Mill ${cards}${from}`,
        action: { MoveTop: { count: amount, to: 'Graveyard', ...of } },
      }
    case 'exileTop':
      return {
        preview: `${who.name}: ${theTop} to exile (${library})`,
        apply: `Exile ${cards}${from}`,
        action: { MoveTop: { count: amount, to: 'Exile', ...of } },
      }
    case 'bottomTop':
      return {
        preview: `${input.name}: ${theTop} to the bottom of your library`,
        apply: `Put ${cards} on the bottom`,
        action: { MoveTop: { count: amount, to: 'LibraryBottom' } },
      }
  }
}
