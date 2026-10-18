import { createFileRoute } from '@tanstack/react-router'
import { RegisterRoute } from '@/features/auth/AuthRoutes'
import { parseAuthSearch } from '@/features/auth/next'

export const Route = createFileRoute('/register')({
  validateSearch: parseAuthSearch,
  component: RegisterRoute,
})
