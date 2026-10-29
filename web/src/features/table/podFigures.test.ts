// Goal: the words and figures of a seat pod are pure: a card count reads "1 card" or "n cards", a chip is named after its zone and count, the zones drawn depend on the command zone alone, the state tokens are exactly `active` and `priority`, the priority dot is for a seat that holds priority without the turn and is named after it, a tile's name repeats everything the tile shows, and a seat's hue differs from its neighbours' for any table up to 12 seats.
import { describe, expect, it } from 'vitest'
import type { PlayerView } from '@/protocol/generated'
import {
  cardCount,
  emptyLabel,
  chipZones,
  podCounts,
  podState,
  podStateWords,
  priorityName,
  seatHue,
  showsPriorityDot,
  tileName,
  zoneLabel,
  ZONE_NAME,
} from './podFigures'
import type { PodCounts, ZoneKey } from './podFigures'

const COUNTS: PodCounts = { hand: 7, library: 46, graveyard: 5, exile: 1, command: 1 }

const zones = (counts: Partial<Record<ZoneKey, number>>): PlayerView =>
  ({
    hand: { count: counts.hand ?? 0, cards: null },
    library: { count: counts.library ?? 0, cards: null },
    graveyard: { count: counts.graveyard ?? 0, cards: [] },
    command: { count: counts.command ?? 0, cards: [] },
  }) as unknown as PlayerView

describe('cardCount and zoneLabel', () => {
  it.each([
    [0, '0 cards'],
    [1, '1 card'],
    [2, '2 cards'],
    [99, '99 cards'],
  ])('says %i as "%s"', (count, words) => {
    expect(cardCount(count)).toBe(words)
  })

  it.each<[ZoneKey, number, string]>([
    ['hand', 7, 'Hand 7 cards'],
    ['library', 1, 'Library 1 card'],
    ['graveyard', 0, 'Graveyard 0 cards'],
    ['exile', 12, 'Exile 12 cards'],
    ['command', 1, 'Command zone 1 card'],
  ])('names the %s chip with %i cards "%s"', (zone, count, name) => {
    expect(zoneLabel(zone, count)).toBe(name)
    expect(ZONE_NAME[zone]).toBe(name.split(/ \d/)[0])
  })
})

describe('podCounts and chipZones', () => {
  it('reads the counts of the zones the view carries and the exile the caller counted', () => {
    expect(podCounts(zones({ hand: 7, library: 46, graveyard: 5, command: 1 }), 3)).toEqual({
      ...COUNTS,
      exile: 3,
    })
  })

  it.each([
    [0, ['hand', 'library', 'graveyard', 'exile']],
    [1, ['hand', 'library', 'graveyard', 'exile', 'command']],
    [2, ['hand', 'library', 'graveyard', 'exile', 'command']],
  ])('draws the zones of a command zone holding %i cards: %j', (command, drawn) => {
    expect(chipZones({ ...COUNTS, command })).toEqual(drawn)
  })

  it('draws the four chips even when every one is empty: a zero is a figure', () => {
    expect(chipZones({ hand: 0, library: 0, graveyard: 0, exile: 0, command: 0 })).toHaveLength(4)
  })
})

describe('podState, podStateWords and the priority dot', () => {
  it.each([
    [false, false, undefined, '', false],
    [true, false, 'active', 'Active turn.', false],
    // The dot has its own name, so the words add nothing for a seat that holds priority without the turn.
    [false, true, 'priority', '', true],
    [true, true, 'active priority', 'Active turn. Has priority.', false],
  ])('active %s and priority %s: %j, "%s", dot %s', (active, priority, state, words, dot) => {
    expect(podState(active, priority)).toBe(state)
    expect(podStateWords(active, priority)).toBe(words)
    expect(showsPriorityDot(active, priority)).toBe(dot)
  })

  it.each([
    ['Bo', false, 'Bo has priority'],
    ['Alice', false, 'Alice has priority'],
    ['Alice', true, 'You have priority'],
  ])('names the dot of %s (you: %s) "%s"', (name, you, expected) => {
    expect(priorityName(name, you)).toBe(expected)
  })
})

describe('tileName', () => {
  const facts = {
    name: 'Bo',
    life: 38,
    counts: COUNTS,
    active: false,
    priority: false,
    eliminated: false,
    focused: false,
  }

  it('says the action and then every figure of the tile', () => {
    expect(tileName(facts)).toBe(
      "Show Bo's board. Life 38, 7 cards in hand, 46 in library, 5 in graveyard, 1 in exile, 1 in the command zone",
    )
  })

  it('leaves out the command zone while it is empty', () => {
    expect(tileName({ ...facts, counts: { ...COUNTS, command: 0 } })).toBe(
      "Show Bo's board. Life 38, 7 cards in hand, 46 in library, 5 in graveyard, 1 in exile",
    )
  })

  it('ends with the marks of the turn, the elimination and whether it is shown', () => {
    expect(tileName({ ...facts, active: true, priority: true, eliminated: true, focused: true })).toMatch(
      /, active turn, has priority, eliminated, shown now$/,
    )
  })
})

describe('seatHue', () => {
  it('is a hue of the colour wheel for any seat', () => {
    for (const seat of [-3, 0, 1, 11, 12, 255, Number.NaN]) {
      const hue = seatHue(seat)
      expect(hue).toBeGreaterThanOrEqual(0)
      expect(hue).toBeLessThan(360)
    }
  })

  it('keeps every seat of a table of up to 12 apart from the others by at least 12 degrees', () => {
    for (let seats = 2; seats <= 12; seats += 1) {
      const hues = Array.from({ length: seats }, (_, seat) => seatHue(seat))
      for (let a = 0; a < hues.length; a += 1) {
        for (let b = a + 1; b < hues.length; b += 1) {
          const gap = Math.abs((hues[a] ?? 0) - (hues[b] ?? 0))
          expect(Math.min(gap, 360 - gap), `${seats} seats: ${a} and ${b}`).toBeGreaterThanOrEqual(12)
        }
      }
    }
  })

  it('is the same for a seat wherever it is asked', () => {
    expect(seatHue(3)).toBe(seatHue(3))
  })
})

describe('the label of an empty zone', () => {
  it.each<[ZoneKey, number, string]>([
    ['graveyard', 44, 'Grave'],
    ['graveyard', 59, 'Grave'],
    ['graveyard', 60, 'Graveyard'],
    ['graveyard', 92, 'Graveyard'],
    ['exile', 44, 'Exile'],
    ['command', 48, 'Cmd'],
    ['command', 74, 'Command zone'],
  ])('writes the %s at %i px as %s', (zone, width, label) => {
    expect(emptyLabel(zone, width)).toBe(label)
  })
})
