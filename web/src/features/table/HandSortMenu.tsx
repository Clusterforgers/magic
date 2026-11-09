import { ArrowDownUp } from 'lucide-react'
import { Button } from '@/components/ui/button'
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuGroup,
  DropdownMenuLabel,
  DropdownMenuRadioGroup,
  DropdownMenuRadioItem,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu'
import { HAND_SORTS, isHandSort } from './handOrder'
import type { HandOrderMode, HandSort } from './handOrder'
import { modeLabel } from './useHandOrder'

/**
 * The sort control of the hand dock: an icon button naming the order in force, opening the auto sorts. A dragged
 * order is "Custom" and checks none of them.
 */
export function HandSortMenu({
  mode,
  onChoose,
}: {
  mode: HandOrderMode
  onChoose: (sort: HandSort) => void
}) {
  return (
    <DropdownMenu>
      <DropdownMenuTrigger
        render={
          <Button
            variant="ghost"
            size="icon-sm"
            data-hand-sort={mode}
            aria-label={`Sort hand: ${modeLabel(mode)}`}
            title={`Sort hand: ${modeLabel(mode)}`}
          />
        }
      >
        <ArrowDownUp aria-hidden="true" />
      </DropdownMenuTrigger>
      <DropdownMenuContent align="start" side="top">
        <DropdownMenuGroup>
          <DropdownMenuLabel>Sort hand</DropdownMenuLabel>
          <DropdownMenuRadioGroup
            value={mode}
            onValueChange={(value) => {
              if (isHandSort(value)) onChoose(value)
            }}
          >
            {HAND_SORTS.map(({ value, label }) => (
              <DropdownMenuRadioItem key={value} value={value} closeOnClick>
                {label}
              </DropdownMenuRadioItem>
            ))}
          </DropdownMenuRadioGroup>
        </DropdownMenuGroup>
      </DropdownMenuContent>
    </DropdownMenu>
  )
}
