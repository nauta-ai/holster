// Local-first scan history. Persists a small list of recent Doctor scans to
// localStorage so the main view can show recent activity without re-scanning.
// Stores only metadata (path, counts, verdict) — never raw findings or values.

import type { ScanReport } from './api';

const KEY = 'buildbelt:scanHistory';
const MAX_ENTRIES = 10;

export interface ScanHistoryEntry {
  timestamp: number;
  root_path: string;
  scanned_files: number;
  elapsed_ms: number;
  real_finding_count: number;
  fixture_finding_count: number;
  summary_by_risk_excluding_fixtures: Record<string, number>;
}

export function loadScanHistory(): ScanHistoryEntry[] {
  try {
    const raw = localStorage.getItem(KEY);
    if (!raw) return [];
    const parsed = JSON.parse(raw);
    if (!Array.isArray(parsed)) return [];
    return parsed.filter(
      (entry): entry is ScanHistoryEntry =>
        typeof entry === 'object' &&
        entry !== null &&
        typeof entry.root_path === 'string' &&
        typeof entry.timestamp === 'number'
    );
  } catch {
    return [];
  }
}

export function recordScan(report: ScanReport): void {
  try {
    const entry: ScanHistoryEntry = {
      timestamp: Date.now(),
      root_path: report.root_path,
      scanned_files: report.scanned_files,
      elapsed_ms: report.elapsed_ms,
      real_finding_count: report.real_finding_count,
      fixture_finding_count: report.fixture_finding_count,
      summary_by_risk_excluding_fixtures: { ...report.summary_by_risk_excluding_fixtures }
    };

    const history = loadScanHistory();
    // Dedupe by root_path: if same path scanned again, replace existing entry
    // so the most recent result is what's shown.
    const filtered = history.filter((e) => e.root_path !== entry.root_path);
    filtered.unshift(entry);
    const trimmed = filtered.slice(0, MAX_ENTRIES);
    localStorage.setItem(KEY, JSON.stringify(trimmed));
  } catch {
    // localStorage may be unavailable; recent-scans display will just be empty.
  }
}

export function clearScanHistory(): void {
  try {
    localStorage.removeItem(KEY);
  } catch {
    // ignore
  }
}
