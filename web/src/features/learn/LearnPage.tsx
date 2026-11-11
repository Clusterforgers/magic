import { useNavigate } from '@tanstack/react-router'
import {
  BookOpenIcon,
  CircleAlertIcon,
  CircleCheckIcon,
  HandIcon,
  LayersIcon,
  LayoutDashboardIcon,
  ListChecksIcon,
  SwordsIcon,
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
import { fragmentOf, seatLinkPath } from '@/lib/seatLink'
import { api as sharedApi } from '@/net/api'
import type { Api } from '@/net/api'
import { pageTourOf, startPageTour } from './pageTours'
import { doneStore, rememberTutorial, TUTORIALS } from './tutorials'
import type { Tutorial, TutorialId } from './tutorials'

const ICONS: Record<TutorialId, LucideIcon> = {
  game: SwordsIcon,
  table: LayoutDashboardIcon,
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

interface RowProps {
  tutorial: Tutorial
  done: boolean
  busy: boolean
  starting: boolean
  onStart: (tutorial: Tutorial) => void
}

/** One tutorial: its icon, name, length, words, Done mark and Start or Replay. */
function TutorialRow({ tutorial, done, busy, starting, onStart }: RowProps) {
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
          <h2 id={titleId} className="font-heading text-base font-semibold">
            {tutorial.title}
          </h2>
          <p className="text-xs text-muted-foreground tabular-nums">
            About {tutorial.minutes} {tutorial.minutes === 1 ? 'minute' : 'minutes'}
          </p>
        </div>
        {done ? (
          <span
            data-slot="tutorial-done"
            className="flex items-center gap-1 text-xs font-medium text-success"
          >
            <CircleCheckIcon aria-hidden="true" className="size-4" />
            Done
          </span>
        ) : null}
      </div>
      <p className="flex-1 text-sm text-muted-foreground">{tutorial.text}</p>
      <div>
        <Button
          type="button"
          size="sm"
          variant={done ? 'outline' : 'default'}
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
 * `/learn`: the tutorials of the application, each with what it teaches, a Done mark kept in this browser and Start or
 * Replay. A table tutorial starts a game against the practice bot and opens it; a page tour opens its first page.
 */
export function LearnPage({ api = sharedApi }: LearnPageProps) {
  usePageTitle('Learn')
  const navigate = useNavigate()
  const done = useStore(doneStore, (state) => state.done)
  const [starting, setStarting] = useState<TutorialId | null>(null)
  const [failure, setFailure] = useState<string | null>(null)

  const start = async (tutorial: Tutorial): Promise<void> => {
    setFailure(null)
    if (tutorial.kind === 'page') {
      const tour = pageTourOf(tutorial.id)
      if (tour === undefined) return
      startPageTour(tutorial.id)
      if (tour.start !== '/learn') void navigate({ to: tour.start })
      return
    }
    setStarting(tutorial.id)
    try {
      const seat = await api.startTutorial()
      rememberTutorial(seat.game, tutorial.id)
      void navigate({
        to: '/game/$gameId',
        params: { gameId: seat.game },
        hash: fragmentOf(seatLinkPath(seat.game, seat.seat, seat.token)),
      })
    } catch (error) {
      setFailure(error instanceof Error ? error.message : 'The tutorial game could not be started.')
      setStarting(null)
    }
  }

  const finished = done.length
  return (
    <PageShell width="wide" className="flex flex-col gap-5 sm:py-6">
      <header className="grid gap-1">
        <h1 className="text-2xl font-semibold tracking-tight">Learn</h1>
        <p className="text-sm text-muted-foreground">
          Short tutorials of the whole application. Pick any one; each can be replayed.{' '}
          <span data-slot="learn-progress" className="tabular-nums">
            {finished} of {TUTORIALS.length} done.
          </span>
        </p>
      </header>
      {failure === null ? null : (
        <Alert variant="destructive">
          <CircleAlertIcon aria-hidden="true" />
          <AlertTitle>Could not start the tutorial</AlertTitle>
          <AlertDescription>{failure}</AlertDescription>
        </Alert>
      )}
      <ul aria-label="Tutorials" className="grid gap-3 md:grid-cols-2 xl:grid-cols-3">
        {TUTORIALS.map((tutorial) => (
          <TutorialRow
            key={tutorial.id}
            tutorial={tutorial}
            done={done.includes(tutorial.id)}
            busy={starting !== null}
            starting={starting === tutorial.id}
            onStart={(chosen) => void start(chosen)}
          />
        ))}
      </ul>
      <LexiconDialog />
    </PageShell>
  )
}
