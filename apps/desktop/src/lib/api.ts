// Typed wrappers around Tauri commands defined in src-tauri/src/lib.rs.
// All errors come back as plain strings (we mapped VaultError -> String in Rust)
// and are thrown so callers can use try/catch.

import { invoke } from '@tauri-apps/api/core';

export type VaultStatusKind = 'no_vault' | 'locked' | 'unlocked';

export interface VaultStatusReport {
  status: VaultStatusKind;
  path: string | null;
}

export interface KeyMetadataDto {
  id: string;
  provider: string;
  label: string;
  project_tag: string | null;
  created_at: string; // ISO
  expires_at: string | null;
  last_used_at: string | null;
  status: string;
  notes: string | null;
}

export interface AddKeyArgs {
  provider: string;
  label: string;
  project_tag: string | null;
  notes: string | null;
  key_value: string;
}

export type RuntimeExportTarget = 'env_file';

export interface RuntimeExportArgs {
  key_ids: string[];
  target_dir: string;
  filename: string | null;
  profile_name: string | null;
  target: RuntimeExportTarget;
  dry_run: boolean;
  backup_existing: boolean;
  update_gitignore: boolean;
}

export interface RuntimeExportReport {
  dry_run: boolean;
  target_path: string;
  profile_name: string;
  key_count: number;
  exported_key_names: string[];
  preview_lines: string[];
  file_exists: boolean;
  backup_path: string | null;
  git_tracked: boolean;
  gitignore_updated: boolean;
  audit_log_path: string | null;
}

export const PROVIDERS = [
  'anthropic',
  'openai',
  'google',
  'replicate',
  'elevenlabs',
  'pinecone',
  'stripe',
  'cloudflare',
  'generic'
] as const;

export async function vaultStatus(): Promise<VaultStatusReport> {
  return await invoke<VaultStatusReport>('vault_status');
}

export async function createVault(password: string): Promise<VaultStatusReport> {
  return await invoke<VaultStatusReport>('create_vault', { password });
}

export async function unlockVault(password: string): Promise<void> {
  await invoke<void>('unlock_vault', { password });
}

export async function lockVault(): Promise<void> {
  await invoke<void>('lock_vault');
}

export async function listKeys(): Promise<KeyMetadataDto[]> {
  return await invoke<KeyMetadataDto[]>('list_keys');
}

export async function addKey(args: AddKeyArgs): Promise<KeyMetadataDto> {
  return await invoke<KeyMetadataDto>('add_key', { args });
}

export async function deleteKey(id: string): Promise<void> {
  await invoke<void>('delete_key', { id });
}

/**
 * Decrypt key by id and write to OS clipboard. Returns the auto-clear delay
 * in seconds.
 */
export async function copyToClipboard(id: string): Promise<number> {
  return await invoke<number>('copy_to_clipboard', { id });
}

export async function exportRuntimeProfile(args: RuntimeExportArgs): Promise<RuntimeExportReport> {
  return await invoke<RuntimeExportReport>('export_runtime_profile', { args });
}

// ── M3: Local repo scan for leaked secrets ──────────────────────────────────

export type ScanRiskLevel = 'critical' | 'high' | 'medium' | 'low';
export type ScanTier = 'tier1' | 'tier2' | 'tier3';

export interface ScanArgs {
  path: string;
  follow_symlinks: boolean;
  respect_gitignore: boolean;
  /** 0 = use default (5 MB) */
  max_file_size_bytes: number;
}

/**
 * One scanner finding. Only `redacted_preview` ever touches the matched
 * substring; the raw match never crosses the IPC boundary.
 */
export interface ScanDetection {
  secret_type: string;
  provider: string;
  display_name: string;
  file_path: string | null;
  line_number: number;
  redacted_preview: string;
  risk_level: ScanRiskLevel;
  tier: ScanTier;
  git_tracked: boolean | null;
  recommended_action: string;
  rotation_url: string | null;
  docs_url: string | null;
}

export interface ScanDetectorSummary {
  detector_id: string;
  display_name: string;
  provider: string;
  tier: ScanTier;
  risk_level: ScanRiskLevel;
  count: number;
}

export interface ScanReport {
  root_path: string;
  scanned_files: number;
  skipped_binary: number;
  skipped_oversize: number;
  skipped_unreadable: number;
  skipped_ignored: number;
  elapsed_ms: number;
  detections: ScanDetection[];
  summary_by_detector: ScanDetectorSummary[];
  summary_by_risk: Record<string, number>;
  summary_by_provider: Record<string, number>;
  respect_gitignore: boolean;
  follow_symlinks: boolean;
}

export async function scanProject(args: ScanArgs): Promise<ScanReport> {
  return await invoke<ScanReport>('scan_project_for_secrets', { args });
}

// ── M3.1 T3.1.2: Safe .gitignore helper ─────────────────────────────────────

export interface GitignoreAuditArgs {
  path: string;
}

export interface GitignoreRuleLine {
  line: string;
  already_present: boolean;
}

export interface GitignoreRuleSet {
  id: string;
  label: string;
  description: string;
  default_on: boolean;
  locked_on: boolean;
  auto_detected: boolean;
  header_comment: string;
  rules: GitignoreRuleLine[];
}

export interface GitignoreAuditReport {
  root_path: string;
  target_path: string;
  gitignore_exists: boolean;
  project_types: string[];
  rule_sets: GitignoreRuleSet[];
  existing_line_count: number;
}

export interface GitignoreRuleSetSelection {
  rule_set_id: string;
  lines: string[];
}

export interface GitignoreApplyArgs {
  path: string;
  selections: GitignoreRuleSetSelection[];
}

export interface GitignoreApplyReport {
  target_path: string;
  created_new_file: boolean;
  lines_added: number;
  appended_block: string;
}

export async function gitignoreAudit(args: GitignoreAuditArgs): Promise<GitignoreAuditReport> {
  return await invoke<GitignoreAuditReport>('gitignore_audit', { args });
}

export async function gitignoreApply(args: GitignoreApplyArgs): Promise<GitignoreApplyReport> {
  return await invoke<GitignoreApplyReport>('gitignore_apply', { args });
}

// ── M3.1 T3.1.3: Agent runtime profiles ─────────────────────────────────────

export interface AgentProfile {
  id: string;
  name: string;
  description: string;
  default_filename: string;
  suggested_env_vars: string[];
  todo_note: string | null;
}

export async function listAgentProfiles(): Promise<AgentProfile[]> {
  return await invoke<AgentProfile[]>('list_agent_profiles');
}

// ── M3.1 T3.1.1: .env.example generator ─────────────────────────────────────

export interface EnvExampleLine {
  name: string;
  comment: string | null;
}

export interface EnvExampleProposal {
  source_kind: string; // "vault" | "env_file"
  source_label: string;
  lines: EnvExampleLine[];
  parsed_count: number;
  skipped_count: number;
}

export interface EnvExampleFromVaultArgs {
  key_ids: string[];
  include_holster_comments: boolean;
}

export interface EnvExampleFromFileArgs {
  source_path: string;
}

export interface EnvExampleApplyArgs {
  target_dir: string;
  filename: string | null;
  lines: EnvExampleLine[];
  overwrite: boolean;
  include_header_comments: boolean;
}

export interface EnvExampleApplyReport {
  target_path: string;
  file_existed: boolean;
  overwrote: boolean;
  line_count: number;
  audit_log_path: string | null;
}

export async function envExampleFromVault(args: EnvExampleFromVaultArgs): Promise<EnvExampleProposal> {
  return await invoke<EnvExampleProposal>('env_example_from_vault', { args });
}

export async function envExampleFromFile(args: EnvExampleFromFileArgs): Promise<EnvExampleProposal> {
  return await invoke<EnvExampleProposal>('env_example_from_file', { args });
}

export async function envExampleApply(args: EnvExampleApplyArgs): Promise<EnvExampleApplyReport> {
  return await invoke<EnvExampleApplyReport>('env_example_apply', { args });
}

// ── M4: Holster Auth — local TOTP vault ─────────────────────────────────────

export interface TotpAccountDto {
  id: string;
  label: string;
  issuer: string | null;
  account_name: string | null;
  backup_code_count: number;
  created_at: string;
  last_used_at: string | null;
}

export interface AddTotpAccountArgs {
  label: string;
  issuer: string | null;
  account_name: string | null;
  secret_or_uri: string;
  backup_codes: string | null;
}

export interface TotpCodeReport {
  code: string;
  seconds_remaining: number;
  period: number;
}

export async function listTotpAccounts(): Promise<TotpAccountDto[]> {
  return await invoke<TotpAccountDto[]>('list_totp_accounts');
}

export async function addTotpAccount(args: AddTotpAccountArgs): Promise<TotpAccountDto> {
  return await invoke<TotpAccountDto>('add_totp_account', { args });
}

export async function getTotpCode(id: string): Promise<TotpCodeReport> {
  return await invoke<TotpCodeReport>('get_totp_code', { id });
}
