import { teamColour, teamLabel } from '@/lib/teams'
import { cn } from '@/lib/utils'

interface TeamBadgeProps {
  /** The team, counted from 0. */
  team: number
  /** `pill` for lists, `chip` for the tight rows of the table. */
  size?: 'pill' | 'chip'
  className?: string
}

/** A team's label ("Team 1") in its colour, edged and with a dot; the words carry it all, the colour only groups. */
export function TeamBadge({ team, size = 'pill', className }: TeamBadgeProps) {
  const colour = teamColour(team)
  return (
    <span
      data-slot="team-badge"
      data-team={team}
      className={cn(
        'inline-flex shrink-0 items-center gap-1 rounded-full border font-medium whitespace-nowrap',
        size === 'pill' ? 'h-6 px-2 text-xs' : 'h-4 px-1 text-xs leading-4',
        colour.edge,
        colour.text,
        className,
      )}
    >
      <span aria-hidden="true" className={cn('size-2 shrink-0 rounded-full', colour.dot)} />
      {teamLabel(team)}
    </span>
  )
}
