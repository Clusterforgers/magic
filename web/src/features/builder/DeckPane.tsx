import {
  ChartColumnIcon,
  CircleAlertIcon,
  HandIcon,
  LibraryIcon,
  PencilIcon,
  ShieldCheckIcon,
} from 'lucide-react'
import { useId, useMemo, useRef, useState } from 'react'
import type { Ref } from 'react'
import { Link } from '@tanstack/react-router'
import { EmptyState } from '@/components/EmptyState'
import { Pill } from '@/components/Pill'
import { Alert, AlertDescription } from '@/components/ui/alert'
import { Button, buttonVariants } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { NativeSelect, NativeSelectOption } from '@/components/ui/native-select'
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs'
import { DeckMarker } from '@/features/decks/DeckMarker'
import { cardsText, DECK_FORMATS, formatLabel } from '@/features/decks/format'
import { BY_HAND_TITLE, byHandCopies } from '@/lib/byHand'
import { cn } from '@/lib/utils'
import type { FixMarksApi } from '@/net/fixMarks'
import type { DeckState, DeckZone } from '@/protocol/generated'
import { CheckBar } from './CheckBar'
import type { CardsApi } from '@/net/cards'
import { CommanderSlot } from './CommanderSlot'
import type { FindKind } from './CommanderSlot'
import { hasRoom, linesIn, totalOf } from './deckEdit'
import type { Draft } from './deckEdit'
import { useDeckLook } from './deckLook'
import { handCards, handSentence, previewFromDraft } from './deckSummary'
import { HandList } from './HandList'
import { cardsIn, identityOf } from './identity'
import { LegalitySection } from './LegalitySection'
import { LineRow } from './LineRow'
import type { LineActions } from './LineRow'
import { LookToggle } from './LookToggle'
import { PictureZone } from './PictureZone'
import { arrange, checkOf, checksOf, lineKey, outsideLines } from './report'
import type { SortKey } from './report'
import { SaveBar } from './SaveBar'
import { saveStatus } from './saveStatus'
import { StatsPanel } from './StatsPanel'
import type { LiveCheck } from './useLiveCheck'
import { ZONE_TITLE, ZONES } from './zones'

/** What the deck pane can ask of the builder. */
export interface DeckActions extends LineActions {
  rename: (name: string) => void
  setFormat: (format: string) => void
  save: () => void
}

/** The views of the pane's body: the cards by zone, the deck in numbers, and the cards played by hand. */
export type PaneView = 'cards' | 'stats' | 'hand'

const isView = (value: unknown): value is PaneView =>
  value === 'cards' || value === 'stats' || value === 'hand'

interface DeckPaneProps {
  draft: Draft
  /** The saved deck, which its cards to read are read for. */
  deckId?: string
  /** The card endpoint the words of the cards played by hand are quoted from. */
  cardsApi: Pick<CardsApi, 'get'>
  /** The check state of the deck as last saved. */
  state: DeckState
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
  /** The card whose choice as a commander is being checked, if one is. */
  judging?: string | null
  /** Sets the card search to look for a commander, partner or Background. */
  onFind?: (kind: FindKind) => void
  /** The fix-mark endpoints; with them a card that is not Verified gets the flag in its row. */
  fixMarks?: FixMarksApi
  /** Tells the page the cards played by hand were asked for, so it brings the pane into view under `lg`. */
  onShowHand?: () => void
  /** Sets the display of the pane (`flex` or `hidden`), which the page switches under the M breakpoint. */
  className?: string
}

/**
 * The deck half of the builder: the name and format, the marker of the deck's check ("Unsaved changes" joins it while
 * the draft differs), the size with how many cards are played by hand, then three views as tabs, each with the whole
 * width of the pane: Cards (the check's counts with the Issues only switch, the sort and the choice between pictures
 * and the dense list, over the commanders, the legality issues and the cards by zone), Stats and Played by hand. At the
 * foot sits the save bar. The counts, marks, reasons and legality follow the draft through the live check. Only the
 * body scrolls.
 */
export function DeckPane({
  draft,
  deckId,
  cardsApi,
  state,
  check,
  dirty,
  saving,
  nameProblem,
  saveError,
  nameRef,
  actions,
  judging = null,
  onFind = () => undefined,
  fixMarks,
  onShowHand = () => undefined,
  className,
}: DeckPaneProps) {
  const id = useId()
  const [issuesOnly, setIssuesOnly] = useState(false)
  const [sort, setSort] = useState<SortKey>('name')
  const [look, setLook] = useDeckLook()
  const [view, setView] = useState<PaneView>('cards')
  const bodyRef = useRef<HTMLDivElement>(null)
  const { report } = check
  const checks = useMemo(() => checksOf(report), [report])
  // The report is of the saved list while the draft's cards differ and no check of them has come back.
  const asSaved = check.source === 'saved' && check.status !== 'current'
  const total = totalOf(draft)
  const room = hasRoom(draft)
  // A Commander deck keeps its command zone in the slot above the lines; any other format lists a stray one as a zone.
  const slot = draft.format === 'commander'
  const zones: readonly DeckZone[] = ZONES.filter(
    (zone) => zone === 'main' || (totalOf(draft, zone) > 0 && !(slot && zone === 'commander')),
  )
  const outsideByLine = useMemo(() => outsideLines(report.legality), [report.legality])
  const flagged = useMemo(() => new Set(outsideByLine.keys()), [outsideByLine])
  const commanders = linesIn(draft, 'commander')
  const arranged = zones.map((zone) => ({
    zone,
    lines: arrange(linesIn(draft, zone), checks, { issuesOnly, sort, flagged }),
  }))
  // With only the issues shown, a zone without any has nothing to list.
  const shown = issuesOnly ? arranged.filter(({ lines }) => lines.length > 0) : arranged
  const preview = useMemo(() => previewFromDraft(draft, checks), [draft, checks])
  const hand = useMemo(() => handCards(draft, checks), [draft, checks])
  const counts = report.summary.counts
  const byHandCount = byHandCopies(counts)
  const status = saveStatus({
    dirty,
    saving,
    name: draft.name,
    issues: report.legality,
    format: draft.format,
  })
  // Each view starts at its top; the body keeps one scroll position for all three.
  const show = (next: PaneView): void => {
    setView(next)
    bodyRef.current?.scrollTo?.({ top: 0 })
  }
  const showHand = (): void => {
    show('hand')
    onShowHand()
  }
  const empty = (
    <EmptyState icon={LibraryIcon} as="h3" title="This deck has no cards">
      Search for a card and add it with the row or the Enter key. Each card appears here with its copies.
    </EmptyState>
  )
  return (
    <section aria-label="Deck" data-view={view} className={cn('surface min-h-0 flex-col', className)}>
      <Tabs
        value={view}
        onValueChange={(next) => {
          if (isView(next)) show(next)
        }}
        className="flex min-h-0 grow flex-col gap-0"
      >
        <header className="grid gap-2 border-b p-3">
          <div className="grid min-w-0 gap-1">
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
            <DeckMarker state={state} dirty={dirty} />
            {dirty ? (
              <Pill tone="warning">
                <PencilIcon aria-hidden="true" />
                Unsaved changes
              </Pill>
            ) : null}
            <p className="text-sm text-muted-foreground tabular-nums">{cardsText(total)}</p>
            {total === 0 ? null : byHandCount > 0 || counts.unplayable > 0 ? (
              <Button
                type="button"
                variant="link"
                size="sm"
                data-slot="hand-summary"
                className="h-auto px-0 py-1 tabular-nums"
                onClick={showHand}
              >
                <HandIcon aria-hidden="true" />
                {handSentence(counts)}
              </Button>
            ) : (
              <p data-slot="hand-summary" className="flex items-center gap-1 text-sm text-muted-foreground">
                <ShieldCheckIcon aria-hidden="true" className="size-4 text-support-verified" />
                {handSentence(counts)}
              </p>
            )}
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
          <TabsList aria-label="Deck views" data-slot="deck-views" className="w-full max-w-full sm:w-fit">
            <TabsTrigger value="cards" className="flex-1 gap-1 px-2 sm:flex-none sm:gap-2 sm:px-3">
              <LibraryIcon aria-hidden="true" className="size-4 max-sm:hidden" />
              Cards
              <span className="font-normal text-muted-foreground tabular-nums">{total}</span>
            </TabsTrigger>
            <TabsTrigger value="stats" className="flex-1 gap-1 px-2 sm:flex-none sm:gap-2 sm:px-3">
              <ChartColumnIcon aria-hidden="true" className="size-4 max-sm:hidden" />
              Stats
            </TabsTrigger>
            <TabsTrigger value="hand" className="flex-1 gap-1 px-2 sm:flex-none sm:gap-2 sm:px-3">
              <HandIcon aria-hidden="true" className="size-4 max-sm:hidden" />
              {BY_HAND_TITLE}
              <span className="font-normal text-muted-foreground tabular-nums">{byHandCount}</span>
            </TabsTrigger>
          </TabsList>
          {view !== 'cards' || total === 0 ? null : (
            <CheckBar
              counts={counts}
              toRead={report.summary.to_read}
              readTarget={deckId === undefined ? undefined : { deck: deckId }}
              deckId={deckId}
              asSaved={asSaved}
              status={check.status}
              error={check.error}
              source={check.source}
              onRetry={check.retry}
              issuesOnly={issuesOnly}
              onIssuesOnly={setIssuesOnly}
              sort={sort}
              onSort={setSort}
              leading={<LookToggle look={look} onChange={setLook} />}
            />
          )}
        </header>
        <div
          ref={bodyRef}
          className="relative min-h-0 flex-1 p-3 sm:overflow-y-auto sm:[scrollbar-gutter:stable]"
        >
          <TabsContent value="cards" className="grid gap-4 outline-none">
            {total === 0 && !slot ? (
              empty
            ) : (
              <>
                {slot ? (
                  <CommanderSlot
                    lines={commanders}
                    identity={identityOf(cardsIn(commanders, 'commander'))}
                    outside={outsideByLine.size}
                    judging={judging}
                    checkOf={(line) => checkOf(checks, line)}
                    onRemove={(card) => actions.remove('commander', card.oracle_id)}
                    onPreview={actions.preview}
                    onFind={onFind}
                  />
                ) : null}
                {total === 0 ? (
                  empty
                ) : (
                  <>
                    <LegalitySection issues={report.legality} format={report.format} stale={asSaved} />
                    {shown.length === 0 ? (
                      <EmptyState icon={ShieldCheckIcon} as="h3" title="No cards with issues">
                        Every card in this deck runs whole. Turn off Issues only to see the whole deck.
                      </EmptyState>
                    ) : null}
                    {shown.map(({ zone, lines }) => (
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
                        ) : look === 'pictures' ? (
                          <PictureZone
                            zone={zone}
                            lines={lines}
                            sort={sort}
                            format={draft.format}
                            room={room}
                            checks={checks}
                            outsideByLine={outsideByLine}
                            actions={actions}
                            fixMarks={fixMarks}
                          />
                        ) : (
                          <ul aria-label={`${ZONE_TITLE[zone]} cards`} className="grid gap-1">
                            {lines.map((line) => (
                              <LineRow
                                key={line.card.oracle_id}
                                line={line}
                                format={draft.format}
                                room={room}
                                check={checkOf(checks, line)}
                                outside={outsideByLine.get(lineKey(line)) ?? null}
                                actions={actions}
                                fixMarks={fixMarks}
                                readTarget={deckId === undefined ? undefined : { deck: deckId }}
                              />
                            ))}
                          </ul>
                        )}
                      </section>
                    ))}
                  </>
                )}
              </>
            )}
          </TabsContent>
          <TabsContent value="stats" className="outline-none">
            {total === 0 ? empty : <StatsPanel deck={preview} counts={counts} asSaved={asSaved} />}
          </TabsContent>
          <TabsContent value="hand" className="outline-none">
            <HandList cards={hand} cardsApi={cardsApi} onPreview={actions.preview} fixMarks={fixMarks} />
          </TabsContent>
        </div>
      </Tabs>
      <SaveBar status={status} onSave={actions.save} />
    </section>
  )
}
