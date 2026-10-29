/** The decisions that look at the library or order effects, for the dev table: `?prompt=replacement`, `replacement-two`, `scry`, `scry-one`, `scry-order`, `scry-bottom`, `put-back` and their clocks. */
import type { PendingDecision } from '@/play/store'
import type { ObjectId, ObjectView, ReplacementCandidate, StateView } from '@/protocol/generated'
import { CREATURES, INSTANTS, LANDS } from './cards'
import type { Template } from './cards'
import type { Fixture } from './fixture'

type LookedPrompt = (fixture: Fixture) => PendingDecision
type LookedMoment = (fixture: Fixture) => Fixture

/** The scry's own question, in the engine's words (`wording::SCRY`). */
const SCRY_WORDS = 'Put any number of the cards on the bottom of your library.'

const TOP_WORDS = 'Put the cards on top of your library in any order.'
const BOTTOM_WORDS = 'Put the cards on the bottom of your library in any order.'

/** The cards of the viewer's library that a scry looks at, top first. */
const LOOKED: readonly Template[] = [
  CREATURES[3] ?? CREATURES[0],
  INSTANTS[0] ?? LANDS[0],
  LANDS[1] ?? LANDS[0],
  CREATURES[2] ?? CREATURES[0],
  INSTANTS[3] ?? LANDS[0],
]

const libraryCard = (template: Template, id: ObjectId, seat: number): ObjectView => ({
  id,
  owner: seat,
  controller: seat,
  zone: { Library: seat },
  name: template.name,
  mana_cost: template.cost,
  supertypes: template.supertypes ?? [],
  types: template.types,
  subtypes: template.subtypes ?? [],
  colors: template.colors,
  power: template.power ?? null,
  toughness: template.toughness ?? null,
  abilities: template.produces === undefined ? [] : [{ Mana: { produces: template.produces } }],
  permanent: null,
})

/** The ids of the cards the viewer looks at, top first: the objects the moment added to the library. */
const lookedIds = (view: StateView): ObjectId[] =>
  view.objects
    .filter((o) => typeof o.zone === 'object' && 'Library' in o.zone && o.zone.Library === view.viewer)
    .map((o) => o.id)

/** The viewer's library as a scry shows it: five cards of the top, known to the viewer alone. */
const libraryMoment: LookedMoment = (fixture) => {
  const { view } = fixture
  const first = Math.max(...view.objects.map((o) => o.id)) + 1
  const cards = LOOKED.map((template, index) => libraryCard(template, first + index, view.viewer))
  return { ...fixture, view: { ...view, objects: [...view.objects, ...cards] } }
}

const askerOf = (view: StateView): ObjectId =>
  view.objects.find(
    (o) => o.zone === 'Battlefield' && o.controller === view.viewer && o.types.includes('Creature'),
  )?.id ?? 0

const handOf = (view: StateView): ObjectId[] => view.players[view.viewer]?.hand.cards ?? []

/** A scry of `count` cards: which of them go to the bottom. */
const scry =
  (count: number): LookedPrompt =>
  ({ view }) => ({
    id: 1,
    decision: {
      player: view.viewer,
      kind: {
        ChooseCards: {
          source: askerOf(view),
          from: lookedIds(view).slice(0, count),
          min: 0,
          max: count,
          text: SCRY_WORDS,
        },
      },
    },
  })

/** The cards that stay on top (or go to the bottom) are put in order. */
const orderLooked =
  (count: number, text: string): LookedPrompt =>
  ({ view }) => ({
    id: 1,
    decision: {
      player: view.viewer,
      kind: { OrderCards: { source: askerOf(view), cards: lookedIds(view).slice(0, count), text } },
    },
  })

/** Two cards of the hand are put back on top of the library, as a Brainstorm does. */
const putBack: LookedPrompt = ({ view }) => ({
  id: 1,
  decision: {
    player: view.viewer,
    kind: { OrderCards: { source: askerOf(view), cards: handOf(view).slice(0, 2), text: TOP_WORDS } },
  },
})

/** Several permanents of the viewer's replace the same event. */
const replacement =
  (count: number): LookedPrompt =>
  ({ view }) => {
    const permanents = view.objects.filter(
      (o) => o.zone === 'Battlefield' && o.controller === view.viewer && !o.types.includes('Land'),
    )
    const candidates: ReplacementCandidate[] = permanents
      .slice(0, count)
      .map((o, index) => ({ source: o.id, ability: 0, ...(index === 2 ? { shield: 1 } : {}) }))
    return {
      id: 1,
      decision: {
        player: view.viewer,
        kind: { ChooseReplacement: { candidates, text: 'Create 2 token(s).' } },
      },
    }
  }

const clocked =
  (prompt: LookedPrompt): LookedPrompt =>
  (fixture) => ({
    ...prompt(fixture),
    deadlineMs: 10_000,
  })

/** The moments the dev table needs for the decisions above, by `?prompt=` name. */
export const LOOKED_MOMENTS: Record<string, LookedMoment> = {
  scry: libraryMoment,
  'scry-clock': libraryMoment,
  'scry-one': libraryMoment,
  'scry-order': libraryMoment,
  'scry-bottom': libraryMoment,
}

/** The decisions by their `?prompt=` name; the viewer is not the active seat, so these run on a response clock. */
export const LOOKED_PROMPTS: Record<string, LookedPrompt> = {
  replacement: replacement(3),
  'replacement-two': replacement(2),
  'replacement-clock': clocked(replacement(3)),
  scry: scry(4),
  'scry-one': scry(1),
  'scry-clock': clocked(scry(4)),
  'scry-order': orderLooked(4, TOP_WORDS),
  'scry-bottom': orderLooked(3, BOTTOM_WORDS),
  'put-back': putBack,
}
