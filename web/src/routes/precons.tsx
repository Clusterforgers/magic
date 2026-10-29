import { createFileRoute } from '@tanstack/react-router'
import { PreconsPage } from '@/features/precons/PreconsPage'

export const Route = createFileRoute('/precons')({ component: () => <PreconsPage /> })
