/**
 * The dev table's `?prompt=targets-twins`: the first opponent's second creature is a copy of its first, untapped and
 * alike in all a face shows, and the viewer's spell asks for one or two of every creature on the table, more than the
 * table chooses in place, so the targets stand in the choice overlay with the twins numbered (`twins.ts`).
 */
import type { PendingDecision } from '@/play/store'
import type { ObjectView, Target } from '@/protocol/generated'
import { creaturesOf, opponents, ownSpell } from './effectPrompts'
import type { Fixture } from './fixture'

/** The first opponent's first two creatures, or none where it has fewer. */
function twinPair(fixture: Fixture): [ObjectView, ObjectView] | null {
  const { view } = fixture
  const [seat] = opponents(view)
  if (seat === undefined) return null
  const [first, second] = creaturesOf(view, seat)
  return first === undefined || second === undefined ? null : [first, second]
}

/** The fixture with the first opponent's second creature a twin of its first. */
function twinTable(fixture: Fixture): Fixture {
  const pair = twinPair(fixture)
  if (pair === null) return fixture
  const [first, second] = pair
  const twin: ObjectView = {
    ...first,
    id: second.id,
    attachments: [],
    permanent: first.permanent === null ? null : { ...first.permanent, tapped: false, damage: 0 },
  }
  const original: ObjectView = {
    ...first,
    permanent: first.permanent === null ? null : { ...first.permanent, tapped: false, damage: 0 },
  }
  const objects = fixture.view.objects.map((object) =>
    object.id === second.id ? twin : object.id === first.id ? original : object,
  )
  return { ...fixture, view: { ...fixture.view, objects } }
}

/** The viewer's spell asks for one or two of every creature on the table. */
function twinTargets({ view }: Fixture): PendingDecision {
  const creatures = view.players.flatMap((player) => creaturesOf(view, player.seat))
  return {
    id: 1,
    decision: {
      player: view.viewer,
      kind: {
        ChooseTargets: {
          source: ownSpell(view)?.id ?? 0,
          spec: 0,
          legal: creatures.map((object): Target => ({ Object: { id: object.id, counter: 0 } })),
          min: 1,
          max: 2,
          can_cancel: true,
          wording: 'one or two target creatures',
        },
      },
    },
  }
}

/** The twin scene's prompt. */
export const TWIN_PROMPTS: Record<string, (fixture: Fixture) => PendingDecision> = {
  'targets-twins': twinTargets,
}

/** The twin scene's table. */
export const TWIN_TABLES: Record<string, (fixture: Fixture) => Fixture> = {
  'targets-twins': twinTable,
}
