/** @type {import('tailwindcss').Config} */
export default {
  content: ['./src/**/*.{astro,html,js,ts,jsx,tsx,md,mdx}'],
  theme: {
    extend: {
      colors: {
        bg:       '#0d0f0e',
        surface:  { DEFAULT: '#131614', 2: '#191c1a' },
        border:   { DEFAULT: '#242824', 2: '#2e342e' },
        text:     { DEFAULT: '#e8ede9', muted: '#7a8a7c', dim: '#4a574c' },
        accent:   { DEFAULT: '#34d399', dim: '#065f46' },
        calls:    '#60a5fa',
        hasa:     '#f472b6',
        isa:      '#fb923c',
        ref:      '#a78bfa',
      },
      fontFamily: {
        sans: ['Fira Sans', 'sans-serif'],
        mono: ['Fira Code', 'monospace'],
      },
      fontSize: {
        '2xs': '0.65rem',
        xs:    ['0.75rem', { lineHeight: '1.2' }],
      },
      screens: {
        // custom breakpoints matching the existing design
        sm:  '480px',
        md:  '700px',
        lg:  '960px',
        xl:  '1200px',
      },
      keyframes: {
        fadeUp: {
          from: { opacity: '0', transform: 'translateY(16px)' },
          to:   { opacity: '1', transform: 'translateY(0)' },
        },
        blink: {
          '0%, 100%': { opacity: '1' },
          '50%':      { opacity: '0.4' },
        },
      },
      animation: {
        'fade-up':      'fadeUp 0.5s ease both',
        'fade-up-1':    'fadeUp 0.5s 0.1s ease both',
        'fade-up-2':    'fadeUp 0.5s 0.2s ease both',
        'fade-up-3':    'fadeUp 0.5s 0.3s ease both',
        'fade-up-35':   'fadeUp 0.5s 0.35s ease both',
        'fade-up-4':    'fadeUp 0.5s 0.4s ease both',
        blink:          'blink 2s ease infinite',
      },
      maxWidth: {
        content: '900px',
        hero:    '640px',
        prose:   '680px',
      },
    },
  },
  plugins: [],
};
