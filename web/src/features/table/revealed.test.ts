// Goal: what Reveal and Note put in the view reads the same everywhere: a seat's revealed cards and where each is, the chip's words, the first words of a note cut at three words or 18 characters, and the words a card's spoken name adds.
import { describe, expect, it } from 'vitest'
import type { PlayerView } from '@/protocol/generated'
import { buildFixture } from './dev/fixture'
import {
  handMarkWords,
  noteLead,
  noteName,
  revealedChipName,
  revealedLabel,
  revealedLine,
  revealedOf,
  revealedPlace,
  revealedWhere,
} from './revealed'
import { must } from '@/test-utils/batches'

const player = (over: Partial<PlayerView>): PlayerView => ({
  ...must(buildFixture({ seats: 2 }).view.players[0]),
  ...over,
})

describe('the cards a seat shows', () => {
  it.each<[string, Partial<PlayerView>, number[], number | undefined]>([
    ['nothing', {}, [], undefined],
    ['two hand cards', { revealed_hand: [4, 5] }, [4, 5], undefined],
    ['the top card', { revealed_top: 9 }, [9], 9],
    ['both, the hand first', { revealed_hand: [4], revealed_top: 9 }, [4, 9], 9],
  ])('lists %s', (_name, over, all, top) => {
    const shown = revealedOf(player(over))
    expect(shown.all).toEqual(all)
    expect(shown.top).toBe(top)
  })

  it('shows nothing for a seat that is not in the view', () => {
    expect(revealedOf(undefined).all).toEqual([])
  })

  it.each([
    [4, 'hand'],
    [5, 'hand'],
    [9, 'top'],
    [7, undefined],
  ] as const)('finds card %i in %s of the seats', (id, place) => {
    const seats = [player({ seat: 0 }), player({ seat: 1, revealed_hand: [4, 5], revealed_top: 9 })]
    expect(revealedPlace(seats, id)).toBe(place)
  })
})

describe('the words of the chip and the preview', () => {
  it('names the chip with the count and where the cards are', () => {
    const both = revealedOf(player({ revealed_hand: [4, 5], revealed_top: 9 }))
    expect(revealedChipName(both)).toBe('Revealed 3 cards: 2 in hand, the top of the library')
    expect(revealedWhere(revealedOf(player({ revealed_top: 9 })))).toBe('the top of the library')
    expect(revealedLabel(1)).toBe('Revealed 1 card')
  })

  it('says where a card is shown in the preview', () => {
    expect(revealedLine('hand', 'Bo')).toBe("Revealed to every seat, in Bo's hand.")
    expect(revealedLine('top', 'Bo')).toBe("Revealed to every seat, on top of Bo's library.")
  })

  it.each([
    [undefined, undefined, []],
    ['hand', undefined, ['revealed to every seat']],
    [undefined, 'Back to hand', ['note: Back to hand']],
    ['top', 'Mind the Trick', ['revealed to every seat', 'note: Mind the Trick']],
  ] as const)('adds %s and %s to the spoken name', (place, note, words) => {
    expect(handMarkWords(place, note)).toEqual(words)
  })
})

describe('the first words of a note', () => {
  it.each([
    ['Tapped', 'Tapped'],
    ['Back to hand', 'Back to hand'],
    ['Back to hand next turn', 'Back to hand…'],
    ['  spaced   out  words  ', 'spaced out words'],
    ['Supercalifragilistic', 'Supercalifragilis…'],
    ['Holds the trick until my end step', 'Holds the trick…'],
    ['', ''],
  ])('shows %j as %j', (note, lead) => {
    expect(noteLead(note)).toBe(lead)
  })

  it('never cuts the name, which holds the whole note', () => {
    const note = 'Holds the trick until my end step'
    expect(noteName(note)).toBe(`Note: ${note}`)
  })
})
