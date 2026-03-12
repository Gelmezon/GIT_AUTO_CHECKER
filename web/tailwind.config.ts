import type { Config } from 'tailwindcss'

export default {
  content: ['./index.html', './src/**/*.{vue,ts}'],
  theme: {
    extend: {
      colors: {
        paper: '#f4efe8',
        ink: '#201a15',
        clay: '#a8562a',
        sand: '#e8ddcf',
        leaf: '#2d6a4f',
        blush: '#c96c50',
      },
      boxShadow: {
        card: '0 24px 60px rgba(77, 52, 35, 0.10)',
      },
      borderRadius: {
        panel: '24px',
      },
      fontFamily: {
        display: ['"Cormorant Garamond"', 'Georgia', 'serif'],
        body: ['"IBM Plex Sans"', '"Segoe UI"', 'sans-serif'],
      },
    },
  },
  plugins: [],
} satisfies Config
