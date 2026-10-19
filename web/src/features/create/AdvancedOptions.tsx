import { ChevronRight } from 'lucide-react'
import { useId } from 'react'
import { Pill } from '@/components/Pill'
import { Label } from '@/components/ui/label'

interface AdvancedOptionsProps {
  /** A link for every seat is on. */
  links: boolean
  onLinks(links: boolean): void
}

/** The options behind a disclosure: the format, and the earlier way of making a game with a link for every seat. */
export function AdvancedOptions({ links, onLinks }: AdvancedOptionsProps) {
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
        <fieldset className="grid gap-2">
          <legend className="mb-1 text-sm font-medium">Format</legend>
          <div className="flex flex-wrap gap-2">
            <label className="flex min-h-9 items-center gap-2 rounded-lg border border-border-strong px-3 py-2 text-sm has-checked:border-primary has-checked:bg-muted pointer-coarse:min-h-11">
              <input type="radio" name="format" value="normal" defaultChecked className="accent-primary" />
              Normal
            </label>
            <label className="flex min-h-9 items-center gap-2 rounded-lg border border-border px-3 py-2 text-sm text-muted-foreground pointer-coarse:min-h-11">
              <input type="radio" name="format" value="commander" disabled />
              Commander
              <Pill>coming soon</Pill>
            </label>
          </div>
        </fieldset>
        <div className="flex items-start gap-3">
          <input
            id={linksId}
            type="checkbox"
            checked={links}
            aria-describedby={`${linksId}-help`}
            className="mt-1 accent-primary"
            onChange={(event) => onLinks(event.target.checked)}
          />
          <div className="grid gap-0.5">
            <Label htmlFor={linksId}>Give every seat its own link</Label>
            <p id={`${linksId}-help`} className="text-sm text-muted-foreground">
              Name each seat and choose its deck yourself, then send every player the link for their seat. The
              game is invite only.
            </p>
          </div>
        </div>
      </div>
    </details>
  )
}
