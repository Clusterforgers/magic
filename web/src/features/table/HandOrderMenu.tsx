/**
 * The hand order part of the table menu: the auto sort (as drawn, mana value, by type) and, under "By type", the order
 * of the type groups, moved up and down with buttons. Both are the account's and hold in every game; dragging a card
 * in the dock still makes a custom order for the game, which picking a sort here drops.
 */
import { ArrowDown, ArrowUp } from 'lucide-react'
import { Button } from '@/components/ui/button'
import { useGame } from '@/features/game/gameContext'
import { HAND_GROUPS, HAND_SORTS, moveGroup } from './handOrder'
import { useHandMode } from './useHandOrder'

function groupLabel(value: string): string {
  return HAND_GROUPS.find((group) => group.value === value)?.label ?? value
}

/** The sort buttons and, while "By type" holds, the ordered list of the groups. */
export function HandOrderMenu() {
  const hand = useHandMode(useGame().game)
  const groups = [...hand.groups]
  return (
    <div className="grid gap-2">
      <div className="inline-flex w-fit flex-wrap gap-0.5 rounded-lg bg-muted p-0.5">
        {HAND_SORTS.map(({ value, label }) => (
          <Button
            key={value}
            size="sm"
            variant={hand.mode === value ? 'default' : 'ghost'}
            aria-pressed={hand.mode === value}
            onClick={() => hand.choose(value)}
          >
            {label}
          </Button>
        ))}
      </div>
      {hand.mode === 'custom' ? (
        <p className="text-xs text-muted-foreground">
          Your dragged order holds in this game; pick a sort to drop it.
        </p>
      ) : null}
      {hand.mode === 'Groups' ? (
        <ol aria-label="Order of the type groups" className="grid grid-cols-2 gap-x-3 gap-y-1">
          {groups.map((group, at) => {
            const label = groupLabel(group)
            return (
              <li key={group} data-group={group} className="flex min-w-0 items-center gap-1 text-sm">
                <span
                  aria-hidden="true"
                  className="w-4 text-right text-xs text-muted-foreground tabular-nums"
                >
                  {at + 1}
                </span>
                <span className="min-w-0 flex-1 truncate">{label}</span>
                <Button
                  size="icon-xs"
                  variant="ghost"
                  aria-label={`Move ${label} up`}
                  disabled={at === 0}
                  onClick={() => hand.order(moveGroup(groups, at, at - 1))}
                >
                  <ArrowUp aria-hidden="true" />
                </Button>
                <Button
                  size="icon-xs"
                  variant="ghost"
                  aria-label={`Move ${label} down`}
                  disabled={at === groups.length - 1}
                  onClick={() => hand.order(moveGroup(groups, at, at + 1))}
                >
                  <ArrowDown aria-hidden="true" />
                </Button>
              </li>
            )
          })}
        </ol>
      ) : null}
    </div>
  )
}
