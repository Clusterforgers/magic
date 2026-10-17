import { createFileRoute, Link } from '@tanstack/react-router'

export const Route = createFileRoute('/')({
  component: () => (
    <main className="p-6">
      <Link to="/spike" className="underline">
        Board performance spike
      </Link>
    </main>
  ),
})
