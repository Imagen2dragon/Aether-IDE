declare module 'electron' {
  export interface BrowserWindow {
    loadFile(path: string): Promise<void>;
    loadURL(url: string): Promise<void>;
    on(event: string, callback: (...args: any[]) => void): void;
  }
  export const BrowserWindow: {
    new (options?: any): BrowserWindow;
    getAllWindows(): BrowserWindow[];
  };
  export const app: any;
  export const dialog: any;
  export const contextBridge: any;
}

declare namespace NodeJS {
  interface Process {
    resourcesPath?: string;
  }
}
