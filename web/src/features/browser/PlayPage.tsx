import { Link } from '@tanstack/react-router'
import { CircleAlertIcon, PlusIcon } from 'lucide-react'
import { useRef } from 'react'
import { useFitWindow } from '@/components/appChrome'
import { PageShell } from '@/components/PageShell'
import { Alert, AlertDescription, AlertTitle } from '@/components/ui/alert'
import { Button, buttonVariants } from '@/components/ui/button'
import { usePageTitle } from '@/hooks/usePageTitle'
import { api as sharedApi } from '@/net/api'
import type { Api } from '@/net/api'
import { ContinueGames } from './ContinueGames'
import { GuestJoinDialog } from './GuestJoinDialog'
import { JoinWithCode } from './JoinWithCode'
import { OpenGamesPanel } from './OpenGamesPanel'
import { useJoinGame } from './useJoinGame'
import { useOpenGames } from './useOpenGames'

/** What `PlayPage` takes. */
export interface PlayPageProps {
  /** The client; the shared one by default. */
  api?: Pick<Api, 'listLobbies' | 'joinGame' | 'joinByCode'>
}

/**
 * `/`: Play. The open games to join, a button to create a game and a field for a join code. The page fits the window
 * under the header; only the list of games scrolls. A Join, a row's or the code's, takes a seat and opens the game: in
 * one click for a signed-in visitor, after a name in a dialog for a guest.
 */
export function PlayPage({ api = sharedApi }: PlayPageProps) {
  usePageTitle('Play')
  useFitWindow()
  const open = useOpenGames(api)
  const join = useJoinGame(api, open.refresh)
  const code = useRef<HTMLInputElement>(null)
  const refreshButton = useRef<HTMLButtonElement>(null)
  const { asking, failure } = join
  return (
    <PageShell width="wide" className="flex min-h-0 flex-1 flex-col gap-4 sm:py-6">
      <header className="flex flex-wrap items-center justify-between gap-3">
        <h1 className="text-2xl font-semibold tracking-tight">Play</h1>
        <Link to="/new" className={buttonVariants({ size: 'lg' })}>
          <PlusIcon aria-hidden="true" />
          Create game
        </Link>
      </header>
      <ContinueGames />
      <JoinWithCode
        inputRef={code}
        onJoin={join.startCode}
        pending={join.joiningCode}
        busy={join.busy}
        failure={join.codeProblem}
        onEdit={join.dismissCode}
      />
      {failure === null ? null : (
        <Alert variant="destructive">
          <CircleAlertIcon aria-hidden="true" />
          <AlertTitle>{`Could not join ${failure.game.host}'s game`}</AlertTitle>
          <AlertDescription>{failure.text}</AlertDescription>
          {/* Under the words, so the icon's column stays as wide as the icon. */}
          <div className="col-start-2 mt-2">
            <Button
              type="button"
              variant="outline"
              size="sm"
              onClick={() => {
                join.dismiss()
                refreshButton.current?.focus()
              }}
            >
              Dismiss
            </Button>
          </div>
        </Alert>
      )}
      <OpenGamesPanel
        open={open}
        onJoin={join.start}
        joining={join.joining}
        busy={join.busy}
        refreshRef={refreshButton}
        onFocusCode={() => code.current?.focus()}
      />
      {asking === null ? null : (
        <GuestJoinDialog
          title={
            asking.target.kind === 'game' ? `Join ${asking.target.game.host}'s game` : 'Join with a code'
          }
          open={asking.open}
          pending={join.busy}
          failure={join.dialogFailure}
          // The button that asked; if polling took the row away meanwhile, the one control that is always there.
          returnFocus={() => (asking.opener?.isConnected === true ? asking.opener : refreshButton.current)}
          onSubmit={join.submit}
          onCancel={join.cancel}
          onClosed={join.closed}
        />
      )}
    </PageShell>
  )
}
