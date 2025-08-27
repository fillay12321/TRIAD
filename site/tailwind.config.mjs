/**** TailwindCSS config for TRIAD site ****/
/** @type {import('tailwindcss').Config} */
export default {
  content: [
    './src/**/*.{astro,html,js,jsx,ts,tsx,md,mdx}',
  ],
  theme: {
    extend: {
      colors: {
        ink: '#0e1116',
        mist: '#c8d0e0',
        accent: '#7c9cff',
        glow: '#9ef0ff',
      },
    },
  },
  plugins: [],
};
