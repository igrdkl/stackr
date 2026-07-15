interface IconProps {
  size?: number
  className?: string
}

/** Stackr app icon — accent squircle holding the white hexagon + caret mark. */
export function StackrIcon({ size = 30, className }: IconProps) {
  return (
    <svg
      width={size}
      height={size}
      viewBox="0 0 48 48"
      className={className}
      fill="none"
      strokeLinecap="round"
      strokeLinejoin="round"
    >
      <defs>
        <linearGradient id="stackrIconBg" x1="0" y1="0" x2="0" y2="1">
          <stop offset="0" stopColor="#5a89ff" />
          <stop offset="1" stopColor="#3f6ff0" />
        </linearGradient>
      </defs>
      <rect width="48" height="48" rx="11" fill="url(#stackrIconBg)" />
      <polygon points="24,11 35,17.5 35,30.5 24,37 13,30.5 13,17.5" fill="#ffffff" />
      <polyline points="21.5,21 26.5,25 21.5,29" stroke="#4f7fff" strokeWidth="2.6" />
    </svg>
  )
}

/** Bare mark — solid accent hexagon + knockout caret. Use on light/blue surfaces. */
export function StackrMark({ size = 24, className }: IconProps) {
  return (
    <svg
      width={size}
      height={size}
      viewBox="0 0 48 48"
      className={className}
      fill="none"
      strokeLinecap="round"
      strokeLinejoin="round"
    >
      <polygon points="24,5 40,14.5 40,33.5 24,43 8,33.5 8,14.5" fill="#4f7fff" />
      <polyline points="20.5,17 28.5,24 20.5,31" stroke="#0c0e13" strokeWidth="3.8" />
    </svg>
  )
}
