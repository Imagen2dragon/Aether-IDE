"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.DaemonClient = void 0;
const net = require("net");
const fs = require("fs");
const path = require("path");
const vscode = require("vscode");
class DaemonClient {
    client = null;
    pendingRequests = new Map();
    buffer = '';
    port = 44888;
    sessionToken = null;
    constructor() {
        const config = vscode.workspace.getConfiguration('aether');
        this.port = config.get('daemonPort', 44888);
        this.loadSessionToken();
    }
    loadSessionToken() {
        try {
            const localAppData = process.env.LOCALAPPDATA || (process.env.HOME ? path.join(process.env.HOME, '.aether') : '.');
            const tokenFile = process.env.LOCALAPPDATA
                ? path.join(localAppData, 'AetherIDE', 'daemon_session.token')
                : path.join(localAppData, 'daemon_session.token');
            if (fs.existsSync(tokenFile)) {
                this.sessionToken = fs.readFileSync(tokenFile, 'utf8').trim();
                return this.sessionToken;
            }
        }
        catch (e) {
            console.warn('[Aether] Error reading daemon session token:', e);
        }
        return null;
    }
    getSessionToken() {
        if (!this.sessionToken) {
            this.loadSessionToken();
        }
        return this.sessionToken;
    }
    async connect() {
        return new Promise((resolve, reject) => {
            this.client = net.createConnection({ port: this.port, host: '127.0.0.1' }, () => {
                console.log(`[Aether] Connected to native daemon at 127.0.0.1:${this.port}`);
                resolve();
            });
            this.client.on('data', (data) => {
                this.buffer += data.toString('utf8');
                let newlineIdx;
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
    async sendRequest(action, params = {}) {
        if (!this.client || this.client.destroyed) {
            try {
                await this.connect();
            }
            catch (e) {
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
            this.client.write(JSON.stringify(envelope) + '\n');
            setTimeout(() => {
                if (this.pendingRequests.has(id)) {
                    this.pendingRequests.delete(id);
                    reject(new Error(`Daemon request ${action} timed out after 30s`));
                }
            }, 30000);
        });
    }
    handleMessage(raw) {
        try {
            const envelope = JSON.parse(raw);
            if (envelope.session_token) {
                this.sessionToken = envelope.session_token;
            }
            if (envelope.type === 'Response' && this.pendingRequests.has(envelope.id)) {
                const { resolve, reject } = this.pendingRequests.get(envelope.id);
                this.pendingRequests.delete(envelope.id);
                const resp = envelope.data;
                if (resp.success) {
                    resolve(resp.data);
                }
                else {
                    reject(new Error(resp.error || 'Unknown error'));
                }
            }
        }
        catch (e) {
            console.error('[Aether] Error parsing IPC message:', e);
        }
    }
}
exports.DaemonClient = DaemonClient;
//# sourceMappingURL=daemonClient.js.map