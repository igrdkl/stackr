import type { ReactNode } from 'react'

interface ScreenHeaderProps {
  title: string
  subtitle: string
  right?: ReactNode
  className?: string
}

export function ScreenHeader({ title, subtitle, right, className }: ScreenHeaderProps) {
  const head = (
    <div>
      <h1 className="text-[21px] font-semibold tracking-[-.01em]">{title}</h1>
      <p className="text-[13.5px] text-fg-muted2 mt-[6px]">{subtitle}</p>
    </div>
  )
  if (right) {
    return (
      <div className={`flex items-end justify-between ${className ?? ''}`}>
        {head}
        {right}
      </div>
    )
  }
  return <div className={className}>{head}</div>
}
