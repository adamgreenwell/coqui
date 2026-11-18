'use client';

import { useCallback, useEffect, useMemo, useState } from 'react';
import type { FormEvent } from 'react';

const PAGE_SIZE = 50;
const USER_AGENT =
  'Mozilla/5.0 (compatible; Coqui/0.1; +https://github.com/adamgreenwell/coqui)';

type CrawlStatus =
  | 'pending'
  | 'running'
  | 'paused'
  | 'completed'
  | 'failed'
  | 'cancelled';

interface CrawlConfig {
  maxDepth: number;
  maxUrls: number;
  concurrency: number;
  requestTimeoutMs: number;
  userAgent: string;
  sameHostOnly: boolean;
  respectRobots: boolean;
}

interface CrawlSummary {
  id: string;
  seed: string;
  status: CrawlStatus;
  config: CrawlConfig;
  createdAt: number;
  startedAt: number | null;
  finishedAt: number | null;
  totalUrls: number;
}

interface UrlRow {
  id: number;
  crawlId: string;
  url: string;
  status: number | null;
  contentType: string | null;
  title: string | null;
  metaDescription: string | null;
  h1: string | null;
  h1Count: number;
  canonical: string | null;
  metaRobots: string | null;
  redirectTarget: string | null;
  redirectChain: string[];
  wordCount: number | null;
  depth: number;
  responseTimeMs: number | null;
  sizeBytes: number | null;
  error: string | null;
  discoveredAt: number;
}

interface UrlPage {
  rows: UrlRow[];
  total: number;
  page: number;
  pageSize: number;
}

interface CrawlProgress {
  crawlId: string;
  crawled: number;
  discovered: number;
  currentUrl: string;
}

interface CrawlFinished {
  crawlId: string;
  status: CrawlStatus;
  error: string | null;
}

type Severity = 'error' | 'warning' | 'opportunity';

interface IssueCount {
  rule: string;
  severity: Severity;
  count: number;
}

const ISSUE_LABELS: Record<string, string> = {
  missing_title: 'Missing title',
  title_too_long: 'Title too long',
  missing_meta_description: 'Missing meta description',
  multiple_h1: 'Multiple H1 headings',
  thin_content: 'Thin content',
  broken_internal_link: 'Broken internal link',
};

const STATUS_LABEL: Record<string, string> = {
  idle: 'Ready',
  pending: 'Starting',
  running: 'Crawling',
  completed: 'Completed',
  failed: 'Failed',
  cancelled: 'Cancelled',
};

const COLUMNS = [
  { key: 'status', label: 'Status' },
  { key: 'url', label: 'URL' },
  { key: 'title', label: 'Title' },
  { key: 'h1', label: 'H1' },
  { key: 'words', label: 'Words' },
  { key: 'canonical', label: 'Canonical' },
  { key: 'robots', label: 'Robots' },
  { key: 'redirect', label: 'Redirect' },
  { key: 'depth', label: 'Depth' },
  { key: 'time', label: 'Time' },
] as const;

type ColumnKey = (typeof COLUMNS)[number]['key'];

const DEFAULT_COLUMNS: Record<ColumnKey, boolean> = {
  status: true,
  url: true,
  title: true,
  h1: true,
  words: true,
  canonical: true,
  robots: false,
  redirect: false,
  depth: false,
  time: false,
};

function statusCodeClass(row: UrlRow): string {
  if (row.error) return 'error';
  const code = row.status ?? 0;
  if (code >= 200 && code < 300) return 'ok';
  if (code >= 300 && code < 400) return 'redirect';
  if (code >= 400 && code < 500) return 'client';
  if (code >= 500) return 'server';
  return 'error';
}

function FrogMark() {
  return (
    <svg width="18" height="18" viewBox="0 0 24 24" aria-hidden="true">
      <path
        d="M4 15c0-3.6 3.6-6.5 8-6.5s8 2.9 8 6.5c0 2.9-3.6 5-8 5s-8-2.1-8-5Z"
        fill="#7ee787"
      />
      <circle cx="8.6" cy="7.1" r="2.1" fill="#7ee787" />
      <circle cx="15.4" cy="7.1" r="2.1" fill="#7ee787" />
      <circle cx="8.6" cy="7.1" r="0.95" fill="#08130c" />
      <circle cx="15.4" cy="7.1" r="0.95" fill="#08130c" />
    </svg>
  );
}

export default function Home() {
  const [isTauri, setIsTauri] = useState(false);
  const [seed, setSeed] = useState('https://example.com');
  const [maxDepth, setMaxDepth] = useState(3);
  const [maxUrls, setMaxUrls] = useState(250);
  const [concurrency, setConcurrency] = useState(8);
  const [sameHostOnly, setSameHostOnly] = useState(true);
  const [respectRobots, setRespectRobots] = useState(true);

  const [crawl, setCrawl] = useState<CrawlSummary | null>(null);
  const [phase, setPhase] = useState<CrawlStatus | 'idle'>('idle');
  const [rows, setRows] = useState<UrlRow[]>([]);
  const [total, setTotal] = useState(0);
  const [page, setPage] = useState(1);
  const [progress, setProgress] = useState<CrawlProgress | null>(null);
  const [issues, setIssues] = useState<IssueCount[]>([]);
  const [activeIssue, setActiveIssue] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [exportPath, setExportPath] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [columns, setColumns] = useState<Record<ColumnKey, boolean>>(DEFAULT_COLUMNS);
  const [columnsOpen, setColumnsOpen] = useState(false);

  useEffect(() => {
    setIsTauri(typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window);
  }, []);

  const loadPage = useCallback(
    async (crawlId: string, pageNumber: number, issue: string | null) => {
      const { invoke } = await import('@tauri-apps/api/core');
      const data = await invoke<UrlPage>('list_urls', {
        crawlId,
        page: pageNumber,
        pageSize: PAGE_SIZE,
        issue,
      });
      setRows(data.rows);
      setTotal(data.total);
    },
    [],
  );

  const loadIssues = useCallback(async (crawlId: string) => {
    const { invoke } = await import('@tauri-apps/api/core');
    const counts = await invoke<IssueCount[]>('get_issue_counts', { crawlId });
    setIssues(counts);
  }, []);

  useEffect(() => {
    if (!isTauri) return;
    let active = true;
    const cleanups: Array<() => void> = [];
    (async () => {
      const { listen } = await import('@tauri-apps/api/event');
      const offProgress = await listen<CrawlProgress>('crawl-progress', (event) => {
        setProgress(event.payload);
      });
      const offFinished = await listen<CrawlFinished>('crawl-finished', async (event) => {
        setPhase(event.payload.status);
        if (event.payload.error) setError(event.payload.error);
        setActiveIssue(null);
        await Promise.all([
          loadPage(event.payload.crawlId, 1, null),
          loadIssues(event.payload.crawlId),
        ]);
        setPage(1);
      });
      if (!active) {
        offProgress();
        offFinished();
        return;
      }
      cleanups.push(offProgress, offFinished);
    })();
    return () => {
      active = false;
      cleanups.forEach((fn) => fn());
    };
  }, [isTauri, loadPage, loadIssues]);

  useEffect(() => {
    if (!isTauri || phase !== 'running' || !crawl) return;
    const timer = setInterval(() => {
      loadPage(crawl.id, page, activeIssue).catch(() => undefined);
    }, 1200);
    return () => clearInterval(timer);
  }, [isTauri, phase, crawl, page, activeIssue, loadPage]);

  async function startCrawl(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setBusy(true);
    setError(null);
    setExportPath(null);
    setRows([]);
    setTotal(0);
    setIssues([]);
    setActiveIssue(null);
    setPage(1);
    setProgress(null);
    try {
      const { invoke } = await import('@tauri-apps/api/core');
      const config: CrawlConfig = {
        maxDepth,
        maxUrls,
        concurrency,
        requestTimeoutMs: 15000,
        userAgent: USER_AGENT,
        sameHostOnly,
        respectRobots,
      };
      const summary = await invoke<CrawlSummary>('create_crawl', { seed, config });
      setCrawl(summary);
      setPhase('running');
      await invoke('start_crawl', { crawlId: summary.id });
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
      setPhase('failed');
    } finally {
      setBusy(false);
    }
  }

  async function pauseCrawl() {
    if (!crawl) return;
    const { invoke } = await import('@tauri-apps/api/core');
    await invoke('pause_crawl', { crawlId: crawl.id });
  }

  async function resumeCrawl() {
    if (!crawl) return;
    setBusy(true);
    setError(null);
    setPhase('running');
    try {
      const { invoke } = await import('@tauri-apps/api/core');
      await invoke('start_crawl', { crawlId: crawl.id });
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
      setPhase('paused');
    } finally {
      setBusy(false);
    }
  }

  async function cancelCrawl() {
    if (!crawl) return;
    const { invoke } = await import('@tauri-apps/api/core');
    await invoke('cancel_crawl', { crawlId: crawl.id });
  }

  async function exportCsv() {
    if (!crawl) return;
    try {
      const { invoke } = await import('@tauri-apps/api/core');
      const path = await invoke<string>('export_csv', { crawlId: crawl.id });
      setExportPath(path);
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    }
  }

  async function goToPage(next: number) {
    if (!crawl) return;
    setPage(next);
    try {
      await loadPage(crawl.id, next, activeIssue);
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    }
  }

  async function filterByIssue(rule: string | null) {
    if (!crawl) return;
    setActiveIssue(rule);
    setPage(1);
    try {
      await loadPage(crawl.id, 1, rule);
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    }
  }

  const totalPages = Math.max(1, Math.ceil(total / PAGE_SIZE));
  const isRunning = phase === 'running' || phase === 'pending';
  const cap = crawl?.config.maxUrls ?? maxUrls;
  const target = Math.max(1, Math.min(cap, progress?.discovered || cap));
  const percent = progress
    ? Math.min(100, Math.round((progress.crawled / target) * 100))
    : 0;
  const displayUrl = phase === 'running' ? progress?.currentUrl : crawl?.seed;
  const issueTotal = useMemo(
    () => issues.reduce((sum, issue) => sum + issue.count, 0),
    [issues],
  );

  function cell(row: UrlRow, key: ColumnKey) {
    switch (key) {
      case 'status':
        return (
          <span className={`status-chip ${statusCodeClass(row)}`}>
            {row.error ? 'ERR' : (row.status ?? '—')}
          </span>
        );
      case 'url':
        return (
          <div className="url-cell" title={row.url}>
            {row.url}
          </div>
        );
      case 'title':
        return (
          <div className="title-cell" title={row.title ?? ''}>
            {row.title ?? '—'}
          </div>
        );
      case 'h1':
        return (
          <div className="title-cell" title={row.h1 ?? ''}>
            {row.h1 ?? '—'}
          </div>
        );
      case 'words':
        return row.wordCount ?? '—';
      case 'canonical':
        return (
          <div className="title-cell mono" title={row.canonical ?? ''}>
            {row.canonical ?? '—'}
          </div>
        );
      case 'robots':
        return row.metaRobots ?? '—';
      case 'redirect':
        return (
          <div className="title-cell mono" title={row.redirectTarget ?? ''}>
            {row.redirectTarget ?? '—'}
          </div>
        );
      case 'depth':
        return row.depth;
      case 'time':
        return row.responseTimeMs != null ? `${row.responseTimeMs} ms` : '—';
      default:
        return null;
    }
  }

  return (
    <div className="app">
      <header className="topbar">
        <div className="brand">
          <span className="brand-mark">
            <FrogMark />
          </span>
          <span className="brand-name">
            coqu<em>í</em>
          </span>
          <span className="badge">M2 · pause &amp; resume</span>
        </div>
        <div className="topbar-right">
          <span className="badge">{isTauri ? 'Desktop' : 'Browser preview'}</span>
        </div>
      </header>

      <div className="layout">
        <aside className="panel sidebar">
          <h2>New crawl</h2>
          <p className="hint">
            Same-host breadth-first crawl with robots.txt, redirect chains and the first audit
            rules. Every row lands in a portable SQLite crawl file.
          </p>
          <form onSubmit={startCrawl}>
            <div className="field">
              <label htmlFor="seed">Start URL</label>
              <input
                id="seed"
                type="url"
                value={seed}
                onChange={(event) => setSeed(event.target.value)}
                placeholder="https://example.com"
                disabled={isRunning}
                required
              />
            </div>
            <div className="grid-2">
              <div className="field">
                <label htmlFor="depth">Max depth</label>
                <input
                  id="depth"
                  type="number"
                  min={0}
                  max={20}
                  value={maxDepth}
                  onChange={(event) => setMaxDepth(Number(event.target.value))}
                  disabled={isRunning}
                />
              </div>
              <div className="field">
                <label htmlFor="maxUrls">Max URLs</label>
                <input
                  id="maxUrls"
                  type="number"
                  min={1}
                  max={100000}
                  value={maxUrls}
                  onChange={(event) => setMaxUrls(Number(event.target.value))}
                  disabled={isRunning}
                />
              </div>
            </div>
            <div className="field">
              <label htmlFor="concurrency">Concurrency</label>
              <input
                id="concurrency"
                type="number"
                min={1}
                max={64}
                value={concurrency}
                onChange={(event) => setConcurrency(Number(event.target.value))}
                disabled={isRunning}
              />
            </div>
            <label className="checkbox">
              <input
                type="checkbox"
                checked={sameHostOnly}
                onChange={(event) => setSameHostOnly(event.target.checked)}
                disabled={isRunning}
              />
              Stay on the seed host
            </label>
            <label className="checkbox">
              <input
                type="checkbox"
                checked={respectRobots}
                onChange={(event) => setRespectRobots(event.target.checked)}
                disabled={isRunning}
              />
              Respect robots.txt
            </label>
            {phase === 'paused' ? (
              <button
                className="primary"
                type="button"
                onClick={resumeCrawl}
                disabled={!isTauri || busy}
              >
                Resume crawl
              </button>
            ) : (
              <button className="primary" type="submit" disabled={!isTauri || busy || isRunning}>
                {isRunning ? 'Crawling…' : 'Start crawl'}
              </button>
            )}
            {isRunning && (
              <div style={{ display: 'flex', gap: 8, marginTop: 8 }}>
                <button
                  className="secondary"
                  type="button"
                  onClick={pauseCrawl}
                  style={{ flex: 1 }}
                >
                  Pause
                </button>
                <button
                  className="secondary"
                  type="button"
                  onClick={cancelCrawl}
                  style={{ flex: 1 }}
                >
                  Stop
                </button>
              </div>
            )}
          </form>

          <p className="footer-note" style={{ marginTop: 18, marginBottom: 0 }}>
            Next up: sitemap seeds, authentication and the full column registry.
          </p>
        </aside>

        <main className="main">
          {!isTauri && (
            <div className="notice">
              This is the browser preview of the UI. Start the desktop shell with{' '}
              <code>npm run dev</code> to crawl; the Rust engine is only available inside Tauri.
            </div>
          )}

          {error && <div className="error">{error}</div>}

          <section className="panel status-card">
            <div>
              <div className="status-title">
                <span className={`dot ${phase}`} />
                {STATUS_LABEL[phase] ?? phase}
                {crawl && <span className="badge">{new URL(crawl.seed).host}</span>}
              </div>
              <div className="status-sub">
                {displayUrl ??
                  (crawl
                    ? 'Crawl finished. Review the audit below.'
                    : 'Start a crawl to see live results and audit issues.')}
              </div>
            </div>
            <div className="metrics">
              <div>
                Crawled
                <strong>{progress?.crawled ?? total}</strong>
              </div>
              <div>
                Discovered
                <strong>{progress?.discovered ?? total}</strong>
              </div>
              <div>
                Issues
                <strong>{issueTotal}</strong>
              </div>
              <div>
                Limit
                <strong>{cap}</strong>
              </div>
            </div>
            <div className="progress-track">
              <div className="progress-bar" style={{ width: `${percent}%` }} />
            </div>
          </section>

          {issues.length > 0 && (
            <section className="panel issues-panel">
              <div className="issues-head">
                <h2>Audit</h2>
                <button
                  className={`chip ${activeIssue === null ? 'active' : ''}`}
                  type="button"
                  onClick={() => filterByIssue(null)}
                >
                  All URLs
                </button>
              </div>
              <div className="issue-grid">
                {issues.map((issue) => (
                  <button
                    key={`${issue.rule}-${issue.severity}`}
                    type="button"
                    className={`issue-card ${issue.severity} ${
                      activeIssue === issue.rule ? 'active' : ''
                    }`}
                    onClick={() => filterByIssue(issue.rule)}
                  >
                    <span className="issue-count">{issue.count}</span>
                    <span className="issue-label">
                      {ISSUE_LABELS[issue.rule] ?? issue.rule}
                    </span>
                    <span className="issue-sev">{issue.severity}</span>
                  </button>
                ))}
              </div>
            </section>
          )}

          <section className="panel results">
            <div className="results-head">
              <h2>
                URLs <span className="count">{total.toLocaleString()}</span>
                {activeIssue && (
                  <span className="filter-chip">
                    {ISSUE_LABELS[activeIssue] ?? activeIssue}
                    <button type="button" onClick={() => filterByIssue(null)} aria-label="Clear filter">
                      ×
                    </button>
                  </span>
                )}
              </h2>
              <div className="results-actions">
                {exportPath && <span className="export-path">{exportPath}</span>}
                <div className="columns-wrap">
                  <button
                    className="secondary"
                    type="button"
                    onClick={() => setColumnsOpen((open) => !open)}
                  >
                    Columns
                  </button>
                  {columnsOpen && (
                    <div className="columns-menu">
                      {COLUMNS.map((column) => (
                        <label key={column.key}>
                          <input
                            type="checkbox"
                            checked={columns[column.key]}
                            onChange={(event) =>
                              setColumns((current) => ({
                                ...current,
                                [column.key]: event.target.checked,
                              }))
                            }
                          />
                          {column.label}
                        </label>
                      ))}
                    </div>
                  )}
                </div>
                <button
                  className="secondary"
                  type="button"
                  onClick={exportCsv}
                  disabled={!isTauri || !crawl || total === 0}
                >
                  Export CSV
                </button>
              </div>
            </div>

            {rows.length === 0 ? (
              <div className="empty">
                <strong>{activeIssue ? 'No URLs match this issue' : 'No URLs yet'}</strong>
                {crawl && isRunning
                  ? 'Waiting for the first response…'
                  : activeIssue
                    ? 'Clear the filter to see the whole crawl.'
                    : 'Start a crawl and rows will stream in here.'}
              </div>
            ) : (
              <div className="table-wrap">
                <table>
                  <thead>
                    <tr>
                      {COLUMNS.filter((column) => columns[column.key]).map((column) => (
                        <th
                          key={column.key}
                          className={
                            ['words', 'depth', 'time'].includes(column.key) ? 'num' : undefined
                          }
                        >
                          {column.label}
                        </th>
                      ))}
                    </tr>
                  </thead>
                  <tbody>
                    {rows.map((row) => (
                      <tr key={row.id}>
                        {COLUMNS.filter((column) => columns[column.key]).map((column) => (
                          <td
                            key={column.key}
                            className={
                              ['words', 'depth', 'time'].includes(column.key) ? 'num' : undefined
                            }
                          >
                            {cell(row, column.key)}
                          </td>
                        ))}
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
            )}

            <div className="pager">
              <span>
                Page {page} of {totalPages}
              </span>
              <div className="pager-controls">
                <button
                  className="secondary"
                  type="button"
                  onClick={() => goToPage(page - 1)}
                  disabled={page <= 1 || !crawl}
                >
                  Previous
                </button>
                <button
                  className="secondary"
                  type="button"
                  onClick={() => goToPage(page + 1)}
                  disabled={page >= totalPages || !crawl}
                >
                  Next
                </button>
              </div>
            </div>
          </section>

          <p className="footer-note">
            M1 scope: robots.txt, manual redirect chains, canonical/meta-robots extraction, link
            edges, six audit rules and issue filtering. Rendering, sitemap seeds and resume follow.
          </p>
        </main>
      </div>
    </div>
  );
}





