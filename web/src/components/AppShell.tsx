import { Link, Outlet } from '@tanstack/react-router'

/** The frame around every page: a slim header with the way home, and the page itself. */
export function AppShell() {
  return (
    <div className="flex min-h-dvh flex-col bg-background text-foreground">
      <header className="flex h-12 items-center border-b px-4 sm:px-6">
        <Link to="/" className="font-heading text-sm font-semibold tracking-tight">
          Magic
        </Link>
      </header>
      <main className="flex-1">
        <Outlet />
      </main>
    </div>
  )
}
