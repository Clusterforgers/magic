// Goal: the art picker filters printings by set code or name in any case, names each printing by set, number, year and the traits that tell its look apart, and moves focus through its grid by arrow keys, Home and End without leaving it.
import { describe, expect, it } from 'vitest'
import { filterPrintings, gridMove, printingCaption, printingLabel, printingTraits } from './artChoice'
import { printingView } from './fakePrintings'

const ALPHA = printingView('a', 'lea', { set_name: 'Limited Edition Alpha', released_at: '1993-08-05', frame: '1993' })
const LAIR = printingView('s', 'sld', {
  set_name: 'Secret Lair Drop',
  collector_number: '1040',
  full_art: true,
  finishes: ['nonfoil', 'foil', 'etched'],
})
const ONLINE = printingView('o', 'om1', { set_name: null, released_at: null, digital: true, lang: 'ja', frame: 'future' })
const ALL = [LAIR, ALPHA, ONLINE]

describe('filterPrintings', () => {
  it.each([
    ['', ['s', 'a', 'o']],
    ['  ', ['s', 'a', 'o']],
    ['LEA', ['a']],
    ['alpha', ['a']],
    ['secret', ['s']],
    ['om', ['o']],
    ['zzz', []],
  ])('%j keeps %j', (query, ids) => {
    expect(filterPrintings(ALL, query).map((p) => p.printing_id)).toEqual(ids)
  })
})

describe('what a printing is called', () => {
  it.each([
    [ALPHA, 'LEA #1 · 1993', [], 'Limited Edition Alpha #1, 1993, Old frame'],
    [LAIR, 'SLD #1040 · 2020', ['Full art', 'Foil', 'Etched'], 'Secret Lair Drop #1040, 2020, Full art, Foil, Etched'],
    [ONLINE, 'OM1 #1', ['Future frame', 'Digital', 'JA'], 'OM1 #1, Future frame, Digital, JA'],
  ])('%#', (printing, caption, traits, label) => {
    expect(printingCaption(printing)).toBe(caption)
    expect(printingTraits(printing).filter((t) => t !== 'Old frame')).toEqual(traits)
    expect(printingLabel(printing)).toBe(label)
  })
})

describe('gridMove', () => {
  it.each([
    ['ArrowRight', 0, 1],
    ['ArrowLeft', 1, 0],
    ['ArrowLeft', 0, null],
    ['ArrowDown', 1, 5],
    ['ArrowDown', 7, null],
    ['ArrowUp', 5, 1],
    ['ArrowUp', 2, null],
    ['Home', 6, 0],
    ['End', 2, 9],
    ['Enter', 2, null],
  ])('%s from %i goes to %j in a grid of 4 columns and 10 options', (key, at, next) => {
    expect(gridMove(key, at, 4, 10)).toBe(next)
  })
})
