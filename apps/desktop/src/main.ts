import { app, BrowserWindow, dialog } from 'electron';
import * as path from 'path';
import * as child_process from 'child_process';
import * as fs from 'fs';

let mainWindow: BrowserWindow | null = null;
let daemonProcess: child_process.ChildProcess | null = null;

function startDaemon() {
  const isDev = !app.isPackaged;
  const daemonPath = isDev
    ? path.join(__dirname, '../../../rust/target/debug/aether-daemon.exe')
    : path.join((process as any).resourcesPath || path.join(__dirname, '..'), 'bin/aether-daemon.exe');

  if (fs.existsSync(daemonPath)) {
    console.log(`[Aether Desktop] Starting native daemon at: ${daemonPath}`);
    daemonProcess = child_process.spawn(daemonPath, [], {
      detached: false,
      stdio: 'ignore',
    });

    daemonProcess.on('exit', (code) => {
      console.warn(`[Aether Desktop] Daemon exited with code ${code}`);
    });
  } else {
    console.warn(`[Aether Desktop] Daemon binary not found at ${daemonPath}; daemon should run via CLI or release bundle.`);
  }
}

function createWindow() {
  mainWindow = new BrowserWindow({
    width: 1440,
    height: 900,
    title: 'AetherIDE — AI-Native Engineering Ecosystem',
    backgroundColor: '#0B0F19',
    webPreferences: {
      nodeIntegration: false,
      contextIsolation: true,
      preload: path.join(__dirname, 'preload.js'),
    },
  });

  const webAppIndex = path.join(__dirname, '../../web-workspace/dist/index.html');
  if (fs.existsSync(webAppIndex)) {
    mainWindow.loadFile(webAppIndex);
  } else {
    mainWindow.loadURL('http://localhost:5173').catch(() => {
      mainWindow?.loadURL(`data:text/html,<html><body style="background:%230B0F19;color:%2338BDF8;font-family:sans-serif;padding:30px;"><h2>AetherIDE Desktop Workbench</h2><p>Native Rust Daemon running on 127.0.0.1:44888.</p><p>Aether Core Extension loaded.</p></body></html>`);
    });
  }

  mainWindow.on('closed', () => {
    mainWindow = null;
  });
}

app.whenReady().then(() => {
  startDaemon();
  createWindow();

  app.on('activate', () => {
    if (BrowserWindow.getAllWindows().length === 0) {
      createWindow();
    }
  });
});

app.on('window-all-closed', () => {
  if (daemonProcess && !daemonProcess.killed) {
    try {
      daemonProcess.kill();
    } catch (e) {}
  }
  if (process.platform !== 'darwin') {
    app.quit();
  }
});
