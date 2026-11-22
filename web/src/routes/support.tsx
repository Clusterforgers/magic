import { createFileRoute } from '@tanstack/react-router'
import { CardSupportPage } from '@/features/cardSupport/CardSupportPage'

export const Route = createFileRoute('/support')({ component: () => <CardSupportPage /> })
