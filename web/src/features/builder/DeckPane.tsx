import { LibraryIcon, PencilIcon, SaveIcon, CircleAlertIcon } from 'lucide-react'
import { useId } from 'react'
import type { Ref } from 'react'
import { Link } from '@tanstack/react-router'
import { EmptyState } from '@/components/EmptyState'
import { Pill } from '@/components/Pill'
import { Alert, AlertDescription } from '@/components/ui/alert'
import { Button, buttonVariants } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { NativeSelect, NativeSelectOption } from '@/components/ui/native-select'
import { DeckStateBadge } from '@/features/decks/DeckStateBadge'
import { cardsText, DECK_FORMATS, formatLabel } from '@/features/decks/format'
import { cn } from '@/lib/utils'
import type { DeckState, DeckZone } from '@/protocol/generated'
import { hasRoom, linesIn, totalOf } from './deckEdit'
import type { Draft } from './deckEdit'
import { LineRow } from './LineRow'
import type { LineActions } from './LineRow'
import { ZONE_TITLE, ZONES } from './zones'

/** What the deck pane can ask of the builder. */
export interface DeckActions extends LineActions {
  rename: (name: string) => void
  setFormat: (format: string) => void
  save: () => void
}

interface DeckPaneProps {
  draft: Draft
  /** The check state of the deck as last saved. */
  state: DeckState
  /** The draft differs from what is saved. */
  dirty: boolean
  saving: boolean
  /** What is wrong with the name, in words, once a save was tried. */
  nameProblem: string | null
  saveError: { message: string; sessionEnded: boolean; deckId: string } | null
  nameRef: Ref<HTMLInputElement>
  actions: DeckActions
  /** Sets the display of the pane (`flex` or `hidden`), which the page switches under the M breakpoint. */
  className?: string
}

/**
 * The deck half of the builder: the name, format and Save (with the saved state, or "Unsaved changes" while the draft
 * differs), then the lines by zone with a total each. Only the lines scroll.
 */
export function DeckPane({
  draft,
  state,
  dirty,
  saving,
  nameProblem,
  saveError,
  nameRef,
  actions,
  className,
}: DeckPaneProps) {
  const id = useId()
  const total = totalOf(draft)
  const room = hasRoom(draft)
  const zones: readonly DeckZone[] = ZONES.filter(
    (zone) =>
      zone === 'main' || totalOf(draft, zone) > 0 || (zone === 'commander' && draft.format === 'commander'),
  )
  return (
    <section aria-label="Deck" className={cn('surface min-h-0 flex-col', className)}>
      <header className="grid gap-2 border-b p-3">
        <div className="flex items-start gap-2">
          <div className="grid min-w-0 flex-1 gap-1">
            <Input
              ref={nameRef}
              aria-label="Deck name"
              value={draft.name}
              autoComplete="off"
              aria-invalid={nameProblem !== null}
              aria-describedby={nameProblem === null ? undefined : `${id}-name-problem`}
              className="font-semibold"
              onChange={(event) => actions.rename(event.target.value)}
            />
            {nameProblem === null ? null : (
              <p id={`${id}-name-problem`} className="text-sm text-destructive">
                {nameProblem}
              </p>
            )}
          </div>
          <Button
            type="button"
            loading={saving}
            aria-disabled={!dirty || undefined}
            onClick={() => dirty && actions.save()}
          >
            <SaveIcon aria-hidden="true" />
            Save
          </Button>
        </div>
        <div className="flex flex-wrap items-center gap-x-3 gap-y-2">
          <NativeSelect
            aria-label="Format"
            value={draft.format}
            className="w-44"
            onChange={(event) => actions.setFormat(event.target.value)}
          >
            {DECK_FORMATS.includes(draft.format) ? null : (
              <NativeSelectOption value={draft.format}>{draft.format}</NativeSelectOption>
            )}
            {DECK_FORMATS.map((key) => (
              <NativeSelectOption key={key} value={key}>
                {formatLabel(key)}
              </NativeSelectOption>
            ))}
          </NativeSelect>
          {dirty ? (
            <Pill tone="warning">
              <PencilIcon aria-hidden="true" />
              Unsaved changes
            </Pill>
          ) : (
            <DeckStateBadge state={state} />
          )}
          <p className="text-sm text-muted-foreground tabular-nums">{cardsText(total)}</p>
        </div>
        {saveError === null ? null : (
          <Alert variant="destructive">
            <CircleAlertIcon aria-hidden="true" />
            <AlertDescription className="text-foreground">{saveError.message}</AlertDescription>
            {saveError.sessionEnded ? (
              <div className="mt-2">
                <Link
                  to="/login"
                  search={{ next: `/decks/${saveError.deckId}` }}
                  className={buttonVariants({ variant: 'outline' })}
                >
                  Sign in
                </Link>
              </div>
            ) : null}
          </Alert>
        )}
        {room ? null : (
          <p role="status" className="text-sm text-muted-foreground">
            The deck is at its limit of 250 cards.
          </p>
        )}
      </header>
      <div className="min-h-0 flex-1 p-3 sm:overflow-y-auto sm:[scrollbar-gutter:stable]">
        {total === 0 ? (
          <EmptyState icon={LibraryIcon} as="h3" title="This deck has no cards">
            Search for a card and add it with the row or the Enter key. Each card appears here with its
            copies.
          </EmptyState>
        ) : (
          <div className="grid gap-4">
            {zones.map((zone) => {
              const lines = linesIn(draft, zone)
              return (
                <section key={zone} aria-labelledby={`${id}-${zone}`} className="grid gap-1">
                  <h3
                    id={`${id}-${zone}`}
                    className="flex items-baseline justify-between gap-2 border-b pb-1 text-sm font-semibold"
                  >
                    {ZONE_TITLE[zone]}{' '}
                    <span className="font-normal text-muted-foreground tabular-nums">
                      {totalOf(draft, zone)}
                    </span>
                  </h3>
                  {lines.length === 0 ? (
                    <p className="px-1 py-2 text-sm text-muted-foreground">
                      {zone === 'commander'
                        ? 'No commander yet. Use the menu beside a card in the search to add one.'
                        : 'No cards here yet.'}
                    </p>
                  ) : (
                    <ul aria-label={`${ZONE_TITLE[zone]} cards`} className="grid gap-1">
                      {lines.map((line) => (
                        <LineRow
                          key={line.card.oracle_id}
                          line={line}
                          format={draft.format}
                          room={room}
                          actions={actions}
                        />
                      ))}
                    </ul>
                  )}
                </section>
              )
            })}
          </div>
        )}
      </div>
    </section>
  )
}
