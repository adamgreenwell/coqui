'use client';

import { useCallback, useEffect, useState } from 'react';
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

const STATUS_LABEL: Record<string, string> = {
  idle: 'Ready',
  pending: 'Starting',
  running: 'Crawling',
  paused: 'Paused',
  completed: 'Completed',
  failed: 'Failed',
  cancelled: 'Cancelled',
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

  const [crawl, setCrawl] = useState<CrawlSummary | null>(null);
  const [phase, setPhase] = useState<CrawlStatus | 'idle'>('idle');
  const [rows, setRows] = useState<UrlRow[]>([]);
  const [total, setTotal] = useState(0);
  const [page, setPage] = useState(1);
  const [progress, setProgress] = useState<CrawlProgress | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [exportPath, setExportPath] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    setIsTauri(typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window);
  }, []);

  const refresh = useCallback(async (crawlId: string, pageNumber: number) => {
    const { invoke } = await import('@tauri-apps/api/core');
    const data = await invoke<UrlPage>('list_urls', {
      crawlId,
      page: pageNumber,
      pageSize: PAGE_SIZE,
    });
    setRows(data.rows);
    setTotal(data.total);
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
        await refresh(event.payload.crawlId, 1);
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
  }, [isTauri, refresh]);

  useEffect(() => {
    if (!isTauri || phase !== 'running' || !crawl) return;
    const timer = setInterval(() => {
      refresh(crawl.id, page).catch(() => undefined);
    }, 1200);
    return () => clearInterval(timer);
  }, [isTauri, phase, crawl, page, refresh]);

  async function startCrawl(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setBusy(true);
    setError(null);
    setExportPath(null);
    setRows([]);
    setTotal(0);
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
      await refresh(crawl.id, next);
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    }
  }

  const totalPages = Math.max(1, Math.ceil(total / PAGE_SIZE));
  const isRunning = phase === 'running' || phase === 'pending';
  const cap = crawl?.config.maxUrls ?? maxUrls;
  const percent = progress ? Math.min(100, Math.round((progress.crawled / Math.max(1, cap)) * 100)) : 0;

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
          <span className="badge">M0 walking skeleton</span>
        </div>
        <div className="topbar-right">
          <span className="badge">{isTauri ? 'Desktop' : 'Browser preview'}</span>
        </div>
      </header>

      <div className="layout">
        <aside className="panel sidebar">
          <h2>New crawl</h2>
          <p className="hint">
            Point Coqui at a site. It follows same-host links, extracts the core SEO fields and
            stores every row in a portable SQLite crawl file.
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
            <button className="primary" type="submit" disabled={!isTauri || busy || isRunning}>
              {isRunning ? 'Crawling…' : 'Start crawl'}
            </button>
            {isRunning && (
              <button
                className="secondary"
                type="button"
                onClick={cancelCrawl}
                style={{ width: '100%', marginTop: 8 }}
              >
                Stop crawl
              </button>
            )}
          </form>

          <p className="footer-note" style={{ marginTop: 18, marginBottom: 0 }}>
            Next up in this slice: robots.txt, redirect chains, issue checks and the full column
            registry.
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
                {progress?.currentUrl ??
                  (crawl
                    ? `Crawling ${crawl.seed}`
                    : 'Start a crawl to see live results. This slice stores title, meta description, H1, word count, status and timing.')}
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
                Limit
                <strong>{cap}</strong>
              </div>
            </div>
            <div className="progress-track">
              <div className="progress-bar" style={{ width: `${percent}%` }} />
            </div>
          </section>

          <section className="panel results">
            <div className="results-head">
              <h2>
                URLs <span className="count">{total.toLocaleString()}</span>
              </h2>
              <div style={{ display: 'flex', gap: 10, alignItems: 'center' }}>
                {exportPath && <span className="export-path">{exportPath}</span>}
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
                <strong>No URLs yet</strong>
                {crawl && isRunning
                  ? 'Waiting for the first response…'
                  : 'Start a crawl and rows will stream in here.'}
              </div>
            ) : (
              <div className="table-wrap">
                <table>
                  <thead>
                    <tr>
                      <th style={{ width: 70 }}>Status</th>
                      <th>URL</th>
                      <th>Title</th>
                      <th>H1</th>
                      <th style={{ width: 70 }} className="num">
                        Words
                      </th>
                      <th style={{ width: 60 }} className="num">
                        Depth
                      </th>
                      <th style={{ width: 80 }} className="num">
                        Time
                      </th>
                    </tr>
                  </thead>
                  <tbody>
                    {rows.map((row) => (
                      <tr key={row.id}>
                        <td>
                          <span className={`status-chip ${statusCodeClass(row)}`}>
                            {row.error ? 'ERR' : (row.status ?? '—')}
                          </span>
                        </td>
                        <td>
                          <div className="url-cell" title={row.url}>
                            {row.url}
                          </div>
                        </td>
                        <td>
                          <div className="title-cell" title={row.title ?? ''}>
                            {row.title ?? '—'}
                          </div>
                        </td>
                        <td>
                          <div className="title-cell" title={row.h1 ?? ''}>
                            {row.h1 ?? '—'}
                          </div>
                        </td>
                        <td className="num">{row.wordCount ?? '—'}</td>
                        <td className="num">{row.depth}</td>
                        <td className="num">
                          {row.responseTimeMs != null ? `${row.responseTimeMs} ms` : '—'}
                        </td>
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
            M0 scope: one seed, same-host BFS, concurrent HTTP, SQLite persistence, live progress
            and CSV export. Rendering, robots.txt, issue rules and the full 220-column model follow
            in phases 1–2.
          </p>
        </main>
      </div>
    </div>
  );
}
