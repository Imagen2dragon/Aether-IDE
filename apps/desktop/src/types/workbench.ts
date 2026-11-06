export type ActiveSidebarView =
  | "explorer"
  | "brain"
  | "git"
  | "agents"
  | "mobile"
  | "build"
  | "settings";

export type BottomPanelTab = "terminal" | "problems" | "output" | "doctor";

export interface WorkspaceNode {
  name: string;
  path: string;
  is_directory: boolean;
  size?: number;
  modified?: string;
  children?: WorkspaceNode[];
}

export interface WorkspaceSummary {
  workspace_id?: string;
  name?: string;
  roots: any[];
  root_count?: number;
  primary_path?: string;
  is_trusted?: boolean;
  open_editors?: string[];
  active_editor?: string;
}

export interface OpenFileTab {
  path: string;
  name: string;
  content: string;
  isDirty: boolean;
  language: string;
}

// Project Brain
export interface BrainNode {
  id: string;
  label: string;
  kind: string;
  metadata: Record<string, string>;
}

export interface BrainEdge {
  source: string;
  target: string;
  relationship: string;
}

export interface ProjectBrainGraph {
  nodes: BrainNode[];
  edges: BrainEdge[];
}

export interface BrainReconcileReport {
  nodes_total: number;
  nodes_added: number;
  nodes_removed: number;
  edges_total: number;
  is_complete: boolean;
  errors: string[];
}

export interface ImpactReport {
  target_file: string;
  direct_dependents: string[];
  transitive_dependents: string[];
  affected_tests: string[];
  risk_score: number;
}

// Project Memory
export interface ProjectMemoryNote {
  id: string;
  title: string;
  content: string;
  tags: string[];
  created_at: string;
  updated_at: string;
}

// AI Gateway
export interface ModelMetadata {
  id: string;
  provider: string;
  name: string;
  context_window: number;
  supports_vision: boolean;
  supports_reasoning: boolean;
  supports_tools: boolean;
  is_local: boolean;
}

export type ProviderStatus =
  | "NotConfigured"
  | "Configured"
  | "AuthenticationFailed"
  | "ConnectionFailed"
  | "ModelAvailable"
  | "RequestSuccessful"
  | "RateLimited"
  | "TemporarilyUnavailable";

export interface CompletionResponse {
  content: string;
  model: string;
  tokens_used: number;
  finish_reason: string;
}

// Safe AI Code Editing
export interface StructuredProposal {
  proposal_id: string;
  workspace_id: string;
  relative_path: string;
  operation: "modify" | "create" | "delete";
  base_hash: string;
  unified_diff: string;
  proposed_content: string;
}

export interface ProposalReviewPreview {
  proposal_id: string;
  relative_path: string;
  operation: string;
  base_hash: string;
  current_disk_hash: string;
  has_conflict: boolean;
  original_content: string;
  proposed_content: string;
  unified_diff: string;
}

export interface ApplyProposalResult {
  success: boolean;
  relative_path: string;
  new_hash: string;
  message: string;
  backup_checkpoint_id?: string;
}

// AI Agents
export interface AgentPersona {
  role_id: string;
  display_name: string;
  description: string;
  system_prompt: string;
  default_tools: string[];
}

export interface AgentTask {
  id: string;
  role: string;
  objective: string;
  status: "Queued" | "Pending" | "Planning" | "Executing" | "WaitingForApproval" | "Running" | "Paused" | "Completed" | "Failed" | "Cancelled";
  current_step: number;
  total_steps: number;
  logs: string[];
  created_at: string;
  updated_at: string;
}

// Mobile Studio
export interface MobileDeviceInfo {
  id: string;
  name: string;
  platform: "android" | "ios";
  device_type: "emulator" | "physical";
  state: "online" | "offline" | "booting";
}

export interface EasCliStatus {
  installed: boolean;
  version?: string;
  logged_in: boolean;
  username?: string;
}

export interface ExpoDevServerInfo {
  is_running: boolean;
  port: number;
  url: string;
  qr_svg?: string;
}

// Build Management
export interface DetectedBuildSystem {
  name: string;
  kind: string;
  build_file: string;
  available_commands: string[];
}

export interface BuildExecutionResult {
  task_id?: string;
  command: string;
  success: boolean;
  exit_code?: number;
  stdout: string;
  stderr: string;
  duration_ms: number;
}

// Storage Management
export interface StorageItem {
  candidate_id?: string;
  name: string;
  path: string;
  relative_path?: string;
  category: string;
  size_bytes: number;
  human_size: string;
  item_count: number;
  is_cleanable: boolean;
}

export interface StorageAnalysisReport {
  total_size_bytes: number;
  total_human_size: string;
  items: StorageItem[];
  cleanable_bytes: number;
  cleanable_human_size: string;
}

export interface StorageCleanupReport {
  dry_run: boolean;
  freed_bytes: number;
  freed_human_size: string;
  deleted_paths: string[];
  errors: string[];
}

// Checkpoint Engine
export interface CheckpointFileRecord {
  root_id: string;
  relative_path: string;
  sha256: string;
  size_bytes: number;
}

export interface WorkspaceCheckpoint {
  checkpoint_id: string;
  workspace_id?: string;
  description: string;
  created_at: string;
  files?: CheckpointFileRecord[];
  file_hashes?: Record<string, string>;
}

export interface CheckpointRestorePreview {
  preview_token: string;
  checkpoint_id: string;
  files_to_create: string[];
  files_to_overwrite: string[];
  conflicts: string[];
  missing_blobs: string[];
  integrity_valid: boolean;
}

export interface CheckpointRestoreReport {
  checkpoint_id: string;
  restored_count: number;
  restored_files: string[];
  errors: string[];
  success: boolean;
  backup_checkpoint_id?: string;
  preserved_backups?: string[];
  compensated_files?: string[];
  recovery_conflicts?: string[];
}

// Git Engine
export interface GitStatusResult {
  branch: string;
  is_clean: boolean;
  modified_files: string[];
  staged_files: string[];
  untracked_files: string[];
}

// LSP Client
export interface Diagnostic {
  range: {
    start: { line: number; character: number };
    end: { line: number; character: number };
  };
  severity: number; // 1 = Error, 2 = Warning, 3 = Info, 4 = Hint
  code?: string;
  source?: string;
  message: string;
}
