import { invoke } from "@tauri-apps/api/core";
import { listen, UnlistenFn } from "@tauri-apps/api/event";
import type {
  AgentPersona,
  AgentTask,
  BrainReconcileReport,
  BuildExecutionResult,
  DetectedBuildSystem,
  Diagnostic,
  EasCliStatus,
  ExpoDevServerInfo,
  GitStatusResult,
  ImpactReport,
  MobileDeviceInfo,
  ModelMetadata,
  ProjectBrainGraph,
  ProjectMemoryNote,
  ProviderStatus,
  StorageAnalysisReport,
  StorageCleanupReport,
  WorkspaceCheckpoint,
  WorkspaceNode,
  WorkspaceSummary,
} from "../types/workbench";

export class TauriService {
  // --- Workspace ---
  static async openFolder(path?: string): Promise<WorkspaceSummary> {
    return invoke("workspace_open_folder", { path });
  }

  static async addFolder(path?: string): Promise<WorkspaceSummary> {
    return invoke("workspace_add_folder", { path });
  }

  static async getTree(rootPath: string): Promise<WorkspaceNode[]> {
    return invoke("workspace_get_tree", { rootPath });
  }

  static async openFile(path?: string): Promise<{ path: string; content: string }> {
    return invoke("workspace_open_file", { path });
  }

  static async saveFile(path: string, content: string): Promise<boolean> {
    return invoke("workspace_save_file", { path, content });
  }

  static async createFile(path: string, content?: string): Promise<boolean> {
    return invoke("workspace_create_file", { path, content });
  }

  static async createFolder(path: string): Promise<boolean> {
    return invoke("workspace_create_folder", { path });
  }

  static async renameEntry(oldPath: string, newPath: string): Promise<boolean> {
    return invoke("workspace_rename_entry", { oldPath, newPath });
  }

  static async deleteEntry(path: string): Promise<boolean> {
    return invoke("workspace_delete_entry", { path });
  }

  static async revealInExplorer(path: string): Promise<boolean> {
    return invoke("workspace_reveal_in_explorer", { path });
  }

  // --- Terminal ---
  static async spawnTerminal(cols = 80, rows = 24): Promise<string> {
    return invoke("terminal_spawn", { cols, rows });
  }

  static async writeTerminal(id: string, data: string): Promise<boolean> {
    return invoke("terminal_write", { id, data });
  }

  static async resizeTerminal(id: string, cols: number, rows: number): Promise<boolean> {
    return invoke("terminal_resize", { id, cols, rows });
  }

  static async killTerminal(id: string): Promise<boolean> {
    return invoke("terminal_kill", { id });
  }

  // --- System ---
  static async ping(): Promise<string> {
    return invoke("app_ping");
  }

  static async getVersion(): Promise<{
    version: string;
    name: string;
    platform: string;
    arch: string;
    framework: string;
  }> {
    return invoke("app_version");
  }

  static async getDoctorReport(): Promise<{
    all_healthy: boolean;
    diagnostics: Array<{ name: string; status: string; message: string; remediation?: string }>;
    timestamp: string;
  }> {
    return invoke("app_doctor");
  }

  // --- Project Brain ---
  static async scanBrain(path?: string): Promise<BrainReconcileReport> {
    return invoke("brain_scan_workspace", { path });
  }

  static async getBrainGraph(): Promise<ProjectBrainGraph> {
    return invoke("brain_get_graph");
  }

  static async calculateImpact(targetFile: string): Promise<ImpactReport> {
    return invoke("brain_calculate_impact", { targetFile });
  }

  // --- Project Memory ---
  static async saveMemoryNote(
    title: string,
    content: string,
    tags: string[] = []
  ): Promise<ProjectMemoryNote> {
    return invoke("memory_save_note", { title, content, tags });
  }

  static async getMemoryNotes(): Promise<ProjectMemoryNote[]> {
    return invoke("memory_get_notes");
  }

  static async deleteMemoryNote(id: string): Promise<boolean> {
    return invoke("memory_delete_note", { id });
  }

  // --- AI Assistant & Model Gateway ---
  static async listModels(): Promise<ModelMetadata[]> {
    return invoke("ai_list_models");
  }

  static async getProviders(): Promise<Record<string, ProviderStatus>> {
    return invoke("ai_get_providers");
  }

  static async completeAi(
    prompt: string,
    system?: string,
    model?: string,
    maxTokens?: number
  ): Promise<{ content: string; model: string; tokens_used: number; finish_reason: string }> {
    return invoke("ai_complete", { prompt, system, model, maxTokens });
  }

  static async streamPrompt(
    prompt: string,
    system?: string,
    model?: string,
    fileContext?: string
  ): Promise<string> {
    return invoke("ai_stream_prompt", { prompt, system, model, fileContext });
  }

  // --- AI Agents & Swarm Orchestration ---
  static async listPersonas(): Promise<AgentPersona[]> {
    return invoke("agent_list_personas");
  }

  static async spawnAgentTask(
    role: string,
    objective: string,
    autonomyMode?: string
  ): Promise<string> {
    return invoke("agent_spawn_task", { role, objective, autonomyMode });
  }

  static async getAgentTask(taskId: string): Promise<AgentTask> {
    return invoke("agent_get_task", { taskId });
  }

  static async executeAgentStep(taskId: string): Promise<AgentTask> {
    return invoke("agent_execute_step", { taskId });
  }

  static async cancelAgentTask(taskId: string): Promise<AgentTask> {
    return invoke("agent_cancel_task", { taskId });
  }

  // --- Mobile Studio ---
  static async discoverMobileDevices(): Promise<MobileDeviceInfo[]> {
    return invoke("mobile_discover_devices");
  }

  static async checkEas(projectDir?: string): Promise<EasCliStatus> {
    return invoke("mobile_check_eas", { projectDir });
  }

  static async getDevServerInfo(
    projectDir?: string,
    port?: number
  ): Promise<ExpoDevServerInfo> {
    return invoke("mobile_get_dev_server", { projectDir, port });
  }

  static async startMetro(projectDir?: string, port?: number): Promise<any> {
    return invoke("mobile_start_metro", { projectDir, port });
  }

  static async stopMetro(): Promise<boolean> {
    return invoke("mobile_stop_metro");
  }

  static async scaffoldMobileApp(
    template: string,
    name: string,
    targetDir: string
  ): Promise<string> {
    return invoke("mobile_scaffold_app", { template, name, targetDir });
  }

  // --- Build Systems ---
  static async detectBuildSystems(path?: string): Promise<DetectedBuildSystem[]> {
    return invoke("build_detect_systems", { path });
  }

  static async runBuildTask(
    command: string,
    workingDir?: string
  ): Promise<BuildExecutionResult> {
    return invoke("build_run_task", { command, workingDir });
  }

  // --- Storage Management ---
  static async analyzeStorage(path?: string): Promise<StorageAnalysisReport> {
    return invoke("storage_analyze", { path });
  }

  static async cleanupStorage(
    targets: string[],
    dryRun = false
  ): Promise<StorageCleanupReport> {
    return invoke("storage_cleanup", { targets, dryRun });
  }

  // --- Checkpoint Engine ---
  static async createCheckpoint(
    description: string,
    files?: string[]
  ): Promise<WorkspaceCheckpoint> {
    return invoke("checkpoint_create", { description, files });
  }

  static async listCheckpoints(): Promise<WorkspaceCheckpoint[]> {
    return invoke("checkpoint_list");
  }

  static async restoreCheckpoint(checkpointId: string): Promise<boolean> {
    return invoke("checkpoint_restore", { checkpointId });
  }

  // --- Git Engine ---
  static async getGitStatus(path?: string): Promise<GitStatusResult> {
    return invoke("git_get_status", { path });
  }

  static async stageGitFile(file: string, path?: string): Promise<boolean> {
    return invoke("git_stage_file", { file, path });
  }

  static async unstageGitFile(file: string, path?: string): Promise<boolean> {
    return invoke("git_unstage_file", { file, path });
  }

  static async commitGit(message: string, path?: string): Promise<string> {
    return invoke("git_commit", { message, path });
  }

  static async getGitDiff(staged = false, path?: string): Promise<string> {
    return invoke("git_get_diff", { staged, path });
  }

  // --- LSP Client ---
  static async getDiagnostics(
    path?: string
  ): Promise<Record<string, Diagnostic[]>> {
    return invoke("lsp_get_diagnostics", { path });
  }

  // --- Event Listeners Helper ---
  static onEvent<T>(event: string, callback: (payload: T) => void): Promise<UnlistenFn> {
    return listen<T>(event, (e) => callback(e.payload));
  }
}
