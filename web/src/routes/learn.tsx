import { createFileRoute } from '@tanstack/react-router'
import { LearnPage } from '@/features/learn/LearnPage'

export const Route = createFileRoute('/learn')({ component: () => <LearnPage /> })
