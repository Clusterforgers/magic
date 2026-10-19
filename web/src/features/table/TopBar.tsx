import { Link, useRouter } from '@tanstack/react-router'
import { SparklesIcon } from 'lucide-react'
import { useState } from 'react'
import { buttonVariants } from '@/components/ui/button'
import { usePlay } from '@/features/game/gameContext'
import { cn } from '@/lib/utils'
import { ConcedeDialog } from './ConcedeDialog'
import { canConcede } from './selectors'
import { SideSheetButton } from './SideTabs'
import { TableMenu } from './TableMenu'
import type { TableMode } from './layout'
import { TurnBadge } from './TurnBadge'
import { StepLadder } from './turn/StepLadder'

const HOME = cn(
  buttonVariants({ variant: 'ghost', size: 'sm' }),
  'shrink-0 gap-1.5 px-2 text-base font-semibold',
)

/** Home: a router link where there is a router, a plain one where there is none (a table drawn on its own). */
function HomeLink() {
  // The router's context is null outside a router.
  const router = useRouter({ warn: false }) as ReturnType<typeof useRouter> | null | undefined
  const content = (
    <>
      <SparklesIcon aria-hidden="true" className="text-primary" />
      <span className="max-sm:sr-only">Magic</span>
    </>
  )
  return router === undefined || router === null ? (
    <a href="/" aria-label="Home" className={HOME}>
      {content}
    </a>
  ) : (
    <Link to="/" aria-label="Home" className={HOME}>
      {content}
    </Link>
  )
}

interface TopBarProps {
  mode: TableMode
  onMode: (mode: TableMode) => void
  pinnedFocus: boolean
  onFollow: () => void
  /** The stop ladder and Full Control sit in the menu, not in the controls dock. */
  stopsInMenu: boolean
  /** The bar is 32 px high, on a phone on its side: the buttons are smaller. */
  compact?: boolean
}

/**
 * The top bar of the table: home, the turn badge, the button of the side sheet on a narrow window, and the table menu
 * (layout, stops and Full Control where the dock has no room, theme, Concede). 44 px high, 32 px on a phone on its
 * side.
 */
export function TopBar({ mode, onMode, pinnedFocus, onFollow, stopsInMenu, compact = false }: TopBarProps) {
  const [menu, setMenu] = useState(false)
  const [conceding, setConceding] = useState(false)
  // The question goes with the right to concede, such as when the game ends while it is open.
  const allowed = usePlay(canConcede)
  return (
    <header data-region="bar" className="flex min-h-0 min-w-0 items-center gap-2 border-b bg-surface px-2">
      <HomeLink />
      <TurnBadge />
      <StepLadder />
      <span className="ml-auto" />
      <SideSheetButton compact={compact} />
      <TableMenu
        compact={compact}
        mode={mode}
        onMode={onMode}
        pinnedFocus={pinnedFocus}
        onFollow={onFollow}
        stops={stopsInMenu}
        open={menu}
        onOpenChange={setMenu}
        onConcede={() => {
          setMenu(false)
          setConceding(true)
        }}
      />
      {allowed ? <ConcedeDialog open={conceding} onOpenChange={setConceding} /> : null}
    </header>
  )
}
