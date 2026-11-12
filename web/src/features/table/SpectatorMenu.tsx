import { useState } from 'react'
import { Button } from '@/components/ui/button'
import { useGame, usePlay } from '@/features/game/gameContext'
import { copyText } from '@/lib/clipboard'

/** What the last action said, for the polite live line. */
type Said =
  { kind: 'idle' } | { kind: 'busy' } | { kind: 'done'; text: string } | { kind: 'failed'; text: string }

/**
 * The spectators part of the table menu, for a player: "Copy watch link" puts a new watch link on the clipboard, and
 * the host allows spectators or stops them (which closes their connections). Nothing for a spectator or a table without
 * a server.
 */
export function SpectatorMenu() {
  const { spectators, lookup } = useGame()
  const host = usePlay((state) => state.seat === 0 && state.view?.spectator !== true)
  const [said, setSaid] = useState<Said>({ kind: 'idle' })
  const [allowed, setAllowed] = useState<boolean | null>(null)
  if (spectators === undefined) return null
  const now = allowed ?? lookup.game?.spectators ?? true
  const copy = async () => {
    setSaid({ kind: 'busy' })
    try {
      const link = await spectators.watchLink()
      const copied = await copyText(link)
      setSaid(
        copied
          ? { kind: 'done', text: 'Watch link copied. Whoever opens it watches without a seat.' }
          : { kind: 'failed', text: `Copy this watch link by hand: ${link}` },
      )
    } catch {
      setSaid({
        kind: 'failed',
        text: now ? 'Could not make a watch link. Try again.' : 'The host does not allow spectators.',
      })
    }
  }
  const toggle = async () => {
    setSaid({ kind: 'busy' })
    try {
      const next = await spectators.setAllowed(!now)
      setAllowed(next)
      setSaid({
        kind: 'done',
        text: next ? 'Spectators are allowed.' : 'Spectators are stopped; their links no longer open.',
      })
    } catch {
      setSaid({ kind: 'failed', text: 'Could not change that. Try again.' })
    }
  }
  return (
    <div className="grid gap-2" data-slot="spectator-menu">
      <div className="flex flex-wrap gap-2">
        <Button
          size="sm"
          variant="outline"
          disabled={!now || said.kind === 'busy'}
          onClick={() => void copy()}
        >
          Copy watch link
        </Button>
        {host ? (
          <Button
            size="sm"
            variant="ghost"
            aria-pressed={now}
            disabled={said.kind === 'busy'}
            onClick={() => void toggle()}
          >
            {now ? 'Stop spectators' : 'Allow spectators'}
          </Button>
        ) : null}
      </div>
      <p aria-live="polite" className="text-xs break-words text-muted-foreground">
        {said.kind === 'done' || said.kind === 'failed' ? said.text : null}
      </p>
    </div>
  )
}
