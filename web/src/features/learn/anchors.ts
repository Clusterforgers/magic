import type { Step } from '@/protocol/generated'

/** One named part of the table the coach can light: what it is, a label for texts, and what opens it when hidden. */
export interface AnchorInfo {
  /** The words a text uses for it when the element has none of its own. */
  label: string
  /** The anchor that shows this one when it is hidden (a folded sheet, a closed menu), if any; of several the first drawn. */
  opener?: string | readonly string[]
}

/** The steps a stop can be set at, as the stops bar names them. */
export const STOP_STEPS = [
  'Upkeep',
  'Draw',
  'Main1',
  'BeginCombat',
  'DeclareAttackers',
  'DeclareBlockers',
  'CombatDamage',
  'EndCombat',
  'Main2',
  'End',
] as const satisfies readonly Step[]

type StopStep = (typeof STOP_STEPS)[number]

/** The anchors with a fixed id, in the registry's order. */
const FIXED = {
  'opening.hand': { label: 'your opening hand' },
  'opening.keep': { label: 'Keep' },
  'opening.mulligan': { label: 'Mulligan' },
  'own.pod': { label: 'your panel' },
  'own.life': { label: 'your life' },
  'own.board': { label: 'your battlefield' },
  'own.graveyard': { label: 'your graveyard' },
  'own.lands': { label: 'your lands', opener: 'own.lands-chip' },
  'own.lands-chip': { label: 'your lands' },
  'opp.pod': { label: "the opponent's panel" },
  'opp.life': { label: "the opponent's life" },
  'opp.board': { label: "the opponent's battlefield" },
  'opp.graveyard': { label: "the opponent's graveyard" },
  'turn.badge': { label: 'the turn badge' },
  'turn.ladder': { label: 'the steps of the turn', opener: 'turn.badge' },
  ask: { label: 'the question and its answers' },
  'ask.card': { label: 'the asking card' },
  'ask.primary': { label: 'the answer' },
  'ask.question': { label: 'the question' },
  stack: { label: 'the stack' },
  'stack.top': { label: 'the top of the stack' },
  'pass.primary': { label: 'the primary button' },
  'pass.end-turn': { label: 'End turn', opener: 'pass.more' },
  'pass.more': { label: 'More' },
  hand: { label: 'your hand' },
  'hand.menu': { label: 'the order of your hand', opener: 'table.menu' },
  command: { label: 'the command zone' },
  'command.tax': { label: 'the commander tax' },
  'pay.tap': { label: 'Tap', opener: ['side.tab.stack', 'sheet.grab'] },
  'pay.auto': { label: 'Pay for me' },
  'pay.confirm': { label: 'Confirm' },
  'pay.meter': { label: 'the cost' },
  'sheet.grab': { label: 'the payment' },
  'attack.all': { label: 'Attack with all' },
  'attack.confirm': { label: 'Attack' },
  'attack.none': { label: 'No attack' },
  'block.confirm': { label: 'Block' },
  'block.none': { label: 'No blocks' },
  'targets.confirm': { label: 'Confirm' },
  'commander.return': { label: 'Command zone' },
  'stops.side.theirs': { label: "the opponent's stops", opener: 'stops.open' },
  'stops.open': { label: 'Stops' },
  fullcontrol: { label: 'Full Control', opener: 'stops.open' },
  'side.open': { label: 'the side panel' },
  'side.tab.log': { label: 'Log', opener: 'side.open' },
  'side.tab.commands': { label: 'Commands', opener: 'side.open' },
  'side.tab.stack': { label: 'Stack', opener: 'side.open' },
  log: { label: 'the log', opener: 'side.tab.log' },
  commands: { label: 'the Commands panel', opener: 'side.tab.commands' },
  'help.menu': { label: 'Help' },
  'help.lexicon': { label: 'Rules lexicon', opener: 'help.menu' },
  lexicon: { label: 'the rules lexicon' },
  'lexicon.close': { label: 'Close' },
  'table.menu': { label: 'the table menu' },
  preview: { label: 'the card preview' },
  'preview.keywords': { label: 'the keywords' },
} as const satisfies Record<string, AnchorInfo>

/** An anchor with a fixed id. */
export type FixedAnchor = keyof typeof FIXED

/** A step's stop in the stops bar. */
export type StopAnchor = `stops.step.${StopStep}`

/** Every anchor id an element may carry. */
export type AnchorId = FixedAnchor | StopAnchor

/** The zones a card ref names: `hand:Forest`, `board:Grizzly Bears`. */
export const CARD_ZONES = ['hand', 'command', 'board', 'opp', 'stack'] as const

/** The zone part of a card ref. */
export type CardZone = (typeof CARD_ZONES)[number]

/** A card by name in a zone: the first one there the pending decision can use, else the first. */
export type CardRef = `${CardZone}:${string}`

/** What a step may light or press: an anchor or a card ref. */
export type Target = AnchorId | CardRef

/** The registry: every fixed anchor and every stop step. */
export const ANCHORS: Readonly<Record<AnchorId, AnchorInfo>> = {
  ...FIXED,
  ...(Object.fromEntries(
    STOP_STEPS.map((step) => [`stops.step.${step}`, { label: `the ${step} stop`, opener: 'stops.open' }]),
  ) as Record<StopAnchor, AnchorInfo>),
}

/** The fixed anchor ids, in the registry's order. */
export const FIXED_ANCHORS = Object.keys(FIXED) as FixedAnchor[]

/** Whether `id` is a registered anchor. */
export function isAnchor(id: string): id is AnchorId {
  return Object.hasOwn(ANCHORS, id)
}

/** `ref` split into its zone and card name, or null when it is an anchor. */
export function cardRef(ref: string): { zone: CardZone; name: string } | null {
  const at = ref.indexOf(':')
  if (at < 0) return null
  const zone = ref.slice(0, at)
  if (!(CARD_ZONES as readonly string[]).includes(zone)) return null
  return { zone: zone as CardZone, name: ref.slice(at + 1) }
}

/** The props that mark an element as anchor `id`. */
export function anchor(id: AnchorId): { 'data-coach': AnchorId } {
  return { 'data-coach': id }
}

/** The props that mark an element as the zone a card ref names; its cards carry `data-object-id`. */
export function coachZone(zone: CardZone): { 'data-coach-zone': CardZone } {
  return { 'data-coach-zone': zone }
}

/** The coach's props of an element that may carry an anchor, passed down to the element that draws it. */
export type CoachMark = { 'data-coach'?: AnchorId }
