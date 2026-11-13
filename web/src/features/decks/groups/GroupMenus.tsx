import {
  ArrowDownIcon,
  ArrowUpIcon,
  ChevronDownIcon,
  EllipsisIcon,
  FolderInputIcon,
  PencilIcon,
  Trash2Icon,
} from 'lucide-react'
import { Button } from '@/components/ui/button'
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuGroup,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuRadioGroup,
  DropdownMenuRadioItem,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu'
import { UNGROUPED } from './grouping'

/** The groups a deck can be moved to. */
export interface GroupChoice {
  id: string
  name: string
}

interface MoveToGroupProps {
  deckName: string
  groups: readonly GroupChoice[]
  /** The group the deck sits in, or null. */
  current: string | null
  onMove(group: string | null): void
}

/** "Group" on a deck's row: a menu of the groups with the deck's own checked, and Ungrouped. */
export function MoveToGroup({ deckName, groups, current, onMove }: MoveToGroupProps) {
  return (
    <DropdownMenu>
      <DropdownMenuTrigger
        render={<Button variant="outline" size="sm" aria-label={`Move ${deckName} to a group`} />}
      >
        <FolderInputIcon aria-hidden="true" />
        Group
        <ChevronDownIcon aria-hidden="true" />
      </DropdownMenuTrigger>
      <DropdownMenuContent align="end">
        <DropdownMenuGroup>
          <DropdownMenuLabel>Move to group</DropdownMenuLabel>
          <DropdownMenuRadioGroup
            value={current ?? UNGROUPED}
            onValueChange={(next: string) => onMove(next === UNGROUPED ? null : next)}
          >
            {groups.map((group) => (
              <DropdownMenuRadioItem key={group.id} value={group.id}>
                {group.name}
              </DropdownMenuRadioItem>
            ))}
            <DropdownMenuRadioItem value={UNGROUPED}>Ungrouped</DropdownMenuRadioItem>
          </DropdownMenuRadioGroup>
        </DropdownMenuGroup>
      </DropdownMenuContent>
    </DropdownMenu>
  )
}

interface GroupActionsProps {
  name: string
  first: boolean
  last: boolean
  onRename(): void
  onMove(by: number): void
  onDelete(): void
}

/** The actions of a group's heading: rename, move up or down, delete (its decks stay, ungrouped). */
export function GroupActions({ name, first, last, onRename, onMove, onDelete }: GroupActionsProps) {
  return (
    <DropdownMenu>
      <DropdownMenuTrigger
        render={<Button variant="ghost" size="icon-sm" aria-label={`Actions for the group ${name}`} />}
      >
        <EllipsisIcon aria-hidden="true" />
      </DropdownMenuTrigger>
      <DropdownMenuContent align="end">
        <DropdownMenuItem onClick={onRename}>
          <PencilIcon aria-hidden="true" />
          Rename
        </DropdownMenuItem>
        <DropdownMenuItem disabled={first} onClick={() => onMove(-1)}>
          <ArrowUpIcon aria-hidden="true" />
          Move up
        </DropdownMenuItem>
        <DropdownMenuItem disabled={last} onClick={() => onMove(1)}>
          <ArrowDownIcon aria-hidden="true" />
          Move down
        </DropdownMenuItem>
        <DropdownMenuItem onClick={onDelete}>
          <Trash2Icon aria-hidden="true" />
          Delete group
        </DropdownMenuItem>
      </DropdownMenuContent>
    </DropdownMenu>
  )
}
