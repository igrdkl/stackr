import type { CSSProperties, ReactNode } from 'react'

interface MonogramProps {
  size: number
  radius: number
  bg: string
  color: string
  fontSize: number
  mono?: boolean
  bold?: boolean
  border?: string
  children: ReactNode
}

/** Colored letter tile used for servers, PHP versions, projects, frameworks, db/cache cards. */
export function Monogram({
  size,
  radius,
  bg,
  color,
  fontSize,
  mono,
  bold,
  border,
  children,
}: MonogramProps) {
  const style: CSSProperties = {
    width: size,
    height: size,
    borderRadius: radius,
    background: bg,
    color,
    fontSize,
    fontWeight: bold ? 700 : 600,
    fontFamily: mono ? "'JetBrains Mono', monospace" : undefined,
    border,
  }
  return (
    <div className="flex items-center justify-center shrink-0" style={style}>
      {children}
    </div>
  )
}
