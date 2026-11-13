import { createFileRoute } from '@tanstack/react-router'
import { DelvesPage } from '@/features/delves/DelvesPage'

export const Route = createFileRoute('/delves')({ component: () => <DelvesPage /> })
