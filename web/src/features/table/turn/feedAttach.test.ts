// Goal: the ticker line of an attach event names the host the attachment left, from the game before the batch: "moves from A to B" while it hung on a permanent or a player, "is attached to B" while it hung on nothing, and the usual words for a batch of the same events that the game before knows nothing of.
import { describe, expect, it } from 'vitest'
import { indexById } from '@/play/view'
import type { ObjectView } from '@/protocol/generated'
import { card } from '@/test-utils/cards'
import { momentAt, nameOf, pub } from '@/test-utils/turnEvents'
import type { TurnAt } from '@/test-utils/turnEvents'
import type { Moment } from '../fx/derive'
import { deriveFeed } from './feed'

const NOW: TurnAt = { number: 5, active: 1, step: 'Main1' }
const BEARS = 9001
const ELVES = 9002
const SWORD = 9003

/** The game with a Bonesplitter that hangs on `host` (a creature, or nothing), in the open on the battlefield. */
function momentWith(host: Partial<ObjectView>): Moment {
  const { view } = momentAt(NOW)
  const objects = [
    ...view.objects,
    card({ id: BEARS, name: 'Grizzly Bears' }),
    card({ id: ELVES, name: 'Llanowar Elves' }),
    card({ id: SWORD, name: 'Bonesplitter', types: ['Artifact'], subtypes: ['Equipment'], ...host }),
  ]
  const shown = { ...view, objects }
  return { view: shown, objectsById: indexById(objects) }
}

const tickerOf = (before: Moment, to: number): string[] =>
  deriveFeed(before, before, [pub({ Attached: { object: SWORD, to } })], nameOf).lines.map(
    (line) => line.text,
  )

describe('the ticker line of an attachment that moves', () => {
  it.each([
    ['a creature', { attached_to: BEARS }, 'Bonesplitter moves from Grizzly Bears to Llanowar Elves'],
    ['a player', { attached_player: 0 }, 'Bonesplitter moves from Alice to Llanowar Elves'],
    ['nothing', {}, 'Bonesplitter is attached to Llanowar Elves'],
  ])('says it from %s', (_name, host, text) => {
    expect(tickerOf(momentWith(host), ELVES)).toEqual([text])
  })

  it('keeps the usual words for an attachment that stays where it is', () => {
    expect(tickerOf(momentWith({ attached_to: ELVES }), ELVES)).toEqual([
      'Bonesplitter becomes attached to Llanowar Elves',
    ])
  })
})
