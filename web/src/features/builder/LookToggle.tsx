import { LayoutGridIcon, ListIcon } from 'lucide-react'
import { Button } from '@/components/ui/button'
import type { DeckLook } from './deckLook'

/** The choice of how the deck pane draws its cards: large pictures in groups by type, or the dense text list. */
export function LookToggle({ look, onChange }: { look: DeckLook; onChange: (look: DeckLook) => void }) {
  return (
    <div role="group" aria-label="Show cards as" className="inline-flex gap-1 rounded-lg bg-muted p-1">
      <Button
        type="button"
        size="sm"
        variant={look === 'pictures' ? 'default' : 'ghost'}
        aria-pressed={look === 'pictures'}
        onClick={() => onChange('pictures')}
      >
        <LayoutGridIcon aria-hidden="true" />
        Pictures
      </Button>
      <Button
        type="button"
        size="sm"
        variant={look === 'list' ? 'default' : 'ghost'}
        aria-pressed={look === 'list'}
        onClick={() => onChange('list')}
      >
        <ListIcon aria-hidden="true" />
        List
      </Button>
    </div>
  )
}
