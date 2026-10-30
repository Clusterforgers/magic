// Goal: the names of objects that left the view are kept for the log lines that tell of them: a snapshot's name table and the batches that take objects from the view add them, the newest objects stay when there are too many, and a page without any keeps the one object it holds.
import { describe, expect, it } from 'vitest'
import type { LogName, ObjectView } from '@/protocol/generated'
import { batchOf, must } from '@/test-utils/batches'
import { startSession } from '@/test-utils/harness'
import { lateJoin } from '@/test-utils/transcripts'
import { LOG_NAMES_LIMIT, mergeLogNames, rememberDeparted } from './rememberNames'

const snapshot = must(
  lateJoin.messages.find((message) => message.type === 'Snapshot'),
  'snapshot',
)
const base = snapshot.state

const named = (names: Record<number, string>): LogName[] =>
  Object.entries(names).map(([object, name]) => ({ object: Number(object), name }))

const held = (...ids: number[]): Record<number, ObjectView> =>
  Object.fromEntries(ids.map((id) => [id, { ...must(base.objects[0], 'an object'), id, name: `Card ${id}` }]))

describe('the names a snapshot brings', () => {
  it.each([
    ['nothing given', {}, undefined, {}],
    ['an empty table', { 3: 'Old' }, [], { 3: 'Old' }],
    ['new names added', { 3: 'Old' }, named({ 4: 'Goblin token' }), { 3: 'Old', 4: 'Goblin token' }],
    ['a name given again replaces the old one', { 3: 'Old' }, named({ 3: 'New' }), { 3: 'New' }],
  ])('%s', (_what, was, given, expected) => {
    expect(mergeLogNames(was, given)).toEqual(expected)
  })

  it('hands back the very same record when it brings none, so nothing re-renders', () => {
    const was = { 3: 'Old' }
    expect(mergeLogNames(was, [])).toBe(was)
    expect(mergeLogNames(was)).toBe(was)
  })

  it('keeps the newest objects when there are more than the limit', () => {
    const many = named(
      Object.fromEntries(Array.from({ length: LOG_NAMES_LIMIT + 5 }, (_, i) => [i, `C${i}`])),
    )
    const kept = mergeLogNames({}, many)
    expect(Object.keys(kept)).toHaveLength(LOG_NAMES_LIMIT)
    expect(kept[0]).toBeUndefined()
    expect(kept[LOG_NAMES_LIMIT + 4]).toBe(`C${LOG_NAMES_LIMIT + 4}`)
  })
})

describe('the names a batch takes from the view', () => {
  it.each([
    ['an object that went', {}, [5], { 5: 'Card 5' }],
    ['an object the page never held', {}, [9], {}],
    ['names already kept stay', { 1: 'Kept' }, [5], { 1: 'Kept', 5: 'Card 5' }],
    ['nothing went', { 1: 'Kept' }, [], { 1: 'Kept' }],
  ])('remember %s', (_what, was, gone, expected) => {
    expect(rememberDeparted(was, held(5), gone)).toEqual(expected)
  })

  it('hands back the very same record when nothing is added', () => {
    const was = { 5: 'Card 5' }
    expect(rememberDeparted(was, held(5), [5])).toBe(was)
    expect(rememberDeparted(was, held(5), [])).toBe(was)
  })

  it('leaves out an object the page saw with no name (face down)', () => {
    const before = { 5: { ...must(base.objects[0], 'an object'), id: 5, name: '' } }
    expect(rememberDeparted({}, before, [5])).toEqual({})
  })
})

describe('the session', () => {
  const started = () => {
    const h = startSession()
    h.join()
    return h
  }

  it('starts with no names and takes the table of every snapshot', () => {
    const h = started()
    expect(h.state().logNames).toEqual({})
    h.deliver({ ...snapshot, names: named({ 70: 'Goblin token' }) })
    expect(h.state().logNames).toEqual({ 70: 'Goblin token' })
    h.deliver({ ...snapshot, names: named({ 71: 'Elf' }) })
    expect(h.state().logNames).toEqual({ 70: 'Goblin token', 71: 'Elf' })
  })

  it('keeps what it knows when a snapshot brings no names', () => {
    const h = started()
    h.deliver({ ...snapshot, names: named({ 70: 'Goblin token' }) })
    const { names: _names, ...bare } = snapshot
    h.deliver(bare)
    expect(h.state().logNames).toEqual({ 70: 'Goblin token' })
  })

  it('remembers the name of an object a batch takes from the view', () => {
    const h = started()
    h.deliver(snapshot)
    const gone = must(base.objects[0], 'an object')
    const now = batchOf(base, { from_seq: snapshot.seq, gone: [gone.id] })
    h.deliver({ type: 'Events', ...now })
    expect(h.state().objectsById[gone.id]).toBeUndefined()
    expect(h.state().logNames).toEqual({ [gone.id]: gone.name })
  })
})
