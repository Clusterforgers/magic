import { useNavigate } from '@tanstack/react-router'
import {
  BookOpenIcon,
  CircleAlertIcon,
  CircleCheckIcon,
  HandIcon,
  LayersIcon,
  ListChecksIcon,
  UsersIcon,
} from 'lucide-react'
import type { LucideIcon } from 'lucide-react'
import { useState } from 'react'
import { useStore } from 'zustand'
import { PageShell } from '@/components/PageShell'
import { Alert, AlertDescription, AlertTitle } from '@/components/ui/alert'
import { Button } from '@/components/ui/button'
import { LexiconDialog } from '@/features/table/lexicon/LexiconDialog'
import { usePageTitle } from '@/hooks/usePageTitle'
import { cn } from '@/lib/utils'
import { api as sharedApi } from '@/net/api'
import type { Api } from '@/net/api'
import { COURSE } from './course'
import { pageTourOf, startPageTour } from './pageTours'
import type { Lesson, LessonId } from './step'
import { doneStore, startLesson, TUTORIALS } from './tutorials'
import type { AppTutorialId, Tutorial, TutorialId } from './tutorials'

const ICONS: Record<AppTutorialId, LucideIcon> = {
  builder: LayersIcon,
  lobby: UsersIcon,
  review: ListChecksIcon,
  manual: HandIcon,
  lexicon: BookOpenIcon,
}

/** What `LearnPage` takes. */
export interface LearnPageProps {
  /** The client; the shared one by default. */
  api?: Pick<Api, 'startTutorial'>
}

const minutes = (count: number): string => `About ${count} ${count === 1 ? 'minute' : 'minutes'}`

/** The Done mark: an icon and the word. */
function DoneMark() {
  return (
    <span data-slot="tutorial-done" className="flex items-center gap-1 text-xs font-medium text-success">
      <CircleCheckIcon aria-hidden="true" className="size-4" />
      Done
    </span>
  )
}

/** One lesson of the course path: its number, name, minutes, what it teaches and its Done mark. */
function LessonRow({
  lesson,
  number,
  done,
  next,
  busy,
  onStart,
}: {
  lesson: Lesson
  number: number
  done: boolean
  next: boolean
  busy: boolean
  onStart: () => void
}) {
  const titleId = `learn-${lesson.id}`
  return (
    <li
      data-slot="lesson"
      data-lesson={lesson.id}
      data-done={done || undefined}
      data-next={next || undefined}
      aria-labelledby={titleId}
      className={cn('surface flex items-start gap-3 p-3', next && 'ring-2 ring-primary')}
    >
      <span
        aria-hidden="true"
        className={cn(
          'grid size-8 shrink-0 place-items-center rounded-full text-sm font-semibold tabular-nums',
          done ? 'bg-success text-on-primary' : 'bg-muted text-foreground',
        )}
      >
        {number}
      </span>
      <div className="grid min-w-0 flex-1 gap-0.5">
        <h3 id={titleId} className="font-heading text-sm font-semibold">
          {lesson.title}
        </h3>
        <p className="text-sm text-muted-foreground">{lesson.text}</p>
        <p className="text-xs text-muted-foreground tabular-nums">{minutes(lesson.minutes)}</p>
      </div>
      <div className="flex shrink-0 flex-col items-end gap-2">
        {done ? <DoneMark /> : null}
        {next ? null : (
          <Button
            type="button"
            size="sm"
            variant="ghost"
            disabled={busy}
            aria-describedby={titleId}
            onClick={onStart}
          >
            {done ? 'Replay' : 'Start'}
          </Button>
        )}
      </div>
    </li>
  )
}

/** One app tutorial: its icon, name, length, words, Done mark and Start or Replay. */
function TutorialRow({
  tutorial,
  done,
  busy,
  starting,
  onStart,
}: {
  tutorial: Tutorial
  done: boolean
  busy: boolean
  starting: boolean
  onStart: (tutorial: Tutorial) => void
}) {
  const Icon = ICONS[tutorial.id]
  const titleId = `learn-${tutorial.id}`
  return (
    <li
      data-slot="tutorial"
      data-tutorial={tutorial.id}
      data-done={done || undefined}
      aria-labelledby={titleId}
      className="surface flex flex-col gap-3 p-4"
    >
      <div className="flex items-start gap-3">
        <span className="grid size-9 shrink-0 place-items-center rounded-lg bg-muted text-foreground">
          <Icon aria-hidden="true" className="size-5" />
        </span>
        <div className="grid min-w-0 flex-1 gap-1">
          <h3 id={titleId} className="font-heading text-base font-semibold">
            {tutorial.title}
          </h3>
          <p className="text-xs text-muted-foreground tabular-nums">{minutes(tutorial.minutes)}</p>
        </div>
        {done ? <DoneMark /> : null}
      </div>
      <p className="flex-1 text-sm text-muted-foreground">{tutorial.text}</p>
      <div>
        <Button
          type="button"
          size="sm"
          variant="outline"
          disabled={busy}
          aria-describedby={titleId}
          onClick={() => onStart(tutorial)}
        >
          {starting ? 'Starting…' : done ? 'Replay' : 'Start'}
        </Button>
      </div>
    </li>
  )
}

/**
 * `/learn`: the course of seven lessons as a numbered path whose next lesson is the one primary button, then the app's
 * tutorials (the page tours and the manual panel). Done marks are kept in this browser. A lesson starts its own
 * scripted game against the practice bot and opens it; a page tour opens its first page.
 */
export function LearnPage({ api = sharedApi }: LearnPageProps) {
  usePageTitle('Learn')
  const navigate = useNavigate()
  const done = useStore(doneStore, (state) => state.done)
  const [starting, setStarting] = useState<TutorialId | null>(null)
  const [failure, setFailure] = useState<string | null>(null)

  const open = async (id: LessonId | 'manual'): Promise<void> => {
    setFailure(null)
    setStarting(id)
    try {
      const start = await startLesson(api, id)
      void navigate({ to: '/game/$gameId', params: { gameId: start.game }, hash: start.hash })
    } catch (error) {
      setFailure(error instanceof Error ? error.message : 'The lesson could not be started.')
      setStarting(null)
    }
  }

  const startApp = (tutorial: Tutorial): void => {
    if (tutorial.kind === 'table') {
      void open('manual')
      return
    }
    setFailure(null)
    const tour = pageTourOf(tutorial.id)
    if (tour === undefined) return
    startPageTour(tutorial.id)
    if (tour.start !== '/learn') void navigate({ to: tour.start })
  }

  const finished = COURSE.filter((lesson) => done.includes(lesson.id)).length
  const next = COURSE.find((lesson) => !done.includes(lesson.id)) ?? null
  const primary =
    next === null ? 'Replay from the start' : finished === 0 ? 'Start' : `Continue: ${next.title}`
  const busy = starting !== null
  return (
    <PageShell width="wide" className="flex flex-col gap-5 sm:py-6">
      <header className="flex flex-wrap items-end gap-3">
        <div className="grid min-w-0 flex-1 gap-1">
          <h1 className="text-2xl font-semibold tracking-tight">Learn</h1>
          <p className="text-sm text-muted-foreground">
            Seven short lessons at a practice table, each one idea in two or three minutes.{' '}
            <span data-slot="learn-progress" className="tabular-nums">
              {finished} of {COURSE.length} lessons done.
            </span>
          </p>
        </div>
        <Button
          type="button"
          data-slot="learn-next"
          disabled={busy}
          onClick={() => void open((next ?? COURSE[0] ?? { id: 'seat' as const }).id)}
        >
          {starting !== null && starting !== 'manual' ? 'Starting…' : primary}
        </Button>
      </header>
      {failure === null ? null : (
        <Alert variant="destructive">
          <CircleAlertIcon aria-hidden="true" />
          <AlertTitle>Could not start the lesson</AlertTitle>
          <AlertDescription>{failure}</AlertDescription>
        </Alert>
      )}
      <section aria-labelledby="learn-course" className="grid gap-2">
        <h2 id="learn-course" className="text-sm font-semibold text-muted-foreground">
          The course
        </h2>
        <ol aria-label="Lessons" className="grid gap-2 lg:grid-cols-2">
          {COURSE.map((lesson, at) => (
            <LessonRow
              key={lesson.id}
              lesson={lesson}
              number={at + 1}
              done={done.includes(lesson.id)}
              next={next?.id === lesson.id}
              busy={busy}
              onStart={() => void open(lesson.id)}
            />
          ))}
        </ol>
      </section>
      <section aria-labelledby="learn-app" className="grid gap-2">
        <h2 id="learn-app" className="text-sm font-semibold text-muted-foreground">
          The app
        </h2>
        <ul aria-label="Tutorials" className="grid gap-3 md:grid-cols-2 xl:grid-cols-3">
          {TUTORIALS.map((tutorial) => (
            <TutorialRow
              key={tutorial.id}
              tutorial={tutorial}
              done={done.includes(tutorial.id)}
              busy={busy}
              starting={starting === tutorial.id}
              onStart={startApp}
            />
          ))}
        </ul>
      </section>
      <LexiconDialog />
    </PageShell>
  )
}
