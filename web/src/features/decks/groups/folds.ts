/** Which groups of a deck picker are open: the one opened first, and the open state while searching. */
import { useMemo, useState } from 'react'
import { groupOf } from './grouping'
import type { GroupLike } from './grouping'
import { lastGroup } from './lastGroup'

/** The section a picker opens first: the group of the chosen deck, else the last used group, else the first one. */
export function firstOpen(
  keys: readonly string[],
  groups: readonly GroupLike[],
  chosenId: string | null,
): string | null {
  const wanted = (chosenId === null ? null : groupOf(groups, chosenId)) ?? lastGroup()
  return keys.find((key) => key === wanted) ?? keys[0] ?? null
}

/** The open groups of a picker: `initial` until the player opens or closes one; every group while searching. */
export function useFolds(initial: ReadonlySet<string>, searching: boolean) {
  const [opened, setOpened] = useState<ReadonlySet<string> | null>(null)
  const open = opened ?? initial
  return useMemo(
    () => ({
      isOpen: (key: string): boolean => searching || open.has(key),
      toggle: (key: string): void => {
        const next = new Set(open)
        if (next.has(key)) next.delete(key)
        else next.add(key)
        setOpened(next)
      },
    }),
    [open, searching],
  )
}
