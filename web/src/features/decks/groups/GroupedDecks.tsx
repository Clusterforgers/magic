import { ChevronRightIcon } from 'lucide-react'
import { useId, useState } from 'react'
import type { DragEvent, ReactNode } from 'react'
import { cn } from '@/lib/utils'
import type { DeckGroupView, DeckListItem } from '@/protocol/generated'
import { DECK_DRAG_TYPE } from './drag'
import { GroupActions } from './GroupMenus'
import { sections } from './grouping'
import type { Section } from './grouping'
import type { DeckGroups } from './useDeckGroups'

interface GroupedDecksProps {
  decks: readonly DeckListItem[]
  groups: DeckGroups
  renderRow(deck: DeckListItem, groupId: string | null): ReactNode
  onRename(group: DeckGroupView): void
  /** Says what a change did, for the page's live region. */
  onAnnounce(text: string): void
}

const count = (n: number): string => `${n} ${n === 1 ? 'deck' : 'decks'}`

function GroupSection({
  section,
  collapsed,
  first,
  last,
  onToggle,
  onDropDeck,
  onRename,
  onMove,
  onDelete,
  children,
}: {
  section: Section<DeckListItem>
  collapsed: boolean
  first: boolean
  last: boolean
  onToggle(): void
  onDropDeck(deckId: string): void
  onRename(): void
  onMove(by: number): void
  onDelete(): void
  children: ReactNode
}) {
  const id = useId()
  const [over, setOver] = useState(false)
  const name = section.group?.name ?? 'Ungrouped'
  const carriesDeck = (event: DragEvent): boolean => event.dataTransfer.types.includes(DECK_DRAG_TYPE)
  return (
    <section
      aria-labelledby={`${id}-heading`}
      data-group={section.key}
      onDragOver={(event) => {
        if (!carriesDeck(event)) return
        event.preventDefault()
        event.dataTransfer.dropEffect = 'move'
        setOver(true)
      }}
      onDragLeave={() => setOver(false)}
      onDrop={(event) => {
        setOver(false)
        const deckId = event.dataTransfer.getData(DECK_DRAG_TYPE)
        if (deckId === '') return
        event.preventDefault()
        onDropDeck(deckId)
      }}
      className={cn('grid gap-2 rounded-xl', over && 'bg-accent/40 ring-2 ring-primary')}
    >
      <div className="flex min-w-0 items-center gap-1">
        <h2 id={`${id}-heading`} className="min-w-0 flex-1">
          <button
            type="button"
            aria-expanded={!collapsed}
            aria-controls={`${id}-decks`}
            onClick={onToggle}
            className="flex min-h-9 w-full min-w-0 items-center gap-2 rounded-md px-1 text-left text-base font-semibold hover:bg-accent/50 focus-visible:ring-2 focus-visible:ring-ring focus-visible:outline-none pointer-coarse:min-h-11"
          >
            <ChevronRightIcon
              aria-hidden="true"
              className={cn('size-4 shrink-0 transition-transform', collapsed ? '' : 'rotate-90')}
            />
            <span className="min-w-0 truncate">{name}</span>
            <span className="shrink-0 text-sm font-normal text-muted-foreground">
              {count(section.items.length)}
            </span>
          </button>
        </h2>
        {section.group === null ? null : (
          <GroupActions
            name={name}
            first={first}
            last={last}
            onRename={onRename}
            onMove={onMove}
            onDelete={onDelete}
          />
        )}
      </div>
      {collapsed ? null : (
        <div id={`${id}-decks`}>
          {section.items.length === 0 ? (
            <p className="rounded-lg border border-dashed px-4 py-3 text-sm text-muted-foreground">
              No decks here. Drag a deck onto this group, or use its Group menu.
            </p>
          ) : (
            <ul aria-label={`Decks in ${name}`} className="grid gap-3">
              {children}
            </ul>
          )}
        </div>
      )}
    </section>
  )
}

/**
 * The decks page in groups: each group a collapsible section (the collapsed state is kept on the server), in the
 * owner's order, then the decks in no group. A deck is dropped onto a group, or moved with its row's Group menu.
 */
export function GroupedDecks({ decks, groups, renderRow, onRename, onAnnounce }: GroupedDecksProps) {
  const [looseClosed, setLooseClosed] = useState(false)
  const all = sections(decks, groups.groups, true).filter(
    (section) => section.group !== null || section.items.length > 0,
  )
  const nameOf = (group: string | null): string =>
    groups.groups.find((entry) => entry.id === group)?.name ?? 'Ungrouped'
  const drop = (deckId: string, target: string | null): void => {
    const deck = decks.find((entry) => entry.id === deckId)
    if (deck === undefined) return
    groups.file(deckId, target)
    onAnnounce(`Moved “${deck.name}” to ${nameOf(target)}.`)
  }
  const real = groups.groups.length
  return (
    <div data-slot="deck-groups" className="grid gap-6">
      {all.map((section, index) => {
        const group = section.group
        const collapsed = group === null ? looseClosed : (groups.groups[index]?.collapsed ?? false)
        return (
          <GroupSection
            key={section.key}
            section={section}
            collapsed={collapsed}
            first={index === 0}
            last={index === real - 1}
            onToggle={() =>
              group === null ? setLooseClosed((was) => !was) : groups.setCollapsed(group.id, !collapsed)
            }
            onDropDeck={(deckId) => drop(deckId, group?.id ?? null)}
            onRename={() => {
              const full = groups.groups.find((entry) => entry.id === group?.id)
              if (full !== undefined) onRename(full)
            }}
            onMove={(by) => {
              if (group !== null) groups.move(group.id, by)
            }}
            onDelete={() => {
              if (group === null) return
              groups.remove(group.id)
              onAnnounce(`Deleted the group “${group.name}”. Its decks are ungrouped.`)
            }}
          >
            {section.items.map((deck) => renderRow(deck, group?.id ?? null))}
          </GroupSection>
        )
      })}
    </div>
  )
}
