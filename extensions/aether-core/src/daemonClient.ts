import * as net from 'net';
import * as fs from 'fs';
import * as path from 'path';
import * as vscode from 'vscode';

export interface IpcEnvelope {
  id: string;
  session_token?: string;
  type: 'Request' | 'Response' | 'Event';
  data: any;
}

export class DaemonClient {
  private client: net.Socket | null = null;
  private pendingRequests = new Map<string, { resolve: (val: any) => void; reject: (err: any) => void }>();
  private buffer = '';
  private port = 44888;
  private sessionToken: string | null = null;

  constructor() {
    const config = vscode.workspace.getConfiguration('aether');
    this.port = config.get<number>('daemonPort', 44888);
    this.loadSessionToken();
  }

  private loadSessionToken(): string | null {
    try {
      const localAppData = process.env.LOCALAPPDATA || (process.env.HOME ? path.join(process.env.HOME, '.aether') : '.');
      const tokenFile = process.env.LOCALAPPDATA
        ? path.join(localAppData, 'AetherIDE', 'daemon_session.token')
        : path.join(localAppData, 'daemon_session.token');

      if (fs.existsSync(tokenFile)) {
        this.sessionToken = fs.readFileSync(tokenFile, 'utf8').trim();
        return this.sessionToken;
      }
    } catch (e) {
      console.warn('[Aether] Error reading daemon session token:', e);
    }
    return null;
  }

  public getSessionToken(): string | null {
    if (!this.sessionToken) {
      this.loadSessionToken();
    }
    return this.sessionToken;
  }

  public async connect(): Promise<void> {
    return new Promise((resolve, reject) => {
      this.client = net.createConnection({ port: this.port, host: '127.0.0.1' }, () => {
        console.log(`[Aether] Connected to native daemon at 127.0.0.1:${this.port}`);
        resolve();
      });

      this.client.on('data', (data) => {
        this.buffer += data.toString('utf8');
        let newlineIdx: number;
        while ((newlineIdx = this.buffer.indexOf('\n')) !== -1) {
          const raw = this.buffer.substring(0, newlineIdx).trim();
          this.buffer = this.buffer.substring(newlineIdx + 1);
          if (raw) {
            this.handleMessage(raw);
          }
        }
      });

      this.client.on('error', (err) => {
        console.warn('[Aether] Daemon connection error:', err.message);
        reject(err);
      });

      this.client.on('close', () => {
        console.log('[Aether] Daemon connection closed');
        this.client = null;
      });
    });
  }

  public async sendRequest(action: string, params: any = {}): Promise<any> {
    if (!this.client || this.client.destroyed) {
      try {
        await this.connect();
      } catch (e) {
        throw new Error(`Cannot connect to Aether daemon: ${e}`);
      }
    }

    const id = `req-${Date.now()}-${Math.random().toString(36).substring(2, 7)}`;
    const envelope = {
      id,
      session_token: this.getSessionToken(),
      type: 'Request',
      data: {
        action,
        params,
      },
    };

    return new Promise((resolve, reject) => {
      this.pendingRequests.set(id, { resolve, reject });
      this.client!.write(JSON.stringify(envelope) + '\n');

      setTimeout(() => {
        if (this.pendingRequests.has(id)) {
          this.pendingRequests.delete(id);
          reject(new Error(`Daemon request ${action} timed out after 30s`));
        }
      }, 30000);
    });
  }

  private handleMessage(raw: string) {
    try {
      const envelope: IpcEnvelope = JSON.parse(raw);
      if (envelope.session_token) {
        this.sessionToken = envelope.session_token;
      }
      if (envelope.type === 'Response' && this.pendingRequests.has(envelope.id)) {
        const { resolve, reject } = this.pendingRequests.get(envelope.id)!;
        this.pendingRequests.delete(envelope.id);
        const resp = envelope.data;
        if (resp.success) {
          resolve(resp.data);
        } else {
          reject(new Error(resp.error || 'Unknown error'));
        }
      }
    } catch (e) {
      console.error('[Aether] Error parsing IPC message:', e);
    }
  }
}
