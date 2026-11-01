// Goal: the chooser's remembered order is read back only when it is one of the orders, and a storage that fails changes nothing.
import { describe, expect, it } from 'vitest'
import { loadSort, saveSort, SORT_KEY } from './deckSort'

const store = (initial: Record<string, string> = {}) => {
  const data = { ...initial }
  return {
    data,
    getItem: (key: string) => data[key] ?? null,
    setItem: (key: string, value: string) => {
      data[key] = value
    },
    removeItem: (key: string) => {
      delete data[key]
    },
  }
}
const broken = {
  getItem: () => {
    throw new Error('blocked')
  },
  setItem: () => {
    throw new Error('blocked')
  },
  removeItem: () => undefined,
}

describe('the remembered order', () => {
  it.each([
    ['name', 'name'],
    ['hand', 'hand'],
    ['default', 'default'],
    ['newest', 'default'],
    ['', 'default'],
  ])('reads %j as %s', (kept, sort) => {
    expect(loadSort(store({ [SORT_KEY]: kept }))).toBe(sort)
  })

  it('is the default order without storage or when it throws', () => {
    expect(loadSort(null)).toBe('default')
    expect(loadSort(broken)).toBe('default')
  })

  it('keeps a choice, and says no when the browser refuses', () => {
    const s = store()
    expect(saveSort('hand', s)).toBe(true)
    expect(s.data[SORT_KEY]).toBe('hand')
    expect(saveSort('name', null)).toBe(false)
    expect(saveSort('name', broken)).toBe(false)
  })
})
