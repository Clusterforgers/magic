import { Button } from '@/components/ui/button'
import type { TableMode } from './layout'

interface TableToolbarProps {
  mode: TableMode
  onMode: (mode: TableMode) => void
  /** True while an opponent has been picked by hand, so "Follow the turn" has something to undo. */
  pinnedFocus: boolean
  onFollow: () => void
}

/** The controls over the table: grid or focus layout, and returning the focus to the player whose turn it is. */
export function TableToolbar({ mode, onMode, pinnedFocus, onFollow }: TableToolbarProps) {
  return (
    <div role="toolbar" aria-label="Table layout" className="flex flex-wrap items-center gap-2">
      <div role="group" aria-label="Layout" className="inline-flex gap-0.5 rounded-lg bg-muted p-0.5">
        <Button
          size="sm"
          variant={mode === 'grid' ? 'default' : 'ghost'}
          aria-pressed={mode === 'grid'}
          onClick={() => onMode('grid')}
        >
          Grid
        </Button>
        <Button
          size="sm"
          variant={mode === 'focus' ? 'default' : 'ghost'}
          aria-pressed={mode === 'focus'}
          onClick={() => onMode('focus')}
        >
          Focus
        </Button>
      </div>
      {mode === 'focus' && pinnedFocus ? (
        <Button size="sm" variant="outline" onClick={onFollow}>
          Follow the turn
        </Button>
      ) : null}
    </div>
  )
}
