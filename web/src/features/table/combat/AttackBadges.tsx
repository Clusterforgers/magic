import { Swords } from 'lucide-react'
import type { CSSProperties } from 'react'
import { useContext } from 'react'
import { usePlay } from '@/features/game/gameContext'
import { cn } from '@/lib/utils'
import type { PlayerId } from '@/protocol/generated'
import { useSeatNamer } from '../names'
import { seatHue } from '../podFigures'
import { ATTACK_BADGES_BY_DEFAULT, TableOptionsContext } from '../tableOptions'
import { useSeatAttacks } from './attacked'
import type { AttackGroup } from './plan'

/** What a badge says to a screen reader and in its tooltip: "Bo attacks you with 3 creatures: Grizzly Bears, …". */
export function attackWords(group: AttackGroup, from: string, names: readonly string[], at?: string): string {
  const count = group.attackers.length
  const what = `${count} creature${count === 1 ? '' : 's'}`
  const verb = group.pending ? 'would attack' : 'attacks'
  return `${from} ${verb}${at === undefined ? '' : ` ${at}`} with ${what}: ${names.join(', ')}`
}

/**
 * The attacks at a seat as badges on its pod or tile, one per attacking seat in that seat's colour ("⚔ 3"), dashed
 * while the attack is still being chosen. They stand in for lines that would cross the table from a board to a pod
 * (boss mode draws the lines as well); nothing is drawn while nobody attacks the seat. On the viewer's own pod they
 * name "you" for a screen reader.
 */
export function AttackBadges({ seat }: { seat: PlayerId }) {
  const on = useContext(TableOptionsContext).attackBadges ?? ATTACK_BADGES_BY_DEFAULT
  return on ? <Badges seat={seat} /> : null
}

function Badges({ seat }: { seat: PlayerId }) {
  const { groups, cardName } = useSeatAttacks(seat)
  const name = useSeatNamer()
  const viewer = usePlay((state) => (state.view?.spectator === true ? null : state.seat))
  if (groups.length === 0) return null
  // On another seat's pod the badge needs no defender; on the viewer's own it says who is attacked.
  const at = seat === viewer ? 'you' : undefined
  return (
    <>
      {groups.map((group) => {
        const words = attackWords(
          group,
          group.from === undefined ? 'A player' : name(group.from),
          group.attackers.map(cardName),
          at,
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
