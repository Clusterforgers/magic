import { BookOpenIcon, CircleHelpIcon, RouteIcon } from 'lucide-react'
import { Button } from '@/components/ui/button'
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu'
import { showLexicon } from './lexicon/lexiconStore'
import { startTour } from './tour/tourStore'

/** The help button of the top bar: the rules lexicon and the table tour. */
export function HelpMenu({ compact = false }: { compact?: boolean }) {
  return (
    <DropdownMenu>
      <DropdownMenuTrigger
        render={
          <Button
            data-slot="help-menu"
            variant="ghost"
            size={compact ? 'icon-sm' : 'icon'}
            title="Help"
            aria-label="Help"
          />
        }
      >
        <CircleHelpIcon aria-hidden="true" />
      </DropdownMenuTrigger>
      <DropdownMenuContent align="end">
        <DropdownMenuItem onClick={() => showLexicon()}>
          <BookOpenIcon aria-hidden="true" />
          Rules lexicon
        </DropdownMenuItem>
        <DropdownMenuItem onClick={startTour}>
          <RouteIcon aria-hidden="true" />
          Table tour
        </DropdownMenuItem>
      </DropdownMenuContent>
    </DropdownMenu>
  )
}
