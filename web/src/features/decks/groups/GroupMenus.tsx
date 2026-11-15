import {
  ArrowDownIcon,
  ArrowUpIcon,
  ChevronDownIcon,
  EllipsisIcon,
  FolderInputIcon,
  FolderPlusIcon,
  PencilIcon,
  Trash2Icon,
} from 'lucide-react'
import { Button } from '@/components/ui/button'
import {
  DropdownMenu,
  DropdownMenuCheckboxItem,
  DropdownMenuContent,
  DropdownMenuGroup,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu'

/** A group a deck can join. */
export interface GroupChoice {
  id: string
  name: string
}

interface DeckGroupsMenuProps {
  deckName: string
  groups: readonly GroupChoice[]
  /** The ids of the groups the deck sits in. */
  memberOf: readonly string[]
  /** The deck joins the group (`member`) or leaves it; its other groups stay. */
  onToggle(group: string, member: boolean): void
  /** Asks for a new group the deck joins. */
  onNewGroup(): void
}

/**
 * "Groups" on a deck's row: a checkbox for each group the deck sits in (a deck in none is ungrouped), then "New
 * group…". A toggle leaves the menu open; it closes only when the row moves to another section.
 */
export function DeckGroupsMenu({ deckName, groups, memberOf, onToggle, onNewGroup }: DeckGroupsMenuProps) {
  return (
    <DropdownMenu>
      <DropdownMenuTrigger
        render={<Button variant="outline" size="sm" aria-label={`Groups of ${deckName}`} />}
      >
        <FolderInputIcon aria-hidden="true" />
        Groups
        <ChevronDownIcon aria-hidden="true" />
      </DropdownMenuTrigger>
      <DropdownMenuContent align="end">
        {groups.length === 0 ? null : (
          <>
            <DropdownMenuGroup>
              <DropdownMenuLabel>Groups</DropdownMenuLabel>
              {groups.map((group) => (
                <DropdownMenuCheckboxItem
                  key={group.id}
                  checked={memberOf.includes(group.id)}
                  onCheckedChange={(checked: boolean) => onToggle(group.id, checked)}
                >
                  {group.name}
                </DropdownMenuCheckboxItem>
              ))}
            </DropdownMenuGroup>
            <DropdownMenuSeparator />
          </>
        )}
        <DropdownMenuItem onClick={onNewGroup}>
          <FolderPlusIcon aria-hidden="true" className="size-4" />
          New group…
        </DropdownMenuItem>
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
