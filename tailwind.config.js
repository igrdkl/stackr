/** @type {import('tailwindcss').Config} */
export default {
  content: ['./index.html', './src/**/*.{ts,tsx}'],
  theme: {
    extend: {
      fontFamily: {
        sans: ['"Geist Sans"', 'Geist', '-apple-system', 'BlinkMacSystemFont', 'sans-serif'],
        mono: ['"JetBrains Mono"', 'ui-monospace', 'monospace'],
      },
      colors: {
        // surfaces
        app: '#0f1117',
        sidebar: '#0c0e13',
        card: '#14171f',
        inset: '#11141b',
        term: '#0a0c10',
        control: '#161a22',
        control2: '#1a1e27',
        chip: '#1a1e27',
        hover: '#1c212b',
        hover2: '#20252f',
        navactive: '#1b2030',
        navhover: '#15181f',
        // borders
        line: {
          DEFAULT: '#222734',
          subtle: '#1e222c',
          soft: '#20242f',
          faint: '#1a1d25',
          input: '#262c38',
          chip: '#242a35',
          ghost: '#2a3140',
          hover: '#2c3340',
          hover2: '#39414f',
        },
        // accent
        accent: {
          DEFAULT: '#4f7fff',
          hover: '#3f6ff0',
          text: '#7a9bff',
          link: '#5f86ff',
        },
        // text / foreground
        fg: {
          DEFAULT: '#e6e8ee',
          bright: '#e8eaf0',
          soft: '#dfe3ea',
          muted: '#9298a6',
          muted2: '#8b91a0',
          dim: '#6f7686',
          dim2: '#878d9c',
          faint: '#5c626f',
          faint2: '#565d6b',
        },
        // status
        ok: '#3fb950',
        warn: '#d9a93a',
        warn2: '#caa14a',
        danger: '#f1645a',
        danger2: '#f85149',
        stopped: '#7c8493',
        success: {
          DEFAULT: '#1f8a4d',
          hover: '#1c7a44',
        },
      },
      keyframes: {
        spin: { to: { transform: 'rotate(360deg)' } },
        fadeup: {
          from: { opacity: '0', transform: 'translateY(8px)' },
          to: { opacity: '1', transform: 'translateY(0)' },
        },
      },
      animation: {
        spin: 'spin .8s linear infinite',
        fadeup: 'fadeup .2s ease',
      },
    },
  },
  plugins: [],
}
