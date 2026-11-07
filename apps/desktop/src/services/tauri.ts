import { invoke } from "@tauri-apps/api/core";
import { listen, UnlistenFn } from "@tauri-apps/api/event";
import type {
  AgentPersona,
  AgentTask,
  ApplyProposalResult,
  BrainReconcileReport,
  BuildExecutionResult,
  CheckpointRestorePreview,
  CheckpointRestoreReport,
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
  ProposalReviewPreview,
  ProviderStatus,
  QuarantineRecord,
  RecoveryManifest,
  StorageAnalysisReport,
  StorageCleanupReport,
  StructuredProposal,
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

  static async getTree(rootPath?: string): Promise<WorkspaceNode[]> {
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

  static async setWorkspaceTrust(rootPath: string, isTrusted: boolean): Promise<{ root: string; trusted: boolean }> {
    return invoke("workspace_set_trust", { rootPath, isTrusted });
  }

  // --- Terminal ---
  static async spawnTerminal(cols = 80, rows = 24, cwd?: string): Promise<any> {
    return invoke("terminal_spawn", { cwd, cols, rows });
  }

  static async writeTerminal(idOrData: string, data?: string): Promise<boolean> {
    const text = data !== undefined ? data : idOrData;
    return invoke("terminal_write", { data: text });
  }

  static async resizeTerminal(idOrCols: string | number, colsOrRows: number, rowsOptional?: number): Promise<boolean> {
    const cols = typeof idOrCols === "number" ? idOrCols : colsOrRows;
    const rows = rowsOptional !== undefined ? rowsOptional : colsOrRows;
    return invoke("terminal_resize", { cols, rows });
  }

  static async killTerminal(_id?: string): Promise<boolean> {
    return invoke("terminal_kill");
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
    fileContext?: string,
    requestId?: string,
    conversationId?: string
  ): Promise<string> {
    return invoke("ai_stream_prompt", {
      prompt,
      system,
      model,
      fileContext,
      requestId,
      conversationId,
    });
  }

  static async cancelAiStream(requestId: string): Promise<boolean> {
    return invoke("ai_cancel_stream", { requestId });
  }

  static async previewProposal(proposal: StructuredProposal): Promise<ProposalReviewPreview> {
    return invoke("ai_preview_proposal", { proposal });
  }

  static async applyProposal(proposal: StructuredProposal, force = false): Promise<ApplyProposalResult> {
    return invoke("ai_apply_proposal", { proposal, force });
  }

  // --- AI Agents & Swarm Orchestration ---
  static async listPersonas(): Promise<AgentPersona[]> {
    return invoke("agent_list_personas");
  }

  static async spawnAgentTask(
    role: string,
    objective: string,
    autonomyMode?: string,
    workspaceRoot?: string
  ): Promise<string> {
    return invoke("agent_spawn_task", { role, objective, autonomyMode, workspaceRoot });
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
    targetDir: string,
    bundleId?: string
  ): Promise<string> {
    return invoke("mobile_scaffold_app", {
      params: {
        template,
        name,
        target_dir: targetDir,
        bundle_id: bundleId,
      },
    });
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

  static async cancelBuildTask(taskId: string): Promise<boolean> {
    return invoke("build_cancel_task", { taskId });
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

  static async restoreCheckpointPreview(checkpointId: string): Promise<CheckpointRestorePreview> {
    return invoke("checkpoint_restore_preview", { checkpointId });
  }

  static async restoreCheckpoint(
    checkpointId: string,
    previewToken: string,
    force = false,
    approvedConflicts?: string[]
  ): Promise<CheckpointRestoreReport> {
    return invoke("checkpoint_restore", {
      checkpointId,
      previewToken,
      force,
      approvedConflicts,
    });
  }

  static async listQuarantineRecords(): Promise<QuarantineRecord[]> {
    return invoke("checkpoint_list_quarantine");
  }

  static async cleanupQuarantineRecord(id: string): Promise<boolean> {
    return invoke("checkpoint_cleanup_quarantine", { id });
  }

  static async reconcileQuarantine(): Promise<RecoveryManifest> {
    return invoke("checkpoint_reconcile_quarantine");
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
