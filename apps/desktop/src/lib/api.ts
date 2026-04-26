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
