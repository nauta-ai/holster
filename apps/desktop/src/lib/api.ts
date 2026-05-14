// Typed wrappers around Tauri commands defined in src-tauri/src/lib.rs.
// All errors come back as plain strings (we mapped VaultError -> String in Rust)
// and are thrown so callers can use try/catch.

import { invoke as tauriInvoke } from '@tauri-apps/api/core';

type InvokeArgs = Record<string, unknown> | undefined;

function hasTauriBridge(): boolean {
  if (typeof window === 'undefined') return true;
  return Boolean((window as unknown as { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__);
}

async function invoke<T>(command: string, args?: InvokeArgs): Promise<T> {
  if (import.meta.env.DEV && !hasTauriBridge()) {
    return mockInvoke<T>(command, args);
  }

  return await tauriInvoke<T>(command, args);
}

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

async function mockInvoke<T>(command: string, args?: InvokeArgs): Promise<T> {
  const now = new Date().toISOString();
  const keys: KeyMetadataDto[] = [
    {
      id: 'demo-openai',
      provider: 'openai',
      label: 'First AI subscription lab',
      project_tag: 'Buildbelt demo',
      created_at: now,
      expires_at: null,
      last_used_at: now,
      status: 'active',
      notes: 'Dev preview sample only'
    },
    {
      id: 'demo-google',
      provider: 'google',
      label: 'Nano Banana image tests',
      project_tag: 'Creative tools',
      created_at: now,
      expires_at: null,
      last_used_at: null,
      status: 'active',
      notes: 'Dev preview sample only'
    }
  ];

  switch (command) {
    case 'vault_status':
    case 'create_vault':
      return { status: 'unlocked', path: '~/.holster/demo-vault' } as T;
    case 'unlock_vault':
    case 'lock_vault':
    case 'delete_key':
      return undefined as T;
    case 'list_keys':
      return keys as T;
    case 'add_key':
      return {
        id: 'demo-added',
        provider: 'generic',
        label: 'Demo key',
        project_tag: null,
        created_at: now,
        expires_at: null,
        last_used_at: null,
        status: 'active',
        notes: null
      } as T;
    case 'copy_to_clipboard':
      return 30 as T;
    case 'export_runtime_profile':
      return {
        dry_run: true,
        target_path: '/tmp/.env.buildbelt',
        profile_name: 'demo',
        key_count: 2,
        exported_key_names: ['OPENAI_API_KEY', 'GOOGLE_API_KEY'],
        preview_lines: ['OPENAI_API_KEY=...', 'GOOGLE_API_KEY=...'],
        file_exists: false,
        backup_path: null,
        git_tracked: false,
        gitignore_updated: false,
        audit_log_path: null
      } as T;
    case 'scan_project_for_secrets':
      return {
        root_path: '/tmp/buildbelt-demo',
        scanned_files: 42,
        skipped_binary: 0,
        skipped_oversize: 0,
        skipped_unreadable: 0,
        skipped_ignored: 8,
        elapsed_ms: 118,
        detections: [],
        summary_by_detector: [],
        summary_by_risk: {},
        summary_by_risk_excluding_fixtures: {},
        summary_by_provider: {},
        real_finding_count: 0,
        fixture_finding_count: 0,
        respect_gitignore: true,
        follow_symlinks: false
      } as T;
    case 'gitignore_audit':
      return {
        root_path: '/tmp/buildbelt-demo',
        target_path: '/tmp/buildbelt-demo/.gitignore',
        gitignore_exists: true,
        project_types: ['node'],
        rule_sets: [],
        existing_line_count: 12
      } as T;
    case 'gitignore_apply':
      return {
        target_path: '/tmp/buildbelt-demo/.gitignore',
        created_new_file: false,
        lines_added: 0,
        appended_block: ''
      } as T;
    case 'list_agent_profiles':
      return [
        {
          id: 'codex',
          name: 'Codex',
          description: 'Local agent runtime profile',
          default_filename: '.env.codex',
          suggested_env_vars: ['OPENAI_API_KEY'],
          todo_note: null
        }
      ] as T;
    case 'env_example_from_vault':
    case 'env_example_from_file':
      return {
        source_kind: 'vault',
        source_label: 'Buildbelt demo',
        lines: [
          { name: 'OPENAI_API_KEY', comment: 'OpenAI API billing key' },
          { name: 'GOOGLE_API_KEY', comment: 'Google AI Studio key' }
        ],
        parsed_count: 2,
        skipped_count: 0
      } as T;
    case 'env_example_apply':
      return {
        target_path: '/tmp/buildbelt-demo/.env.example',
        file_existed: false,
        overwrote: false,
        line_count: 2,
        audit_log_path: null
      } as T;
    case 'list_totp_accounts':
      return [] as T;
    case 'add_totp_account':
      return {
        id: 'demo-totp',
        label: 'Demo account',
        issuer: 'Buildbelt',
        account_name: 'demo@example.com',
        backup_code_count: 0,
        created_at: now,
        last_used_at: null
      } as T;
    case 'get_totp_code':
      return { code: '123456', seconds_remaining: 20, period: 30 } as T;
    default:
      throw new Error(`Demo preview does not implement ${command}`);
  }
}

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

/**
 * Whether a detection is a real-looking finding or a known test/fixture
 * pattern. Mirrors the Rust `Classification` enum with snake_case
 * serialization. Verdict + headline counts should exclude any value other
 * than `real`.
 */
export type ScanClassification = 'real' | 'test_path' | 'test_value' | 'test_path_and_value';

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
  classification: ScanClassification;
}

export function isFixtureClassification(c: ScanClassification): boolean {
  return c !== 'real';
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
  /** Risk counts across ALL detections including test fixtures. */
  summary_by_risk: Record<string, number>;
  /**
   * Risk counts excluding fixture-classified detections — this is what the
   * verdict and headline number should consume.
   */
  summary_by_risk_excluding_fixtures: Record<string, number>;
  summary_by_provider: Record<string, number>;
  /** Number of detections classified as `real`. */
  real_finding_count: number;
  /** Number of detections classified as any fixture variant. */
  fixture_finding_count: number;
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

// ── MCP Preflight (V0.5) ───────────────────────────────────────────────────

export type Verdict = 'safe' | 'caution' | 'risky';
export type Severity = 'info' | 'caution' | 'risk';
export type Category = 'run' | 'share' | 'both';

export interface PreflightFinding {
  check: string;
  severity: Severity;
  category: Category;
  message: string;
  fix_hint: string | null;
}

export interface McpPreflightReport {
  server_name: string | null;
  run_verdict: Verdict;
  share_verdict: Verdict;
  findings: PreflightFinding[];
  raw_command_summary: string;
}

export interface McpPreflightBatchEntry {
  server_name: string;
  report: McpPreflightReport | null;
  error: string | null;
}

export interface McpPreflightBatchReport {
  config_path: string;
  config_found: boolean;
  entries: McpPreflightBatchEntry[];
  parse_error: string | null;
}

/**
 * Analyze a single MCP server config entry.
 *
 * @param json — the raw JSON for the server entry (a single object, not the
 *               outer `mcpServers` wrapper).
 * @param name — optional human-friendly server name. When supplied, populates
 *               `report.server_name` so multi-server tables can label rows.
 */
export async function analyzeMcpConfig(
  json: string,
  name: string | null = null
): Promise<McpPreflightReport> {
  return await invoke<McpPreflightReport>('analyze_mcp_config_cmd', { name, json });
}

/**
 * Scan a Claude Desktop config file (defaults to the standard macOS path)
 * and return a per-server analyzer report.
 *
 * `report.config_found === false` when the file doesn't exist (not an error;
 * the user just doesn't have Claude Desktop installed or has no MCP servers).
 */
export async function analyzeClaudeDesktopConfig(
  path: string | null = null
): Promise<McpPreflightBatchReport> {
  return await invoke<McpPreflightBatchReport>('analyze_claude_desktop_config_cmd', { path });
}
