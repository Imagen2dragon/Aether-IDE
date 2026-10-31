import { contextBridge } from 'electron';

contextBridge.exposeInMainWorld('aetherDesktop', {
  platform: process.platform,
  version: '0.1.0-beta.2',
  daemonUrl: 'ws://127.0.0.1:44888',
});
