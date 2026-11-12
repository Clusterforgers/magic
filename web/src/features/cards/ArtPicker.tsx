import { useEffect, useRef, useState } from 'react'
import type { KeyboardEvent as ReactKeyboardEvent } from 'react'
import { CheckIcon } from 'lucide-react'
import { Button } from '@/components/ui/button'
import { Dialog, DialogContent, DialogDescription, DialogTitle } from '@/components/ui/dialog'
import { Input } from '@/components/ui/input'
import { cn } from '@/lib/utils'
import type { PrintingsApi } from '@/net/printings'
import type { PrintingView } from '@/protocol/generated'
import { filterPrintings, gridMove, printingCaption, printingLabel, printingTraits } from './artChoice'
import { CardImage } from './CardImage'
import { cardImageUrl, identityOf } from './images'

/** What the picker chooses for: the card, the art it shows now and how a choice is saved. */
export interface ArtTarget {
  oracleId: string
  name: string
  /** The printing the line chose; null for the default art. */
  chosen: string | null
}

interface ArtPickerProps {
  /** The card whose art is chosen; null keeps the picker closed. */
  target: ArtTarget | null
  api: Pick<PrintingsApi, 'printingsOf'>
  /** Saves a choice (null for the default); the picker closes once it resolves and shows the error if it rejects. */
  onChoose(printingId: string | null, everywhere: boolean): Promise<void>
  /** Offers "use in every deck", which passes `everywhere` to `onChoose`. */
  offerEverywhere?: boolean
  onClose(): void
}

type Loading = { kind: 'loading' } | { kind: 'failed'; message: string } | { kind: 'ready'; printings: PrintingView[] }

/**
 * A compact picker of a card's art: a grid of its printings, newest first, with their pictures asked for as they come
 * into view, a filter by set code or name, and Default. Choosing saves at once. Arrow keys, Home and End move between
 * the printings; Enter or Space chooses.
 */
export function ArtPicker({ target, api, onChoose, offerEverywhere = false, onClose }: ArtPickerProps) {
  const [loading, setLoading] = useState<Loading>({ kind: 'loading' })
  const [query, setQuery] = useState('')
  const [everywhere, setEverywhere] = useState(false)
  const [saving, setSaving] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const grid = useRef<HTMLUListElement>(null)
  const oracleId = target?.oracleId ?? null

  useEffect(() => {
    if (oracleId === null) return
    const controller = new AbortController()
    setLoading({ kind: 'loading' })
    setQuery('')
    setError(null)
    api.printingsOf(oracleId, controller.signal).then(
      (printings) => setLoading({ kind: 'ready', printings }),
      (failure: unknown) => {
        if (controller.signal.aborted) return
        setLoading({ kind: 'failed', message: failure instanceof Error ? failure.message : 'The art did not load.' })
      },
    )
    return () => controller.abort()
  }, [api, oracleId])

  const choose = async (printingId: string | null): Promise<void> => {
    if (saving) return
    setSaving(true)
    setError(null)
    try {
      await onChoose(printingId, everywhere)
      onClose()
    } catch (failure) {
      setError(failure instanceof Error ? failure.message : 'The art could not be saved.')
    } finally {
      setSaving(false)
    }
  }

  const shown = loading.kind === 'ready' ? filterPrintings(loading.printings, query) : []

  const onGridKey = (event: ReactKeyboardEvent<HTMLUListElement>): void => {
    const list = grid.current
    if (list === null) return
    const options = [...list.querySelectorAll<HTMLButtonElement>('button[data-printing]')]
    const at = options.findIndex((option) => option === document.activeElement)
    if (at < 0) return
    const columns = getComputedStyle(list).gridTemplateColumns.split(' ').filter(Boolean).length || 1
    const next = gridMove(event.key, at, columns, options.length)
    if (next === null) return
    event.preventDefault()
    options[next]?.focus()
  }

  return (
    <Dialog
      open={target !== null}
      onOpenChange={(open) => {
        if (!open && !saving) onClose()
      }}
    >
      <DialogContent data-slot="art-picker" className="max-w-2xl gap-3">
        <div className="grid gap-1 pr-8">
          <DialogTitle>Art for {target?.name}</DialogTitle>
          <DialogDescription>Choose a printing; it is saved at once.</DialogDescription>
        </div>
        <div className="flex flex-wrap items-center gap-2">
          <Input
            type="search"
            aria-label="Filter by set"
            placeholder="Filter by set"
            value={query}
            onChange={(event) => setQuery(event.target.value)}
            className="min-w-0 flex-1"
          />
          <Button
            type="button"
            variant={target?.chosen === null ? 'default' : 'outline'}
            aria-pressed={target?.chosen === null}
            disabled={saving}
            onClick={() => void choose(null)}
          >
            Default
          </Button>
        </div>
        {offerEverywhere ? (
          <label className="flex items-center gap-2 text-sm">
            <input type="checkbox" checked={everywhere} onChange={(event) => setEverywhere(event.target.checked)} />
            Use this art in all my decks
          </label>
        ) : null}
        {error === null ? null : (
          <p role="alert" className="text-sm text-destructive">
            {error}
          </p>
        )}
        {loading.kind === 'loading' ? (
          <p role="status" className="text-sm text-muted-foreground">
            Loading the printings…
          </p>
        ) : loading.kind === 'failed' ? (
          <p role="alert" className="text-sm text-destructive">
            {loading.message}
          </p>
        ) : shown.length === 0 ? (
          <p role="status" className="text-sm text-muted-foreground">
            No printing of a set like “{query.trim()}”.
          </p>
        ) : (
          <ul
            ref={grid}
            aria-label="Printings"
            aria-busy={saving}
            onKeyDown={onGridKey}
            className="grid grid-cols-3 gap-2 sm:grid-cols-4"
          >
            {shown.map((printing) => {
              const current = printing.printing_id === target?.chosen
              const traits = printingTraits(printing)
              return (
                <li key={printing.printing_id} className="min-w-0">
                  <button
                    type="button"
                    data-printing={printing.printing_id}
                    aria-pressed={current}
                    aria-label={printingLabel(printing)}
                    title={printing.set_name ?? undefined}
                    disabled={saving}
                    onClick={() => void choose(printing.printing_id)}
                    className={cn(
                      'grid w-full gap-1 rounded-md p-1 text-left outline-offset-2 hover:bg-accent',
                      current && 'bg-accent ring-2 ring-primary',
                    )}
                  >
                    <span className="relative block">
                      <CardImage
                        src={cardImageUrl(
                          { kind: 'card', identity: identityOf(oracleId ?? '', printing.printing_id) },
                          0,
                          'small',
                        )}
                        alt=""
                        className="aspect-[5/7] w-full"
                      >
                        <span className="grid h-full place-items-center rounded-md bg-muted text-xs text-muted-foreground">
                          {printing.set.toUpperCase()}
                        </span>
                      </CardImage>
                      {current ? (
                        <CheckIcon
                          aria-hidden="true"
                          className="absolute top-1 right-1 size-5 rounded-full bg-primary p-0.5 text-primary-foreground"
                        />
                      ) : null}
                    </span>
                    <span className="truncate text-xs font-medium tabular-nums">{printingCaption(printing)}</span>
                    {traits.length === 0 ? null : (
                      <span className="truncate text-xs text-muted-foreground">{traits.join(' · ')}</span>
                    )}
                  </button>
                </li>
              )
            })}
          </ul>
        )}
      </DialogContent>
    </Dialog>
  )
}
