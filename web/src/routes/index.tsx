import { createFileRoute } from '@tanstack/react-router'
import { PlayPage } from '@/features/browser/PlayPage'

export const Route = createFileRoute('/')({ component: () => <PlayPage /> })
