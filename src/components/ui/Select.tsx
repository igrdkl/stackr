import type { ReactNode } from 'react'
import { ChevronDown } from 'lucide-react'
import { cn } from '../../lib/cn'
import { selectBase } from '../../lib/styles'

interface SelectProps {
  value: string
  onChange: (v: string) => void
  children: ReactNode
  /** right padding to clear the chevron; default 30px */
  padRight?: number
  className?: string
}

export function Select({ value, onChange, children, padRight = 30, className }: SelectProps) {
  return (
    <div className="relative">
      <select
        value={value}
        onChange={(e) => onChange(e.target.value)}
        className={cn(selectBase, className)}
        style={{ padding: `9px ${padRight}px 9px 11px` }}
      >
        {children}
      </select>
      <ChevronDown
        size={14}
        strokeWidth={2}
        className="absolute right-[10px] top-1/2 -translate-y-1/2 text-fg-dim pointer-events-none"
      />
    </div>
  )
}
