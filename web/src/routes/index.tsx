import { createFileRoute } from '@tanstack/react-router'
import { CreateGamePage } from '@/features/create/CreateGamePage'

export const Route = createFileRoute('/')({ component: CreateGamePage })
