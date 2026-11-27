import { useCallback, useEffect, useMemo, useRef, useState } from 'react'
import type { CSSProperties, ReactNode } from 'react'
import { createPortal } from 'react-dom'
import { useStore } from 'zustand'
import { CheckIcon, EllipsisIcon } from 'lucide-react'
import { Button } from '@/components/ui/button'
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu'
import { showLexicon } from '@/features/table/lexicon/lexiconStore'
import { holdPreview } from '@/features/table/preview'
import type { Decision, ObjectId } from '@/protocol/generated'
import type { PlayState } from '@/play/store'
import { cn } from '@/lib/utils'
import { ANCHORS, cardRef } from './anchors'
import type { AnchorId, Target } from './anchors'
import { coachPlace } from './coachPlace'
import type { Place } from './coachPlace'
import { loadCursor, saveCursor } from './course'
import { lessonFacts } from './facts'
import type { Facts } from './facts'
import { stepFits } from './gate'
import type { LessonSession } from './lessonSession'
import { Light } from './coachLight'
import { CARD_W, RING_PAD, grown, look, useCardSize, useLooking } from './coachLook'
import { enabled, resolve } from './resolve'
import type { CardLookup } from './resolve'
import { advance, anchorLabel, litOf, shown as shownStep, textParts } from './step'
import type { Lesson, LessonStep, TextPart } from './step'

/** How long a do step may wait without its place before the coach says it lost it, in ms. */
const LOST_MS = 3000
/** How long a held input's reason shows, and the done tick, in ms. */
const REFUSED_MS = 3000
const TICK_MS = 300

/** What a coach does at the lesson's edges. */
export interface CoachProps {
  lesson: Lesson
  /** The game's id, which keeps the step across a reload. */
  game: string
  session: LessonSession
  /** Starts the lesson again in a new game. */
  onRestart: () => void
  /** Back to the Learn page. */
  onLeave: () => void
  /** The lesson is finished: marks it done. */
  onFinish: () => void
  /** Starts the next lesson; absent after the last. */
  onNextLesson?: (() => void) | undefined
  /** The next lesson's name. */
  nextTitle?: string | undefined
}

/** The cards the pending decision can use, which a card ref prefers. */
function usableIds(decision: Decision | null): Set<ObjectId> {
  const ids = new Set<ObjectId>()
  if (decision === null) return ids
  const kind = decision.kind
  if ('Priority' in kind)
    for (const id of [...kind.Priority.castable, ...kind.Priority.playable_lands]) ids.add(id)
  if ('DeclareAttackers' in kind) for (const option of kind.DeclareAttackers.attackers) ids.add(option.object)
  if ('DeclareBlockers' in kind)
    for (const option of kind.DeclareBlockers.blockers) {
      ids.add(option.object)
      for (const id of option.can_block) ids.add(id)
    }
  if ('ChooseTargets' in kind)
    for (const target of kind.ChooseTargets.legal) if ('Object' in target) ids.add(target.Object.id)
  return ids
}

/** The words an element shows: its `data-label`, its text without key hints, else its accessible name. */
function wordsOf(element: HTMLElement): string {
  const label = element.dataset['label']
  if (label !== undefined && label !== '') return label
  const copy = element.cloneNode(true) as HTMLElement
  for (const hint of copy.querySelectorAll('[data-slot="key-hint"], [data-slot="key-cap"], kbd'))
    hint.remove()
  const text = (copy.textContent ?? '').replace(/\s+/g, ' ').trim()
  return text !== '' ? text : (element.getAttribute('aria-label') ?? '')
}

/** A step's words: names of buttons in bold, lexicon terms as links. */
function Words({ parts }: { parts: readonly TextPart[] }) {
  return (
    <>
      {parts.map((part, at) =>
        part.kind === 'name' ? (
          <strong key={at} className="font-semibold">
            {part.text}
          </strong>
        ) : part.kind === 'term' ? (
          <button
            key={at}
            type="button"
            className="font-medium underline decoration-dotted underline-offset-2"
            onClick={() => showLexicon(part.text)}
          >
            {part.text}
          </button>
        ) : (
          <span key={at}>{part.text}</span>
        ),
      )}
    </>
  )
}

/**
 * The coach of a Learn lesson (`docs/tutorial.md`, Coach): over the table it lights the one part to press with a ring
 * in its own colour, a scrim and a pointer, and shows the step in a card placed clear of what it lights, the prompt
 * band, the primary button, an open sheet and the preview. It moves on as the game does (`advance`), keeps its step
 * across a reload, sets the session's gate to the step's input and shows why an input was held back.
 */
export function Coach({
  lesson,
  game,
  session,
  onRestart,
  onLeave,
  onFinish,
  onNextLesson,
  nextTitle,
}: CoachProps) {
  const state = useStore(session.store)
  const refused = useStore(session.coach, (coach) => coach.refused)
  const steps = lesson.steps
  const [index, setIndex] = useState(() => Math.min(loadCursor(game), steps.length))
  const [press, setPress] = useState(0)
  const [playOn, setPlayOn] = useState(false)
  const [folded, setFolded] = useState(false)
  const [ticked, setTicked] = useState<number | null>(null)
  const [lostSince, setLostSince] = useState<number | null>(null)
  const [now, setNow] = useState(() => Date.now())
  const card = useRef<HTMLDivElement>(null)
  const cardSize = useCardSize(card)

  const lookup = useMemo<CardLookup>(
    () => ({ objects: state.objectsById, usable: usableIds(state.pending?.decision ?? null) }),
    [state.objectsById, state.pending],
  )
  const base = lessonFacts(state)
  const pageShown = useRef<ReadonlySet<AnchorId>>(new Set())
  const byHand = useMemo(() => manualCount(state.log, state.seat), [state.log, state.seat])
  const facts: Facts = { ...base, page: { shown: pageShown.current, byHand } }

  // The cursor follows the game forward; a reload comes back to it.
  const next = advance(steps, index, facts)
  if (next !== index) {
    setIndex(next)
    setPress(0)
    setTicked(Date.now())
  }
  useEffect(() => saveCursor(game, index), [game, index])

  const ended = index >= steps.length
  useEffect(() => {
    if (ended) onFinish()
  }, [ended, onFinish])

  const view = shownStep(steps, Math.min(index, steps.length - 1), facts)
  const step: LessonStep | undefined = ended ? undefined : steps[view.index]
  const current = ended ? undefined : steps[index]
  const waiting = facts.decision !== null || facts.over

  // The gate holds every input but the current step's.
  const allow = current?.kind === 'do' ? current.allow : undefined
  const gateKey = `${current?.id ?? 'end'}:${JSON.stringify(allow ?? null)}`
  useEffect(() => {
    if (playOn) {
      session.setGate(null)
      return
    }
    session.setGate(ended ? null : { step: current?.id ?? 'end', allow, hint: current?.hint })
    // eslint-disable-next-line react-hooks/exhaustive-deps -- `gateKey` stands for the step and its input
  }, [session, gateKey, playOn, ended])

  // The press the coach lights: the next of the step's presses whose element is there and enabled, else an earlier one.
  const presses = step?.kind === 'do' ? (step.presses ?? []) : []
  let pressAt = Math.min(press, Math.max(0, presses.length - 1))
  if (step?.kind === 'do' && !view.moving) {
    for (let at = pressAt; at > 0; at -= 1) {
      const found = resolve(presses[at] ?? 'pass.primary', lookup)
      if (found.kind === 'found' && found.via === null && enabled(found.element)) break
      pressAt = at - 1
    }
  }
  const targets = step === undefined || view.moving || playOn ? [] : litOf(step, pressAt)
  const page = look(targets, lookup)
  pageShown.current = page.shown

  // Measures again while anything moves, and on every resize.
  const tickNow = useCallback(() => setNow(Date.now()), [])
  useLooking(!ended && !playOn, tickNow)

  // A press inside the lit element moves to the step's next press.
  const litElement = page.lit[0]?.element ?? null
  const litVia = page.lit[0]?.via ?? null
  useEffect(() => {
    if (step?.kind !== 'do' || litElement === null || litVia !== null) return undefined
    const onPress = (event: PointerEvent): void => {
      if (event.target instanceof Node && litElement.contains(event.target))
        setPress((value) => Math.min(Math.max(value, pressAt) + 1, presses.length - 1))
    }
    document.addEventListener('pointerdown', onPress, true)
    return () => document.removeEventListener('pointerdown', onPress, true)
  }, [step, litElement, litVia, pressAt, presses.length])

  // A part of the scrolled preview (its keywords) comes into view; nothing else scrolls, so the table never moves.
  const litElements = page.lit.map((each) => each.element)
  useEffect(() => {
    for (const element of litElements) {
      const preview = element.closest('[data-coach="preview"]')
      if (preview === null || element === preview) continue
      revealIn(element, preview)
    }
  })

  // The preview stays closed while the coach lights anything but the preview itself or a card to look at, so it never covers
  // the band or the part to press (a tap pins it on a phone).
  const looking = step?.allow?.kind === 'look' || targets.includes('preview')
  const held = !playOn && !ended && !looking && targets.length > 0
  useEffect(() => {
    holdPreview(held)
    return () => holdPreview(false)
  }, [held])

  // A do step that waits on a decision it does not fit, or whose part is not on the page, has lost its place.
  const astray =
    current?.kind === 'do' &&
    !view.moving &&
    facts.decision !== null &&
    (!stepFits(current.allow, state.pending?.decision ?? null) || page.lit.length === 0)
  useEffect(() => {
    setLostSince((since) => (astray ? (since ?? Date.now()) : null))
  }, [astray])
  const lost = !playOn && lostSince !== null && now - lostSince >= LOST_MS

  if (playOn) return null

  const rings = page.lit.map((each) => grown(each.box, RING_PAD))
  const phone = page.phone
  const place: Place = coachPlace({
    window: page.window,
    card: {
      width: phone === undefined ? Math.min(CARD_W, page.window.width - 16) : page.window.width - 16,
      height: cardSize.height,
    },
    lit: rings,
    clear: page.clear,
    ...(phone === undefined ? {} : { phone }),
  })
  const bar = phone !== undefined
  const shut = bar && (folded || place.folded)

  const words = (anchor: AnchorId): string => {
    const found = resolve(anchor, lookup)
    if (found.kind === 'found' && found.via === null) {
      const text = wordsOf(found.element)
      if (text !== '') return text
    }
    return anchorLabel(anchor)
  }
  const opener = page.lit.find((each) => each.via !== null)?.via ?? null
  const showRefused = refused !== null && now - refused.at < REFUSED_MS
  const showTick = ticked !== null && now - ticked < TICK_MS
  const count = steps.length
  const position = Math.min(view.index, count - 1)
  const back = step?.kind === 'read' && steps[view.index - 1]?.kind === 'read' && view.index === index
  const last = index === count - 1

  let body: ReactNode
  let foot: ReactNode
  let title: string
  if (lost) {
    title = 'This lesson lost its place'
    body = <p>The game is somewhere the lesson did not expect. Restart the lesson to try again.</p>
    foot = (
      <Button size="sm" className="bg-coach text-on-coach" onClick={onRestart}>
        Restart lesson
      </Button>
    )
  } else if (ended || step === undefined) {
    title = 'Lesson done'
    body = (
      <p>
        {nextTitle === undefined
          ? 'That was the last lesson. You are ready to play.'
          : `Well done. Next up: ${nextTitle}.`}
      </p>
    )
    foot = (
      <div className="flex flex-wrap gap-2">
        {onNextLesson === undefined ? null : (
          <Button
            size="sm"
            data-slot="coach-next-lesson"
            className="bg-coach text-on-coach"
            onClick={onNextLesson}
          >
            Next lesson
          </Button>
        )}
        <Button size="sm" variant="outline" data-slot="coach-back-to-lessons" onClick={onLeave}>
          Back to lessons
        </Button>
      </div>
    )
  } else {
    title = step.title
    const parts = textParts(step.text, words, page.coarse)
    body = (
      <p data-slot="coach-text">
        <Words parts={parts} />
      </p>
    )
    foot =
      step.kind === 'read' && !view.moving ? (
        <div className="flex gap-2">
          {back ? (
            <Button size="sm" variant="ghost" onClick={() => setIndex(index - 1)}>
              Back
            </Button>
          ) : null}
          <Button
            size="sm"
            data-slot="coach-next"
            className="bg-coach text-on-coach hover:bg-coach/90"
            disabled={!waiting}
            onClick={() => setIndex(index + 1)}
          >
            {last ? 'Finish' : 'Next'}
          </Button>
        </div>
      ) : (
        <span data-slot="coach-foot" className="coach-pill rounded-full px-2.5 py-0.5 text-xs font-semibold">
          {view.moving ? 'The game moves on' : step.kind === 'watch' ? 'Watch' : 'Your move'}
        </span>
      )
  }

  const style: CSSProperties = bar
    ? { left: place.x, top: place.y, width: page.window.width - 16 }
    : { left: place.x, top: place.y, width: Math.min(CARD_W, page.window.width - 16) }
  return createPortal(
    <div
      data-slot="coach"
      data-coach-step={lost ? 'lost' : ended ? 'end' : (step?.id ?? 'end')}
      data-coach-kind={lost || ended ? 'end' : view.moving ? 'moving' : (step?.kind ?? 'end')}
      data-coach-lit={page.lit.map((each) => each.via ?? each.target).join('|')}
      data-coach-press={pressAt}
      data-coach-missing={page.missing.join('|')}
      data-coach-selectors={JSON.stringify(page.lit.map(selectorOf))}
      data-coach-place={`${Math.round(place.x)},${Math.round(place.y)}${place.crowded ? ' crowded' : ''}${place.folded ? ' folded' : ''} clear ${page.clear.map((box) => [box.x, box.y, box.width, box.height].map(Math.round).join(' ')).join(';')}`}
      data-coach-allow={step?.kind === 'do' ? (step.allow?.kind ?? '') : ''}
      data-coach-moment={`${facts.turn}:${facts.step ?? ''}:${facts.decision ?? ''}:${state.pending?.id ?? ''}`}
      className="pointer-events-none fixed inset-0 z-[60]"
    >
      <Light rings={rings} targets={page.lit.map((each) => each.target)} size={page.window} />
      <section
        ref={card}
        aria-label={`Lesson: ${lesson.title}`}
        data-slot="coach-card"
        data-shake={showRefused ? refused.at : undefined}
        data-done={showTick || undefined}
        data-folded={shut || undefined}
        style={style}
        className="coach-card pointer-events-auto fixed grid gap-2 rounded-xl bg-surface p-3 text-sm text-foreground shadow-e3"
      >
        <header className={cn('flex items-center gap-2', shut && 'hidden')}>
          <span className="min-w-0 flex-1 truncate text-xs text-muted-foreground">
            {lesson.title}
            {bar ? ` · ${position + 1} of ${count}` : null}
          </span>
          {bar ? null : (
            <span
              aria-label={`Step ${position + 1} of ${count}`}
              role="img"
              className="flex items-center gap-1"
            >
              {steps.map((each, at) => (
                <span
                  key={each.id}
                  data-done={at < position || undefined}
                  data-current={at === position || undefined}
                  className="coach-dot h-1.5 w-1.5 rounded-full"
                />
              ))}
            </span>
          )}
          <DropdownMenu>
            <DropdownMenuTrigger
              render={
                <Button size="icon-sm" variant="ghost" aria-label="Lesson menu" data-slot="coach-menu" />
              }
            >
              <EllipsisIcon aria-hidden="true" />
            </DropdownMenuTrigger>
            <DropdownMenuContent align="end" layer="z-[70]">
              <DropdownMenuItem onClick={onLeave}>Leave lesson</DropdownMenuItem>
              <DropdownMenuItem onClick={onRestart}>Restart lesson</DropdownMenuItem>
              <DropdownMenuItem onClick={() => setPlayOn(true)}>Play on without the coach</DropdownMenuItem>
            </DropdownMenuContent>
          </DropdownMenu>
        </header>
        <h2 className="flex items-center gap-1.5 font-heading text-base font-semibold">
          {showTick ? <CheckIcon aria-hidden="true" className="size-4 text-success" /> : null}
          {bar ? (
            <button
              type="button"
              className="text-left"
              aria-expanded={!shut}
              onClick={() => setFolded(!folded)}
            >
              {title}
            </button>
          ) : (
            title
          )}
        </h2>
        <div className={cn('grid gap-2', shut && 'hidden')}>
          {body}
          {opener === null || lost || ended ? null : (
            <p data-slot="coach-opener" className="text-xs text-muted-foreground">
              Open {ANCHORS[opener].label} first.
            </p>
          )}
        </div>
        {/* The reason of a held input and the foot stay on a folded bar: Next is never out of reach. */}
        <p
          role="status"
          aria-live="polite"
          data-slot="coach-refused"
          className={cn('rounded-md px-2 py-1 text-xs', showRefused ? 'coach-refused' : 'sr-only')}
        >
          {showRefused ? refused.reason : ''}
        </p>
        <div className="flex items-center justify-between gap-2">{foot}</div>
      </section>
    </div>,
    document.body,
  )
}

/** Scrolls the boxes between `element` and `root` that clip it, so the whole of it shows. */
function revealIn(element: HTMLElement, root: Element): void {
  for (let box = element.parentElement; box !== null; box = box.parentElement) {
    if (box.scrollHeight > box.clientHeight + 1) {
      const inner = element.getBoundingClientRect()
      const outer = box.getBoundingClientRect()
      if (inner.bottom > outer.bottom) box.scrollTop += inner.bottom - outer.bottom + 8
      else if (inner.top < outer.top) box.scrollTop -= outer.top - inner.top + 8
    }
    if (box === root) return
  }
}

/** A selector that finds a lit element again, for the tests: its anchor, or its card in its zone. */
function selectorOf(each: { target: Target; via: AnchorId | null; element: HTMLElement }): string {
  if (each.via !== null) return `[data-coach="${each.via}"]`
  const ref = cardRef(each.target)
  if (ref === null) return `[data-coach="${each.target}"]`
  const candidate = each.element.dataset['candidate']
  if (candidate !== undefined) return `[data-slot="card-choices"] button[data-candidate="${candidate}"]`
  return `[data-coach-zone="${ref.zone}"] [data-object-id="${each.element.dataset['objectId'] ?? ''}"]`
}

/** The manual actions `seat` took, from the log. */
function manualCount(log: PlayState['log'], seat: PlayState['seat']): number {
  return log.filter((entry) => {
    const event = entry.event
    if (!('Public' in event)) return false
    const inner = event.Public
    return typeof inner === 'object' && 'ManualAction' in inner && inner.ManualAction.player === seat
  }).length
}
