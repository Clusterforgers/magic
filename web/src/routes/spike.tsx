import { createFileRoute } from '@tanstack/react-router'
import { SpikePage } from '@/features/spike/SpikePage'

const CONFIG = { players: 4, permanentsPerPlayer: 37, handSize: 7, seed: 1 }

export const Route = createFileRoute('/spike')({ component: () => <SpikePage config={CONFIG} /> })
