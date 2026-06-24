/// <reference types="vite/client" />

declare module '*.js?url' {
  const src: string;
  export default src;
}
