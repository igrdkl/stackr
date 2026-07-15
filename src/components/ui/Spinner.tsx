import { Loader2 } from 'lucide-react'

interface SpinnerProps {
  size?: number
  strokeWidth?: number
  className?: string
}

export function Spinner({ size = 14, strokeWidth = 2.4, className }: SpinnerProps) {
  return <Loader2 size={size} strokeWidth={strokeWidth} className={`animate-spin ${className ?? ''}`} />
}
