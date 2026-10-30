/** Pure: the commands of the Commands panel, what each one says it does, the preview of its change and the action it sends. */
import type { CounterKind, ManaType, ManualAction, ObjectId, PlayerId } from '@/protocol/generated'
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
  | 'draw'
  | 'look'
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

/** What the panel is made from: the seat and the numbers its commands change. */
export interface PanelInput {
  seat: PlayerId
  /** The seat's name, as the table shows it. */
  name: string
  life: number
  library: number
  hand: number
  counters: readonly (readonly [CounterKind, number])[]
  /** The top cards the seat looked at and has not put back yet, top first; none otherwise. */
  looked: readonly ObjectId[]
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

/** The player counters the panel lists by name, in order. */
const PLAYER_COUNTERS = [
  ['poison', 'Poison', 'Poison counters on you; ten of them and you lose.'],
  ['energy', 'Energy', 'Energy counters on you, to pay energy costs.'],
  ['experience', 'Experience', 'Experience counters on you.'],
] as const

/** The groups and commands of the panel for `input`. */
export function panelGroups(input: PanelInput): PanelGroup[] {
  const off = input.off
  const noCards = off ?? (input.library === 0 ? 'Your library is empty.' : undefined)
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
      effect,
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
      id: 'mill',
      group: 'library',
      label: 'Mill',
      effect: 'Put the top cards of your library into your graveyard.',
      amount: count(LIMITS.cards),
      unavailable: noCards,
    },
    {
      id: 'exileTop',
      group: 'library',
      label: 'Exile from the top',
      effect: 'Exile the top cards of your library face up.',
      amount: count(LIMITS.cards),
      unavailable: noCards,
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
    { id: 'you', label: 'You', commands: you },
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

/** The draft a row of `command` opens with. */
export function startDraft(command: Command): PanelDraft {
  return {
    ...START_TOKEN,
    amount: command.amount?.start ?? 0,
    mana: 'Green',
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
    case 'life':
      return {
        preview: `${input.name}: ${input.life} → ${input.life + amount} life`,
        apply: amount > 0 ? `Gain ${amount} life` : `Lose ${-amount} life`,
        action: { Life: { player: input.seat, delta: amount } },
      }
    case 'poison':
    case 'energy':
    case 'experience':
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
    case 'token':
      return tokenPreview(draft, amount)
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
  const kind = counterKind(command.id)
  const now = heldOf(input.counters, kind)
  const word = command.label.toLowerCase()
  if (now + amount < 0) return { problem: `You have ${plural(now, `${word} counter`)}.` }
  return {
    preview: `${input.name}: ${now} → ${now + amount} ${word}`,
    apply:
      amount > 0
        ? `Add ${plural(amount, `${word} counter`)}`
        : `Remove ${plural(-amount, `${word} counter`)}`,
    action: { PlayerCounters: { player: input.seat, counter: kind, delta: amount } },
  }
}

function fromTop(id: 'draw' | 'mill' | 'exileTop' | 'bottomTop', amount: number, input: PanelInput): Preview {
  if (amount > input.library) return { problem: `Your library has ${plural(input.library, 'card')}.` }
  const cards = plural(amount, 'card')
  const theTop = amount === 1 ? 'the top card goes' : `the top ${cards} go`
  const library = `library ${input.library} → ${input.library - amount}`
  switch (id) {
    case 'draw':
      return {
        preview: `${input.name}: ${library}, hand ${input.hand} → ${input.hand + amount}`,
        apply: `Draw ${cards}`,
        action: { Draw: { count: amount } },
      }
    case 'mill':
      return {
        preview: `${input.name}: ${theTop} to your graveyard (${library})`,
        apply: `Mill ${cards}`,
        action: { MoveTop: { count: amount, to: 'Graveyard' } },
      }
    case 'exileTop':
      return {
        preview: `${input.name}: ${theTop} to exile (${library})`,
        apply: `Exile ${cards}`,
        action: { MoveTop: { count: amount, to: 'Exile' } },
      }
    case 'bottomTop':
      return {
        preview: `${input.name}: ${theTop} to the bottom of your library`,
        apply: `Put ${cards} on the bottom`,
        action: { MoveTop: { count: amount, to: 'LibraryBottom' } },
      }
  }
}
