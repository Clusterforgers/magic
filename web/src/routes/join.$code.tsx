import { createFileRoute } from '@tanstack/react-router'
import { JoinCodeRoute } from '@/features/browser/JoinCodePage'

export const Route = createFileRoute('/join/$code')({ component: JoinCodeRoute })
