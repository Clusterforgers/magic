import { ChevronRight } from 'lucide-react'
import { useId } from 'react'
import { Label } from '@/components/ui/label'

interface AdvancedOptionsProps {
  /** A link for every seat is on. */
  links: boolean
  /** A link per seat is a Normal-format flow: Commander decks are chosen in the lobby. */
  linksAllowed: boolean
  onLinks(links: boolean): void
}

/** The options behind a disclosure: the earlier way of making a game, with a link for every seat. */
export function AdvancedOptions({ links, linksAllowed, onLinks }: AdvancedOptionsProps) {
  const linksId = useId()
  return (
    <details className="group surface">
      <summary className="flex cursor-pointer list-none items-center gap-2 px-4 py-3 text-sm font-medium marker:hidden pointer-coarse:py-4 [&::-webkit-details-marker]:hidden">
        <ChevronRight
          aria-hidden="true"
          className="size-4 transition-transform duration-fast ease-out group-open:rotate-90"
        />
        Advanced
      </summary>
      <div className="grid gap-4 border-t px-4 py-4">
        <div className="flex items-start gap-3">
          <input
            id={linksId}
            type="checkbox"
            checked={links}
            disabled={!linksAllowed}
            aria-describedby={`${linksId}-help`}
            className="mt-1 accent-primary"
            onChange={(event) => onLinks(event.target.checked)}
          />
          <div className="grid gap-1">
            <Label htmlFor={linksId}>Give every seat its own link</Label>
            <p id={`${linksId}-help`} className="text-sm text-muted-foreground">
              Name each seat and choose its deck yourself, then send every player the link for their seat. The
              game is invite only.
              {linksAllowed ? '' : ' It needs the Normal format.'}
            </p>
          </div>
        </div>
      </div>
    </details>
  )
}
