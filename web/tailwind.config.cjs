/** @type {import('tailwindcss').Config} */
module.exports = {
  content: ['./src/**/*.{html,js,svelte,ts}'],
  theme: {
    extend: {
      fontFamily: {
        sans: ['Inter', 'ui-sans-serif', 'system-ui']
      },
      colors: {
        brand: {
          500: '#6d9eff',
          600: '#4f7cf4',
          700: '#3c63cc'
        }
      }
    }
  },
  plugins: []
};
