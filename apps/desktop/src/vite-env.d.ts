/// <reference types="vite/client" />

declare global {
  interface Window {
    __TAURI__?: any;
    __aether_dispatchNativeResponse?: (action: string, payload: any) => void;
    term?: any;
  }
}

export {};
