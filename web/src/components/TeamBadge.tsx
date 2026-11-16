import { teamColour, teamLabel } from '@/lib/teams'
import { cn } from '@/lib/utils'

interface TeamBadgeProps {
  /** The team, counted from 0. */
  team: number
  /** `pill` for lists, `chip` for the tight rows of the table, `mini` for a seat tile: the dot and the number. */
  size?: 'pill' | 'chip' | 'mini'
  className?: string
}

/**
 * A team's label ("Team 1") in its colour, edged and with a dot; the words carry it all, the colour only groups. A
 * `mini` badge shows the number alone beside the dot and keeps the word for a reader and the tooltip.
 */
export function TeamBadge({ team, size = 'pill', className }: TeamBadgeProps) {
  const colour = teamColour(team)
  const label = teamLabel(team)
  return (
    <span
      data-slot="team-badge"
      data-team={team}
      title={size === 'mini' ? label : undefined}
      className={cn(
        'inline-flex shrink-0 items-center gap-1 rounded-full border font-medium whitespace-nowrap',
        size === 'pill' ? 'h-6 px-2 text-xs' : 'h-4 px-1 text-xs leading-4',
        colour.edge,
        colour.text,
        className,
      )}
    >
      <span aria-hidden="true" className={cn('size-2 shrink-0 rounded-full', colour.dot)} />
      {size === 'mini' ? (
        <>
          <span className="sr-only">Team </span>
          {team + 1}
        </>
      ) : (
        label
      )}
    </span>
  )
}
