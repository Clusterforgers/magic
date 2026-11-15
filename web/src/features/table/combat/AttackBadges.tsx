import { Swords } from 'lucide-react'
import type { CSSProperties } from 'react'
import { useContext, useMemo } from 'react'
import { useShallow } from 'zustand/react/shallow'
import { usePlay } from '@/features/game/gameContext'
import { cn } from '@/lib/utils'
import type { ObjectId, PlayerId } from '@/protocol/generated'
import { useSeatNamer } from '../names'
import { seatHue } from '../podFigures'
import { ATTACK_BADGES_BY_DEFAULT, TableOptionsContext } from '../tableOptions'
import { usePendingCombat } from './pending'
import { attackGroups } from './plan'
import type { AttackGroup } from './plan'

/** What a badge says to a screen reader and in its tooltip: "Bo attacks with 3 creatures: Grizzly Bears, …". */
export function attackWords(group: AttackGroup, from: string, names: readonly string[]): string {
  const count = group.attackers.length
  const what = `${count} creature${count === 1 ? '' : 's'}`
  const verb = group.pending ? 'would attack' : 'attacks'
  return `${from} ${verb} with ${what}: ${names.join(', ')}`
}

/**
 * The attacks at a seat as badges on its pod or tile, one per attacking seat in that seat's colour ("⚔ 3"), dashed
 * while the attack is still being chosen. They stand in for lines that would cross the table from a board to a pod;
 * nothing is drawn while nobody attacks the seat.
 */
export function AttackBadges({ seat }: { seat: PlayerId }) {
  const on = useContext(TableOptionsContext).attackBadges ?? ATTACK_BADGES_BY_DEFAULT
  return on ? <Badges seat={seat} /> : null
}

function Badges({ seat }: { seat: PlayerId }) {
  const combat = usePlay((state) => state.view?.combat ?? null)
  const pending = usePendingCombat()
  const attackers = useMemo(() => {
    const ids: ObjectId[] = []
    for (const attacker of combat?.attackers ?? []) ids.push(attacker.object)
    for (const [attacker] of pending.attacks) ids.push(attacker)
    return ids
  }, [combat, pending])
  const facts = usePlay(
    useShallow((state) =>
      attackers.flatMap((id) => [state.objectsById[id]?.controller ?? -1, state.objectsById[id]?.name ?? '']),
    ),
  )
  const name = useSeatNamer()
  const groups = useMemo(() => {
    const controllerOf = (id: ObjectId): PlayerId | undefined => {
      const seen = facts[attackers.indexOf(id) * 2]
      return typeof seen === 'number' && seen >= 0 ? seen : undefined
    }
    return attackGroups(combat, pending, controllerOf).get(seat) ?? []
  }, [combat, pending, facts, attackers, seat])
  if (groups.length === 0) return null
  const cardName = (id: ObjectId): string => String(facts[attackers.indexOf(id) * 2 + 1] ?? '')
  return (
    <>
      {groups.map((group) => {
        const words = attackWords(
          group,
          group.from === undefined ? 'A player' : name(group.from),
          group.attackers.map(cardName),
        )
        return (
          <span
            key={`${group.from ?? 'unknown'}:${group.pending}`}
            data-slot="attack-badge"
            data-from={group.from}
            data-pending={group.pending || undefined}
            role="img"
            aria-label={words}
            title={words}
            style={
              group.from === undefined ? undefined : ({ '--seat-hue': seatHue(group.from) } as CSSProperties)
            }
            className={cn(
              'inline-flex h-6 shrink-0 items-center gap-1 rounded-full border-2 bg-background px-1.5 text-xs font-semibold tabular-nums',
              'border-[oklch(0.6_0.14_var(--seat-hue,78))] text-[oklch(0.42_0.12_var(--seat-hue,78))] dark:border-[oklch(0.74_0.13_var(--seat-hue,78))] dark:text-[oklch(0.88_0.09_var(--seat-hue,78))]',
              group.pending && 'border-dashed',
            )}
          >
            <Swords aria-hidden="true" className="size-3.5" />
            {group.attackers.length}
          </span>
        )
      })}
    </>
  )
}
