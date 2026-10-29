import { CheckIcon } from 'lucide-react'
import type { ReactNode } from 'react'
import { Pill } from '@/components/Pill'
import { Button } from '@/components/ui/button'
import { DeckStateBadge } from '@/features/decks/DeckStateBadge'
import { cn } from '@/lib/utils'
import type { DeckPick } from './useDeckChoice'
import { blockedLine, verdictLabel, verdictState } from './verdict'
import type { Verdict } from './verdict'
import type { VerdictState } from './useVerdicts'

interface DeckRowProps {
  /** The id the server takes: a built-in deck's, a stored deck's or `precon:<code>`. */
  id: string
  name: string
  /** The facts under the name: format, size, set. */
  meta: ReactNode
  verdict: VerdictState
  selected: boolean
  pending: boolean
  /** Another request is out. */
  waiting: boolean
  onPress(pick: DeckPick): void
  onRetry(id: string): void
}

function Reasons({ verdict, name }: { verdict: Verdict; name: string }) {
  if (verdict.kind === 'runs') return null
  const items = verdict.kind === 'blocked' ? verdict.reasons : verdict.names
  if (items.length === 0) return null
  return (
    <details data-slot="deck-reasons" className="px-1 text-xs">
      <summary className="cursor-pointer py-1 font-medium underline-offset-3 hover:underline">
        {verdict.kind === 'blocked' ? 'Why' : 'Which cards'}
      </summary>
      <ul
        aria-label={
          verdict.kind === 'blocked' ? `Why ${name} cannot be taken` : `Cards of ${name} played by hand`
        }
        className="list-disc pl-4 text-muted-foreground"
      >
        {items.map((item) => (
          <li key={item} className="[overflow-wrap:anywhere]">
            {item}
          </li>
        ))}
      </ul>
    </details>
  )
}

/**
 * One deck of the chooser: a button that chooses it, the table's verdict on it as an icon and words, and for a deck
 * the table cannot take the reason beside it. A deck that cannot be taken is not pressable but stays focusable, so the
 * reason is read with it; one the check did not answer for can still be pressed, and the server judges it.
 */
export function DeckRow({
  id,
  name,
  meta,
  verdict,
  selected,
  pending,
  waiting,
  onPress,
  onRetry,
}: DeckRowProps) {
  const reasonId = `deck-reason-${id}`
  const blocked = verdict.status === 'ready' && verdict.verdict.kind === 'blocked' ? verdict.verdict : null
  const line = blocked === null ? null : blockedLine(blocked.reasons)
  return (
    <li data-deck-option={id} className="grid content-start gap-1">
      <Button
        type="button"
        variant="outline"
        aria-pressed={selected}
        aria-describedby={line === null ? undefined : reasonId}
        loading={pending}
        disabled={blocked !== null || (waiting && !pending)}
        focusableWhenDisabled={blocked !== null}
        className={cn(
          'h-auto min-h-11 w-full justify-start gap-3 py-2 text-left whitespace-normal',
          selected && 'border-primary',
        )}
        onClick={() => onPress({ id, name })}
      >
        {pending || !selected ? null : <CheckIcon aria-hidden="true" />}
        <span className="grid min-w-0 flex-1 gap-1">
          <span className="font-medium [overflow-wrap:anywhere]">{name}</span>
          <span className="flex flex-wrap items-center gap-x-2 gap-y-1 text-xs font-normal text-muted-foreground">
            <span>{meta}</span>
            {verdict.status === 'ready' ? (
              <DeckStateBadge state={verdictState(verdict.verdict)} label={verdictLabel(verdict.verdict)} />
            ) : verdict.status === 'checking' ? (
              <Pill tone="neutral" data-slot="deck-checking">
                Checking at this table…
              </Pill>
            ) : null}
          </span>
        </span>
      </Button>
      {line === null ? null : (
        <p
          id={reasonId}
          data-slot="deck-blocked"
          className="px-1 text-xs text-destructive [overflow-wrap:anywhere]"
        >
          {line}
        </p>
      )}
      {verdict.status === 'ready' ? <Reasons verdict={verdict.verdict} name={name} /> : null}
      {verdict.status === 'failed' ? (
        <div className="flex flex-wrap items-center gap-2 px-1 text-xs">
          <span data-slot="deck-check-failed" className="text-muted-foreground">
            Could not check this deck at the table.
          </span>
          <Button type="button" variant="outline" size="sm" onClick={() => onRetry(id)}>
            Check again
          </Button>
        </div>
      ) : null}
    </li>
  )
}
