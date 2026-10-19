import { Link, Outlet } from '@tanstack/react-router'
import { PlusIcon, SparklesIcon } from 'lucide-react'
import { useRef } from 'react'
import { OfflineNotice } from '@/components/OfflineNotice'
import { ThemeMenu } from '@/components/ThemeMenu'
import { buttonVariants } from '@/components/ui/button'
import { TooltipProvider } from '@/components/ui/tooltip'
import { AccountNav, AuthNotice } from '@/features/auth/AccountNav'
import { useAuth } from '@/features/auth/context'
import { cn } from '@/lib/utils'

// A tab of the main navigation: quiet until it is the current page, which gets a bar under it as well as strong text.
const TAB =
  "relative inline-flex h-11 items-center border-b-2 border-transparent px-3 text-sm font-medium text-muted-foreground transition-colors duration-fast ease-out hover:text-foreground aria-[current='page']:border-primary aria-[current='page']:text-foreground sm:h-14"

/** The frame around every page: a header with the brand, the main navigation, the theme and the account, then the page. */
export function AppShell() {
  const main = useRef<HTMLElement>(null)
  const signedIn = useAuth((state) => state.status === 'signed-in')
  return (
    <TooltipProvider>
      <div className="flex min-h-dvh flex-col bg-background text-foreground">
        {/* A button rather than a `#main` link: a hash would replace the seat token in the game address. */}
        <button
          type="button"
          className="sr-only focus:not-sr-only focus:absolute focus:top-2 focus:left-2 focus:z-50 focus:rounded-lg focus:bg-primary focus:px-4 focus:py-2 focus:text-sm focus:font-medium focus:text-on-primary"
          onClick={() => main.current?.focus()}
        >
          Skip to content
        </button>
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
                <Link to="/games" className={TAB}>
                  My games
                </Link>
              ) : null}
              <Link to="/" className={cn(buttonVariants({ size: 'sm' }), 'my-auto ml-auto')}>
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
        <AuthNotice />
        <OfflineNotice />
        <main id="main" ref={main} tabIndex={-1} className="flex-1 outline-none">
          <Outlet />
        </main>
      </div>
    </TooltipProvider>
  )
}
