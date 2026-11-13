/** An in-memory deck-group server for tests: it keeps the groups and records every call. */
import { DeckError } from '@/net/decks'
import type { DeckGroupsApi } from '@/net/deckGroups'
import type { DeckGroupView } from '@/protocol/generated'
import { filed } from './grouping'

/** The fake's groups and the calls it saw, as "method args". */
export interface FakeGroups {
  api: DeckGroupsApi
  readonly groups: readonly DeckGroupView[]
  calls: string[]
}

/** A group with `decks`, open unless said otherwise. */
export const groupView = (
  id: string,
  name: string,
  decks: string[] = [],
  collapsed = false,
): DeckGroupView => ({
  id,
  name,
  collapsed,
  decks,
})

/** Serves `initial`; `failing` makes every call reject with that code. */
export function startGroups(initial: DeckGroupView[] = [], failing?: 'group_limit' | 'network'): FakeGroups {
  let held = initial.map((group) => ({ ...group, decks: [...group.decks] }))
  const calls: string[] = []
  let made = 0
  const answer = <T>(call: string, value: () => T): Promise<T> => {
    calls.push(call)
    if (failing !== undefined)
      return Promise.reject(new DeckError(failing, failing === 'network' ? 0 : 409, failing))
    return Promise.resolve(value())
  }
  return {
    calls,
    get groups() {
      return held
    },
    api: {
      list: () => answer('list', () => held.map((group) => ({ ...group }))),
      create: (name) =>
        answer(`create ${name}`, () => {
          made += 1
          const group = groupView(`new-${made}`, name)
          held = [...held, group]
          return group
        }),
      change: (id, change) =>
        answer(`change ${id} ${JSON.stringify(change)}`, () => {
          held = held.map((group) => (group.id === id ? { ...group, ...change } : group))
          const found = held.find((group) => group.id === id)
          if (found === undefined) throw new DeckError('not_found', 404, 'no group')
          return found
        }),
      remove: (id) =>
        answer(`remove ${id}`, () => {
          held = held.filter((group) => group.id !== id)
        }),
      reorder: (ids) =>
        answer(`reorder ${ids.join(',')}`, () => {
          held = ids.flatMap((id) => held.filter((group) => group.id === id))
        }),
      file: (deckId, group) =>
        answer(`file ${deckId} ${group ?? 'none'}`, () => {
          held = filed(held, deckId, group)
        }),
    },
  }
}
