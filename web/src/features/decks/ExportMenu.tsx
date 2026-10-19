import { ChevronDownIcon, DownloadIcon } from 'lucide-react'
import { Button } from '@/components/ui/button'
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuGroup,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu'
import type { DecksApi, ExportFormat } from '@/net/decks'
import type { DeckListItem } from '@/protocol/generated'

const OPTIONS: ReadonlyArray<{ format: ExportFormat; label: string }> = [
  { format: 'text', label: 'Plain text' },
  { format: 'arena', label: 'MTG Arena' },
  { format: 'mtgo', label: 'MTG Online' },
]

/**
 * "Export" on a deck's row: a menu of the three file kinds the server writes. Each choice is a link to the endpoint,
 * so the browser saves the answer under the file name the server gives it.
 */
export function ExportMenu({ deck, api }: { deck: Pick<DeckListItem, 'id' | 'name'>; api: DecksApi }) {
  return (
    <DropdownMenu>
      <DropdownMenuTrigger render={<Button variant="outline" size="sm" aria-label={`Export ${deck.name}`} />}>
        <DownloadIcon aria-hidden="true" />
        Export
        <ChevronDownIcon aria-hidden="true" />
      </DropdownMenuTrigger>
      <DropdownMenuContent align="end">
        <DropdownMenuGroup>
          <DropdownMenuLabel>Download as</DropdownMenuLabel>
          {OPTIONS.map(({ format, label }) => (
            <DropdownMenuItem
              key={format}
              nativeButton={false}
              render={<a href={api.exportUrl(deck.id, format)} download />}
            >
              {label}
            </DropdownMenuItem>
          ))}
        </DropdownMenuGroup>
      </DropdownMenuContent>
    </DropdownMenu>
  )
}
