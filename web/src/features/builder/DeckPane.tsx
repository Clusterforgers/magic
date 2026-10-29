import {
  ChevronDownIcon,
  ChevronRightIcon,
  CircleAlertIcon,
  HandIcon,
  LibraryIcon,
  PencilIcon,
  ShieldCheckIcon,
} from 'lucide-react'
import { useEffect, useId, useMemo, useRef, useState } from 'react'
import type { ReactNode, Ref } from 'react'
import { Link } from '@tanstack/react-router'
import { EmptyState } from '@/components/EmptyState'
import { Pill } from '@/components/Pill'
import { Alert, AlertDescription } from '@/components/ui/alert'
import { Button, buttonVariants } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { NativeSelect, NativeSelectOption } from '@/components/ui/native-select'
import { DeckMarker } from '@/features/decks/DeckMarker'
import { cardsText, DECK_FORMATS, formatLabel } from '@/features/decks/format'
import { byHand, copiesOf, landCopies } from '@/features/decks/preview/model'
import { cn } from '@/lib/utils'
import type { FixMarksApi } from '@/net/fixMarks'
import type { DeckState, DeckZone } from '@/protocol/generated'
import { CheckBar } from './CheckBar'
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

/** Which part of the pane an under-`lg` window shows: the cards, or the numbers and the cards played by hand. */
export type DeckPart = 'deck' | 'stats'

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
  /** The card whose choice as a commander is being checked, if one is. */
  judging?: string | null
  /** Sets the card search to look for a commander, partner or Background. */
  onFind?: (kind: FindKind) => void
  /** The fix-mark endpoints; with them a card that is not Verified gets the flag in its row. */
  fixMarks?: FixMarksApi
  /** The part an under-`lg` window shows; from `lg` the pane shows both. */
  part?: DeckPart
  /** Asks the page to bring the cards played by hand into view (it switches to the Stats part under `lg`). */
  onShowHand?: () => void
  /** Sets the display of the pane (`flex` or `hidden`), which the page switches under the M breakpoint. */
  className?: string
}

/** A heading that opens and closes the section under it. */
function Disclosure({
  id,
  title,
  count,
  summary,
  open,
  onToggle,
  children,
  sectionRef,
}: {
  id: string
  title: string
  count?: number
  /** A few words that stay beside the title while the section is closed. */
  summary?: string
  open: boolean
  onToggle: () => void
  children: ReactNode
  sectionRef?: Ref<HTMLElement>
}) {
  return (
    <section ref={sectionRef} aria-labelledby={`${id}-title`} className="grid gap-2">
      <h3 id={`${id}-title`} className="border-b text-sm font-semibold">
        <button
          type="button"
          aria-expanded={open}
          aria-controls={`${id}-body`}
          onClick={onToggle}
          className="flex min-h-8 w-full items-center gap-2 rounded-sm py-1 text-left pointer-coarse:min-h-11"
        >
          {open ? (
            <ChevronDownIcon aria-hidden="true" className="size-4" />
          ) : (
            <ChevronRightIcon aria-hidden="true" className="size-4" />
          )}
          {title}
          {summary === undefined ? null : (
            <span className="ml-auto font-normal text-muted-foreground tabular-nums">{summary}</span>
          )}
          {count === undefined ? null : (
            <span className="ml-auto font-normal text-muted-foreground tabular-nums">{count}</span>
          )}
        </button>
      </h3>
      <div id={`${id}-body`} hidden={!open}>
        {children}
      </div>
    </section>
  )
}

/**
 * The deck half of the builder: the name and format, the marker of the deck's check ("Unsaved changes" joins it while
 * the draft differs), the size with how many cards are played by hand, the check's counts with the Issues only switch,
 * the sort and the choice between pictures and the dense list, then the commanders, the legality issues, the stats and
 * the cards played by hand (disclosures) and the cards by zone, and at the foot the save bar. The counts, marks, reasons
 * and legality follow the draft through the live check. Only the body scrolls. Under `lg` a part is chosen by the page:
 * the cards, or the stats with the cards played by hand.
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
  judging = null,
  onFind = () => undefined,
  fixMarks,
  part = 'deck',
  onShowHand = () => undefined,
  className,
}: DeckPaneProps) {
  const id = useId()
  const [issuesOnly, setIssuesOnly] = useState(false)
  const [sort, setSort] = useState<SortKey>('name')
  const [look, setLook] = useDeckLook()
  const [statsOpen, setStatsOpen] = useState(false)
  const [handOpen, setHandOpen] = useState(false)
  const [seenPart, setSeenPart] = useState(part)
  const handRef = useRef<HTMLElement>(null)
  const [reveal, setReveal] = useState(0)
  const { report } = check
  const checks = useMemo(() => checksOf(report), [report])
  // The report is of the saved list while the draft's cards differ and no check of them has come back.
  const asSaved = check.source === 'saved' && check.status !== 'current'
  const total = totalOf(draft)
  const room = hasRoom(draft)
  // The Stats part is for reading what is played by hand, so it opens that list.
  if (part !== seenPart) {
    setSeenPart(part)
    if (part === 'stats') {
      setStatsOpen(true)
      setHandOpen(true)
    }
  }
  useEffect(() => {
    if (reveal > 0) handRef.current?.scrollIntoView({ block: 'nearest' })
  }, [reveal])
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
  const played = copiesOf([...preview.commanders, ...preview.cards])
  const lands = landCopies([...preview.commanders, ...preview.cards])
  const counts = report.summary.counts
  const byHandCopies = byHand(counts)
  const status = saveStatus({
    dirty,
    saving,
    name: draft.name,
    issues: report.legality.length,
    format: formatLabel(draft.format),
  })
  const showHand = (): void => {
    setHandOpen(true)
    setReveal((n) => n + 1)
    onShowHand()
  }
  return (
    <section
      aria-label="Deck"
      data-part={part}
      className={cn('group/pane surface min-h-0 flex-col', className)}
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
          <DeckMarker state={state} changed={changed} dirty={dirty} />
          {dirty ? (
            <Pill tone="warning">
              <PencilIcon aria-hidden="true" />
              Unsaved changes
            </Pill>
          ) : null}
          <p className="text-sm text-muted-foreground tabular-nums">{cardsText(total)}</p>
          {total === 0 ? null : byHandCopies > 0 || counts.unplayable > 0 ? (
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
        {total === 0 ? null : (
          <div className="max-lg:group-data-[part=stats]/pane:hidden">
            <CheckBar
              counts={counts}
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
          </div>
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
        {total === 0 && !slot ? (
          <EmptyState icon={LibraryIcon} as="h3" title="This deck has no cards">
            Search for a card and add it with the row or the Enter key. Each card appears here with its
            copies.
          </EmptyState>
        ) : (
          <div className="grid gap-4">
            {slot ? (
              <div className="max-lg:group-data-[part=stats]/pane:hidden">
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
              </div>
            ) : null}
            {total === 0 ? (
              <EmptyState icon={LibraryIcon} as="h3" title="This deck has no cards">
                Search for a card and add it with the row or the Enter key. Each card appears here with its
                copies.
              </EmptyState>
            ) : (
              <>
                <div className="max-lg:group-data-[part=stats]/pane:hidden">
                  <LegalitySection issues={report.legality} format={report.format} stale={asSaved} />
                </div>
                <div className="grid gap-4 max-lg:group-data-[part=deck]/pane:hidden">
                  <Disclosure
                    id={`${id}-stats`}
                    title="Stats"
                    summary={`${lands} ${lands === 1 ? 'land' : 'lands'}, ${played - lands} ${played - lands === 1 ? 'spell' : 'spells'}`}
                    open={statsOpen}
                    onToggle={() => setStatsOpen((open) => !open)}
                  >
                    <StatsPanel deck={preview} counts={counts} asSaved={asSaved} />
                  </Disclosure>
                  <Disclosure
                    id={`${id}-hand`}
                    title="Played by hand"
                    count={byHandCopies}
                    open={handOpen}
                    onToggle={() => setHandOpen((open) => !open)}
                    sectionRef={handRef}
                  >
                    <HandList cards={hand} onPreview={actions.preview} fixMarks={fixMarks} />
                  </Disclosure>
                </div>
                <div className="grid gap-4 max-lg:group-data-[part=stats]/pane:hidden">
                  {shown.length === 0 ? (
                    <EmptyState icon={ShieldCheckIcon} as="h3" title="No cards with issues">
                      Every card in this deck plays without manual steps. Turn off Issues only to see the
                      whole deck.
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
                              />
                            ))}
                          </ul>
                        )}
                      </section>
                    )
                  })}
                </div>
              </>
            )}
          </div>
        )}
      </div>
      <SaveBar status={status} onSave={actions.save} />
    </section>
  )
}
