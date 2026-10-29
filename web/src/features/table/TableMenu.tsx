import { setImageChoice, useImageChoice } from '@/features/cards/imageSetting'
import type { ImageChoice } from '@/features/cards/imageSetting'
import { MenuIcon } from 'lucide-react'
import type { ReactNode } from 'react'
import { Button } from '@/components/ui/button'
import { Popover, PopoverContent, PopoverTitle, PopoverTrigger } from '@/components/ui/popover'
import { parseChoice, useTheme } from '@/hooks/useTheme'
import type { ThemeChoice } from '@/hooks/useTheme'
import { usePlay } from '@/features/game/gameContext'
import type { TableMode } from './layout'
import { canConcede } from './selectors'

const THEMES: ReadonlyArray<{ value: ThemeChoice; label: string }> = [
  { value: 'light', label: 'Light' },
  { value: 'dark', label: 'Dark' },
  { value: 'system', label: 'System' },
]

const IMAGE_CHOICES: ReadonlyArray<{ value: ImageChoice; label: string }> = [
  { value: 'on', label: 'On' },
  { value: 'text', label: 'Text only' },
]

function Section({ title, children }: { title: string; children: ReactNode }) {
  return (
    <div role="group" aria-label={title} className="grid gap-2">
      <p aria-hidden="true" className="text-xs font-medium text-muted-foreground">
        {title}
      </p>
      {children}
    </div>
  )
}

interface TableMenuProps {
  /** The mode on screen. */
  mode: TableMode
  /** The player chose a mode; it wins over the rule of the window and the seat count. */
  onMode: (mode: TableMode) => void
  /** True while an opponent has been picked by hand, so "Follow the turn" has something to undo. */
  pinnedFocus: boolean
  onFollow: () => void
  /** Ignored: the stop ladder and Full Control are in the controls dock at every size, never in this menu. */
  stops?: boolean
  /** Asks the table to put the question "Concede the game?" up. */
  onConcede: () => void
  open: boolean
  onOpenChange: (open: boolean) => void
  /** The top bar is low (a phone on its side): a smaller button. */
  compact?: boolean
}

/**
 * The table menu at the right of the top bar: the layout (grid or focus, and following the turn), the theme, and
 * Concede. A panel, not a list of commands, since it holds controls that keep their state.
 */
export function TableMenu({
  mode,
  onMode,
  pinnedFocus,
  onFollow,
  onConcede,
  open,
  onOpenChange,
  compact = false,
}: TableMenuProps) {
  const { choice, setChoice } = useTheme()
  const concede = usePlay(canConcede)
  const images = useImageChoice()
  return (
    <Popover open={open} onOpenChange={onOpenChange}>
      <PopoverTrigger
        render={<Button variant="ghost" size={compact ? 'icon-sm' : 'icon'} aria-label="Table menu" />}
      >
        <MenuIcon aria-hidden="true" />
      </PopoverTrigger>
      <PopoverContent>
        <PopoverTitle className="sr-only">Table menu</PopoverTitle>
        <Section title="Layout">
          <div role="toolbar" aria-label="Table layout" className="flex flex-wrap items-center gap-2">
            <div className="inline-flex gap-0.5 rounded-lg bg-muted p-0.5">
              {(['grid', 'focus'] as const).map((value) => (
                <Button
                  key={value}
                  size="sm"
                  variant={mode === value ? 'default' : 'ghost'}
                  aria-pressed={mode === value}
                  onClick={() => onMode(value)}
                >
                  {value === 'grid' ? 'Grid' : 'Focus'}
                </Button>
              ))}
            </div>
            {mode === 'focus' && pinnedFocus ? (
              <Button size="sm" variant="outline" onClick={onFollow}>
                Follow the turn
              </Button>
            ) : null}
          </div>
        </Section>
        <Section title="Theme">
          <div className="inline-flex gap-0.5 rounded-lg bg-muted p-0.5">
            {THEMES.map(({ value, label }) => (
              <Button
                key={value}
                size="sm"
                variant={choice === value ? 'default' : 'ghost'}
                aria-pressed={choice === value}
                onClick={() => setChoice(parseChoice(value))}
              >
                {label}
              </Button>
            ))}
          </div>
        </Section>
        <Section title="Card images">
          <div className="inline-flex gap-0.5 rounded-lg bg-muted p-0.5">
            {IMAGE_CHOICES.map(({ value, label }) => (
              <Button
                key={value}
                size="sm"
                variant={images === value ? 'default' : 'ghost'}
                aria-pressed={images === value}
                onClick={() => setImageChoice(value)}
              >
                {label}
              </Button>
            ))}
          </div>
        </Section>
        {concede ? (
          <Button
            variant="outline"
            className="justify-start text-destructive"
            aria-haspopup="dialog"
            onClick={onConcede}
          >
            Concede
          </Button>
        ) : null}
      </PopoverContent>
    </Popover>
  )
}
