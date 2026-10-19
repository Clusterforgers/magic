// Goal: the filters become a card request in the order the server reads them (colour letters in WUBRG order, text trimmed, a wrong mana value left out with its reason in words), a search with nothing to search for makes no request, and the zones a format uses are the ones the deck check allows.
import { describe, expect, it } from 'vitest'
import { DECK_FORMATS } from '@/features/decks/format'
import {
  FILTER_FORMATS,
  hasCriteria,
  IDENTITY_LETTERS,
  manaValueProblem,
  NO_FILTERS,
  queryOf,
  toggleLetter,
} from './search'
import type { SearchFilters } from './search'
import { ZONES, zonesFor } from './zones'

const filters = (over: Partial<SearchFilters>): SearchFilters => ({ ...NO_FILTERS, ...over })

describe('manaValueProblem', () => {
  it.each(['', '  ', '0', '3', '12', '2-4', '0-0', '5+', ' 5+ '])('accepts %j', (value) => {
    expect(manaValueProblem(value)).toBeNull()
  })

  it.each(['x', '2-', '-4', '2 - 4', '5++', '1.5', '100', '2-4-6', '+5', '4-2'])(
    'refuses %j in words',
    (value) => {
      expect(manaValueProblem(value)).toMatch(/\.$/)
    },
  )

  it('says what a range runs from', () => {
    expect(manaValueProblem('4-2')).toMatch(/smaller number to the larger/)
    expect(manaValueProblem('x')).toMatch(/5\+/)
  })
})

describe('queryOf', () => {
  it('is null with nothing to search for, whatever the page', () => {
    expect(queryOf(NO_FILTERS)).toBeNull()
    expect(queryOf(filters({ text: '   ', type: ' ' }), 3)).toBeNull()
    expect(hasCriteria(NO_FILTERS)).toBe(false)
  })

  it('carries every field, trimmed, at the page asked for', () => {
    const query = queryOf(
      filters({ text: ' bolt ', identity: ['R'], type: ' instant ', manaValue: ' 1 ', format: 'modern' }),
      3,
    )
    expect(query).toEqual({
      text: 'bolt',
      identity: 'R',
      type: 'instant',
      manaValue: '1',
      format: 'modern',
      page: 3,
    })
  })

  it('searches page 1 unless told otherwise', () => {
    expect(queryOf(filters({ text: 'x' }))?.page).toBe(1)
  })

  it('writes colour letters in WUBRG then C order whatever order they were switched on', () => {
    expect(queryOf(filters({ identity: ['C', 'G', 'W', 'B'] }))?.identity).toBe('WBGC')
  })

  it('searches on a filter alone', () => {
    for (const only of [
      { identity: ['U'] },
      { type: 'elf' },
      { manaValue: '2-4' },
      { format: 'commander' },
    ]) {
      expect(hasCriteria(filters(only))).toBe(true)
    }
  })

  it('leaves out a mana value the server would refuse, and searches on what else there is', () => {
    expect(queryOf(filters({ text: 'x', manaValue: '2-' }))).toMatchObject({ text: 'x', manaValue: '' })
    expect(queryOf(filters({ manaValue: '2-' }))).toBeNull()
  })
})

describe('toggleLetter', () => {
  it('switches a letter on and off, keeping WUBRG order', () => {
    expect(toggleLetter([], 'G')).toEqual(['G'])
    expect(toggleLetter(['G'], 'W')).toEqual(['W', 'G'])
    expect(toggleLetter(['W', 'G'], 'G')).toEqual(['W'])
    expect(toggleLetter(['W'], 'W')).toEqual([])
  })

  it('knows the six identities', () => {
    expect(IDENTITY_LETTERS).toEqual(['W', 'U', 'B', 'R', 'G', 'C'])
  })
})

describe('the formats to filter by', () => {
  it('are the deck formats except Freeform, which has no legality to filter by', () => {
    expect(FILTER_FORMATS).toEqual(DECK_FORMATS.filter((key) => key !== 'freeform'))
    expect(FILTER_FORMATS).toContain('commander')
  })
})

describe('zonesFor', () => {
  it.each([
    ['commander', ['commander', 'main', 'sideboard', 'companion']],
    ['freeform', ['commander', 'main', 'sideboard', 'companion']],
    ['modern', ['main', 'sideboard', 'companion']],
    ['pauper', ['main', 'sideboard', 'companion']],
  ])('lists the zones of %s', (format, zones) => {
    expect(zonesFor(format)).toEqual(zones)
  })

  it('keeps the order of ZONES', () => {
    expect(ZONES).toEqual(['commander', 'main', 'sideboard', 'companion'])
  })
})
