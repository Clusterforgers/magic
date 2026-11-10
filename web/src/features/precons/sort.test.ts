// Goal: the precon list sorts newest first as given, by name, or with the account's favorites first in the given order within each group, without changing the list, and the chosen order is kept in storage that may refuse or hold nonsense.
import { describe, expect, it } from 'vitest'
import { preconItem } from './fakePrecons'
import { loadPreconSort, PRECON_SORT_KEY, savePreconSort, sortPrecons } from './sort'
import type { PreconSort } from './sort'

const ITEMS = [
  preconItem({ code: 'a', name: 'Zombie Horde' }),
  preconItem({ code: 'b', name: 'arcane wizardry' }),
  preconItem({ code: 'c', name: 'Draconic Domination' }),
  preconItem({ code: 'd', name: 'Arcane Maelstrom' }),
]

describe('sortPrecons', () => {
  it.each<[string, PreconSort, string[], string[]]>([
    ['keeps the server order for newest', 'newest', ['c'], ['a', 'b', 'c', 'd']],
    ['orders by name ignoring case', 'name', [], ['d', 'b', 'c', 'a']],
    ['moves the favorites up, each group in the given order', 'favorites', ['d', 'b'], ['b', 'd', 'a', 'c']],
    ['keeps the order with no favorites', 'favorites', [], ['a', 'b', 'c', 'd']],
    [
      'keeps the order when every precon is a favorite',
      'favorites',
      ['d', 'c', 'b', 'a'],
      ['a', 'b', 'c', 'd'],
    ],
    ['ignores a favorite that is not listed', 'favorites', ['gone', 'c'], ['c', 'a', 'b', 'd']],
  ])('%s', (_what, sort, favorites, codes) => {
    const before = [...ITEMS]
    expect(sortPrecons(ITEMS, sort, new Set(favorites)).map((item) => item.code)).toEqual(codes)
    expect(ITEMS).toEqual(before)
  })
})

const store = (value: string | null, fail = false) => {
  const kept = new Map<string, string>(value === null ? [] : [[PRECON_SORT_KEY, value]])
  return {
    kept,
    getItem: (key: string) => {
      if (fail) throw new Error('blocked')
      return kept.get(key) ?? null
    },
    setItem: (key: string, next: string) => {
      if (fail) throw new Error('blocked')
      kept.set(key, next)
    },
    removeItem: (key: string) => void kept.delete(key),
  }
}

describe('the kept order', () => {
  it.each<[string, string | null, boolean, PreconSort]>([
    ['reads a kept order', 'favorites', false, 'favorites'],
    ['falls back to newest with none', null, false, 'newest'],
    ['falls back to newest for nonsense', 'oldest', false, 'newest'],
    ['falls back to newest when the storage refuses', 'name', true, 'newest'],
  ])('%s', (_what, value, fail, sort) => {
    expect(loadPreconSort(store(value, fail))).toBe(sort)
  })

  it('saves the order and says when the browser refused', () => {
    const ok = store(null)
    expect(savePreconSort('name', ok)).toBe(true)
    expect(ok.kept.get(PRECON_SORT_KEY)).toBe('name')
    expect(savePreconSort('name', store(null, true))).toBe(false)
    expect(savePreconSort('name', null)).toBe(false)
  })
})
