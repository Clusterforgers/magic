import { CircleAlertIcon, LibraryIcon, PencilIcon, SaveIcon, ShieldCheckIcon } from 'lucide-react'
import { useId, useMemo, useState } from 'react'
import type { Ref } from 'react'
import { Link } from '@tanstack/react-router'
import { EmptyState } from '@/components/EmptyState'
import { Pill } from '@/components/Pill'
import { Alert, AlertDescription } from '@/components/ui/alert'
import { Button, buttonVariants } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { NativeSelect, NativeSelectOption } from '@/components/ui/native-select'
import { DeckMarker } from '@/features/decks/DeckMarker'
import { cardsText, DECK_FORMATS, formatLabel } from '@/features/decks/format'
import { cn } from '@/lib/utils'
import type { DeckState, DeckZone } from '@/protocol/generated'
import { CheckBar } from './CheckBar'
import { hasRoom, linesIn, totalOf } from './deckEdit'
import type { Draft } from './deckEdit'
import { LegalitySection } from './LegalitySection'
import { LineRow } from './LineRow'
import type { LineActions } from './LineRow'
import { arrange, checkOf, checksOf } from './report'
import type { SortKey } from './report'
import type { LiveCheck } from './useLiveCheck'
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
  /** A check is stored but the deck, the rules of its cards or the engine changed since it. */
  changed: boolean
  /** The check the pane shows: the saved deck's report, or the live check of the draft (its legality issues and the support of each card). */
  check: LiveCheck
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
 * The deck half of the builder: the name, format and Save, the marker of the deck's check ("Unsaved changes" joins it
 * while the draft differs), the check's counts with the Issues only switch and the sort, then the legality issues and
 * the lines by zone with a total each. The counts, marks, reasons and legality follow the draft through the live
 * check. Only the lines scroll.
 */
export function DeckPane({
  draft,
  state,
  changed,
  check,
  dirty,
  saving,
  nameProblem,
  saveError,
  nameRef,
  actions,
  className,
}: DeckPaneProps) {
  const id = useId()
  const [issuesOnly, setIssuesOnly] = useState(false)
  const [sort, setSort] = useState<SortKey>('name')
  const { report } = check
  const checks = useMemo(() => checksOf(report), [report])
  // The report is of the saved list while the draft's cards differ and no check of them has come back.
  const asSaved = check.source === 'saved' && check.status !== 'current'
  const total = totalOf(draft)
  const room = hasRoom(draft)
  const zones: readonly DeckZone[] = ZONES.filter(
    (zone) =>
      zone === 'main' || totalOf(draft, zone) > 0 || (zone === 'commander' && draft.format === 'commander'),
  )
  const arranged = zones.map((zone) => ({
    zone,
    lines: arrange(linesIn(draft, zone), checks, { issuesOnly, sort }),
  }))
  // With only the issues shown, a zone without any has nothing to list.
  const shown = issuesOnly ? arranged.filter(({ lines }) => lines.length > 0) : arranged
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
          <DeckMarker state={state} changed={changed} dirty={dirty} />
          {dirty ? (
            <Pill tone="warning">
              <PencilIcon aria-hidden="true" />
              Unsaved changes
            </Pill>
          ) : null}
          <p className="text-sm text-muted-foreground tabular-nums">{cardsText(total)}</p>
        </div>
        {total === 0 ? null : (
          <CheckBar
            counts={report.summary.counts}
            asSaved={asSaved}
            status={check.status}
            error={check.error}
            source={check.source}
            onRetry={check.retry}
            issuesOnly={issuesOnly}
            onIssuesOnly={setIssuesOnly}
            sort={sort}
            onSort={setSort}
          />
        )}
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
            <LegalitySection issues={report.legality} format={report.format} stale={asSaved} />
            {shown.length === 0 ? (
              <EmptyState icon={ShieldCheckIcon} as="h3" title="No cards with issues">
                Every card in this deck plays without manual steps. Turn off Issues only to see the whole
                deck.
              </EmptyState>
            ) : null}
            {shown.map(({ zone, lines }) => {
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
                          check={checkOf(checks, line)}
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
