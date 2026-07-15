interface ProgressBarProps {
  pct: string // "42%"
  color: string
  className?: string
}

export function ProgressBar({ pct, color, className }: ProgressBarProps) {
  return (
    <div
      className={`h-[7px] bg-[#1c2029] rounded-[5px] overflow-hidden ${className ?? ''}`}
    >
      <div
        className="h-full rounded-[5px] transition-[width] duration-300 ease-out"
        style={{ width: pct, background: color }}
      />
    </div>
  )
}
