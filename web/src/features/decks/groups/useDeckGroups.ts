import { useCallback, useEffect, useMemo, useState } from 'react'
import { describeDeckError } from '@/features/decks/errors'
import type { DeckGroupsApi } from '@/net/deckGroups'
import type { DeckGroupView } from '@/protocol/generated'
import { filed, moved } from './grouping'

/** The account's deck groups and the changes to them. */
export interface DeckGroups {
  /** False until the list arrives, and for good when it cannot be had (the decks then show in one list). */
  ready: boolean
  /** The groups in the owner's order. */
  groups: readonly DeckGroupView[]
  /** Why the last change failed, for the page to announce; null otherwise. */
  problem: string | null
  create(names: readonly string[]): void
  rename(id: string, name: string): void
  setCollapsed(id: string, collapsed: boolean): void
  remove(id: string): void
  /** Moves the group `by` places in the order. */
  move(id: string, by: number): void
  /** Files the deck in the group, or in none for null. */
  file(deckId: string, group: string | null): void
}

const NONE: readonly DeckGroupView[] = []

/**
 * The groups of the signed-in account (`GET /api/deck-groups`), loaded when `enabled`. Every change shows at once; a
 * refused one says why in `problem` and the list is read again, so the page shows what the server keeps.
 */
export function useDeckGroups(api: DeckGroupsApi, enabled = true): DeckGroups {
  const [groups, setGroups] = useState<readonly DeckGroupView[] | null>(null)
  const [problem, setProblem] = useState<string | null>(null)
  const [round, setRound] = useState(0)

  useEffect(() => {
    if (!enabled) return
    let stale = false
    api.list().then(
      (list) => {
        if (!stale) setGroups(list)
      },
      () => {
        // Groups are a convenience: without them every deck shows in one list.
      },
    )
    return () => {
      stale = true
    }
  }, [api, enabled, round])

  const run = useCallback(
    (change: (was: readonly DeckGroupView[]) => readonly DeckGroupView[], send: () => Promise<unknown>) => {
      setGroups((was) => (was === null ? was : change(was)))
      send().then(
        () => setProblem(null),
        (error: unknown) => {
          setProblem(describeDeckError(error))
          setRound((n) => n + 1)
        },
      )
    },
    [],
  )

  const current = groups ?? NONE
  const create = useCallback(
    (names: readonly string[]) => {
      // One after another, so the groups keep the order they were asked in.
      void names
        .reduce<Promise<void>>(
          (before, name) =>
            before.then(() =>
              api.create(name).then((group) => setGroups((was) => [...(was ?? NONE), group])),
            ),
          Promise.resolve(),
        )
        .then(
          () => setProblem(null),
          (error: unknown) => setProblem(describeDeckError(error)),
        )
    },
    [api],
  )
  const rename = useCallback(
    (id: string, name: string) =>
      run(
        (was) => was.map((group) => (group.id === id ? { ...group, name } : group)),
        () => api.change(id, { name }),
      ),
    [api, run],
  )
  const setCollapsed = useCallback(
    (id: string, collapsed: boolean) =>
      run(
        (was) => was.map((group) => (group.id === id ? { ...group, collapsed } : group)),
        () => api.change(id, { collapsed }),
      ),
    [api, run],
  )
  const remove = useCallback(
    (id: string) =>
      run(
        (was) => was.filter((group) => group.id !== id),
        () => api.remove(id),
      ),
    [api, run],
  )
  const move = useCallback(
    (id: string, by: number) => {
      const next = moved(current, id, by)
      run(
        () => next,
        () => api.reorder(next.map((group) => group.id)),
      )
    },
    [api, run, current],
  )
  const file = useCallback(
    (deckId: string, group: string | null) =>
      run(
        (was) => filed(was, deckId, group),
        () => api.file(deckId, group),
      ),
    [api, run],
  )

  return useMemo(
    () => ({
      ready: enabled && groups !== null,
      groups: enabled ? current : NONE,
      problem,
      create,
      rename,
      setCollapsed,
      remove,
      move,
      file,
    }),
    [enabled, groups, current, problem, create, rename, setCollapsed, remove, move, file],
  )
}
