import { Link } from '@tanstack/react-router'
import { PlusIcon } from 'lucide-react'
import { useRef } from 'react'
import { useFitWindow } from '@/components/appChrome'
import { PageShell } from '@/components/PageShell'
import { buttonVariants } from '@/components/ui/button'
import { usePageTitle } from '@/hooks/usePageTitle'
import { api as sharedApi } from '@/net/api'
import type { Api } from '@/net/api'
import { JoinWithCode } from './JoinWithCode'
import type { OnJoin } from './joinTarget'
import { OpenGamesPanel } from './OpenGamesPanel'
import { useOpenGames } from './useOpenGames'

/** What `PlayPage` takes. */
export interface PlayPageProps {
  /** The client; the shared one by default. */
  api?: Pick<Api, 'listLobbies'>
  /** Joins a game of the list or the game a code names. A page without it has every Join disabled and says why. */
  onJoin?: OnJoin
}

/**
 * `/`: Play. The open games to join, a button to create a game and a field for a join code. The page fits the window
 * under the header; only the list of games scrolls.
 */
export function PlayPage({ api = sharedApi, onJoin }: PlayPageProps) {
  usePageTitle('Play')
  useFitWindow()
  const open = useOpenGames(api)
  const code = useRef<HTMLInputElement>(null)
  return (
    <PageShell width="wide" className="flex min-h-0 flex-1 flex-col gap-4 sm:py-6">
      <header className="flex flex-wrap items-center justify-between gap-3">
        <h1 className="text-2xl font-semibold tracking-tight">Play</h1>
        <Link to="/new" className={buttonVariants({ size: 'lg' })}>
          <PlusIcon aria-hidden="true" />
          Create game
        </Link>
      </header>
      <JoinWithCode onJoin={onJoin} inputRef={code} />
      <OpenGamesPanel open={open} onJoin={onJoin} onFocusCode={() => code.current?.focus()} />
    </PageShell>
  )
}
