import { createFileRoute } from '@tanstack/react-router'
import { LoginRoute } from '@/features/auth/AuthRoutes'
import { parseAuthSearch } from '@/features/auth/next'

export const Route = createFileRoute('/login')({ validateSearch: parseAuthSearch, component: LoginRoute })
