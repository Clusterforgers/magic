import { createFileRoute } from '@tanstack/react-router'
import { DelveRoute } from '@/features/delves/DelveRoute'

// The trailing underscore keeps this route out of `delves.tsx`, which is a page of its own and has no outlet.
export const Route = createFileRoute('/delves_/$delveId')({ component: DelveRoute })
