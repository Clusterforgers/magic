import { Link, Outlet } from '@tanstack/react-router'
import { PlusIcon, SparklesIcon } from 'lucide-react'
import { useRef } from 'react'
import { useIsFitWindow, useIsFullBleed } from '@/components/appChrome'
import { FlashNotice } from '@/components/FlashNotice'
import { OfflineNotice } from '@/components/OfflineNotice'
import { ThemeMenu } from '@/components/ThemeMenu'
import { buttonVariants } from '@/components/ui/button'
import { TooltipProvider } from '@/components/ui/tooltip'
import { AccountNav, AuthNotice } from '@/features/auth/AccountNav'
import { useAuth } from '@/features/auth/context'
import { cn } from '@/lib/utils'

// A tab of the main navigation: quiet until it is the current page, which gets a bar under it as well as strong text.
const TAB =
  "relative inline-flex h-11 items-center border-b-2 whitespace-nowrap border-transparent px-3 text-sm font-medium text-muted-foreground transition-colors duration-fast ease-out hover:text-foreground aria-[current='page']:border-primary aria-[current='page']:text-foreground sm:h-14"

/** The brand, the main navigation, the theme and the account. */
function ShellHeader() {
  const signedIn = useAuth((state) => state.status === 'signed-in')
  return (
    <header className="border-b bg-surface">
      <div className="flex flex-wrap items-center gap-x-1 px-4 sm:px-6">
        <Link
          to="/"
          className="flex h-12 items-center gap-2 pr-3 text-base font-semibold tracking-tight sm:h-14"
        >
          <SparklesIcon aria-hidden="true" className="size-5 text-primary" />
          Magic
        </Link>
        <nav
          aria-label="Main"
          className="order-3 flex w-full items-center border-t sm:order-none sm:w-auto sm:flex-1 sm:border-t-0"
        >
          <Link to="/" className={TAB}>
            Play
          </Link>
          {signedIn ? (
            <>
              <Link to="/decks" className={TAB}>
                Decks
              </Link>
              <Link to="/games" className={TAB}>
                My games
              </Link>
            </>
          ) : null}
          <Link to="/new" className={cn(buttonVariants({ size: 'sm' }), 'my-auto ml-auto')}>
            <PlusIcon aria-hidden="true" />
            Create
          </Link>
        </nav>
        <div className="ml-auto flex items-center gap-1 sm:ml-0">
          <ThemeMenu />
          <AccountNav />
        </div>
      </div>
    </header>
  )
}

/**
 * The frame around every page: a header with the brand, the main navigation, the theme and the account, then the page.
 * A page that takes the whole window (the game table, `useFullBleed`) has no header and no scrolling: it is exactly
 * the window, and a notice that must be seen (offline) floats over it. A page that fits the window (`useFitWindow`)
 * keeps the header: the shell is the window high and `main` is the rest, which the page fills and scrolls inside.
 */
export function AppShell() {
  const main = useRef<HTMLElement>(null)
  const bleed = useIsFullBleed()
  const fit = useIsFitWindow() && !bleed
  return (
    <TooltipProvider>
      <div
        data-bleed={bleed || undefined}
        data-fit={fit || undefined}
        className={cn(
          'flex flex-col bg-background text-foreground',
          bleed ? 'h-dvh overflow-hidden' : fit ? 'h-dvh' : 'min-h-dvh',
        )}
      >
        {/* A button rather than a `#main` link: a hash would replace the seat token in the game address. */}
        <button
          type="button"
          className="sr-only focus:not-sr-only focus:absolute focus:top-2 focus:left-2 focus:z-50 focus:rounded-lg focus:bg-primary focus:px-4 focus:py-2 focus:text-sm focus:font-medium focus:text-on-primary"
          onClick={() => main.current?.focus()}
        >
          Skip to content
        </button>
        {bleed ? (
          <div className="pointer-events-none fixed inset-x-0 top-11 z-50">
            <div className="pointer-events-auto">
              <OfflineNotice />
            </div>
          </div>
        ) : (
          <>
            <ShellHeader />
            <AuthNotice />
            <OfflineNotice />
            <FlashNotice />
          </>
        )}
        <main
          id="main"
          ref={main}
          tabIndex={-1}
          // A window too small for the page's own parts scrolls here, not the page.
          className={cn(
            'flex-1 outline-none',
            bleed && 'min-h-0',
            fit && 'flex min-h-0 flex-col overflow-y-auto',
          )}
        >
          <Outlet />
        </main>
      </div>
    </TooltipProvider>
  )
}
