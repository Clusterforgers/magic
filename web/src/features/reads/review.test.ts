// Goal: the review's pure rules hold: cards to read come first and a card to check that is also to read shows once, the next open card skips the done ones and wraps, progress counts every outcome, the count line adds what is left to check, each state has a tone and words, a review key is J, K, the arrows, H or ? outside a text field and without Ctrl, Alt or Meta, and the cards checked survive a reload and a broken storage.
import { afterEach, describe, expect, it, vi } from 'vitest'
import type { FlaggedCard } from '@/features/decks/unvalidated/model'
import type { ReadQueueCard } from '@/protocol/generated'
import {
  nextOpen,
  progressOf,
  reviewItems,
  reviewKey,
  reviewLine,
  rowState,
  saveChecked,
  savedChecked,
} from './review'
import type { Outcome } from './review'

const read = (id: string, state: ReadQueueCard['state'] = 'pending'): ReadQueueCard => ({
  oracle_id: id,
  name: `Read ${id}`,
  read_hash: `h-${id}`,
  state,
  differs: 0,
  rank: null,
  in_decks: true,
})

const check = (id: string, support: FlaggedCard['support'] = 'partial'): FlaggedCard => ({
  oracleId: id,
  name: `Check ${id}`,
  support,
  reasons: [],
  decks: [{ id: 'd1', name: 'Izzet' }],
})

afterEach(() => {
  vi.restoreAllMocks()
  window.localStorage.clear()
})

describe('the items', () => {
  it('put the cards to read first, then the cards to check, each card once', () => {
    const items = reviewItems(
      [read('a'), read('b'), read('a')],
      [check('c'), check('a'), check('d'), check('c')],
    )
    expect(items.map((item) => `${item.kind}:${item.id}`)).toEqual(['read:a', 'read:b', 'check:c', 'check:d'])
  })
})

describe('the next open card', () => {
  const items = reviewItems([read('a'), read('b')], [check('c'), check('d')])
  it.each<[string | null, Record<string, Outcome>, string | null]>([
    [null, {}, 'a'],
    ['a', {}, 'b'],
    ['a', { b: 'correct', c: 'checked' }, 'd'],
    ['d', { a: 'wrong' }, 'b'],
    ['b', { a: 'correct', b: 'correct', c: 'checked', d: 'checked' }, null],
    ['c', { a: 'correct', b: 'correct', d: 'checked' }, 'c'],
  ])('after %s with %j done is %s', (current, done, next) => {
    expect(nextOpen(items, new Map(Object.entries(done)), current)).toBe(next)
  })
})

describe('the progress', () => {
  it.each<[Record<string, Outcome>, number, string]>([
    [{}, 0, '0 of 3 reviewed'],
    [{ a: 'wrong' }, 1, '1 of 3 reviewed'],
    [{ a: 'correct', c: 'checked', gone: 'checked' }, 2, '2 of 3 reviewed'],
  ])('with %j done counts %i', (done, finished, text) => {
    const items = reviewItems([read('a'), read('b')], [check('c')])
    expect(progressOf(items, new Map(Object.entries(done)))).toEqual({ done: finished, total: 3, text })
  })
})

describe('the count line', () => {
  it.each([
    ['3 to read.', 0, '3 to read.'],
    ['3 to read.', 2, '3 to read, 2 to check.'],
    ['0 to read.', 1, '0 to read, 1 to check.'],
  ])('of %s and %i to check is %s', (line, checks, words) => {
    expect(reviewLine(line, checks)).toBe(words)
  })
})

describe('the state of a row', () => {
  const [toRead, toCheck] = reviewItems([read('a')], [check('c', 'manual')])
  it.each([
    ['a draft waiting', toRead, undefined, 'to_read', 'Unverified, waiting for a read'],
    ['a draft found right', toRead, 'correct', 'right', 'You said it reads right'],
    ['a draft found wrong', toRead, 'wrong', 'wrong', 'You said it reads wrong'],
    ['a card played by hand', toCheck, undefined, 'by_hand', 'Played by hand'],
    ['a card checked', toCheck, 'checked', 'checked', 'Checked'],
  ] as const)('of %s', (_case, item, outcome, tone, words) => {
    expect(item).toBeDefined()
    if (item === undefined) return
    expect(rowState(item, outcome)).toEqual({ tone, words })
  })
})

describe('the review keys', () => {
  const press = (
    key: string,
    extra: Partial<KeyboardEvent> = {},
    target: EventTarget | null = document.body,
  ) => reviewKey({ key, ctrlKey: false, metaKey: false, altKey: false, target, ...extra })
  it.each([
    ['j', 'next'],
    ['J', 'next'],
    ['ArrowRight', 'next'],
    ['k', 'previous'],
    ['ArrowLeft', 'previous'],
    ['h', 'by_hand'],
    ['?', 'help'],
    ['c', null],
    ['Enter', null],
  ])('reads %s as %s', (key, asked) => {
    expect(press(key)).toBe(asked)
  })
  it.each(['ctrlKey', 'metaKey', 'altKey'] as const)('ignores a key with %s', (modifier) => {
    expect(press('j', { [modifier]: true })).toBeNull()
  })
  it.each(['input', 'textarea', 'select'])('ignores a key typed in a %s', (tag) => {
    expect(press('j', {}, document.createElement(tag))).toBeNull()
  })
})

describe('the cards checked', () => {
  it('are kept per queue', () => {
    saveChecked('deck:d1', ['a', 'b'])
    expect(savedChecked('deck:d1')).toEqual(['a', 'b'])
    expect(savedChecked('mine')).toEqual([])
  })
  it('read as none from a storage that throws or holds something else', () => {
    window.localStorage.setItem('mtg.review-checked:mine', '{"a":1}')
    expect(savedChecked('mine')).toEqual([])
    vi.spyOn(Storage.prototype, 'getItem').mockImplementation(() => {
      throw new Error('blocked')
    })
    vi.spyOn(Storage.prototype, 'setItem').mockImplementation(() => {
      throw new Error('blocked')
    })
    expect(() => saveChecked('mine', ['a'])).not.toThrow()
    expect(savedChecked('mine')).toEqual([])
  })
})
