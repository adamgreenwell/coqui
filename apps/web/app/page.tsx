'use client';

import { useCallback, useEffect, useMemo, useState } from 'react';
import type { FormEvent } from 'react';

const PAGE_SIZE = 50;
const USER_AGENT =
  'Mozilla/5.0 (compatible; Coqui/0.1; +https://github.com/adamgreenwell/coqui)';

function splitLines(value: string): string[] {
  return value
    .split('\n')
    .map((line) => line.trim())
    .filter(Boolean);
}

function seedLabel(crawl: CrawlSummary): string {
  if (crawl.seedKind === 'list') return 'list file';
  try {
    return new URL(crawl.seed).host;
  } catch {
    return crawl.seedKind;
  }
}

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
  includePatterns: string[];
  excludePatterns: string[];
  authUser: string | null;
  authPassword: string | null;
  customHeaders: string[];
  render: boolean;
  browserPath: string | null;
  renderWaitMs: number;
  renderWorkers: number;
  screenshots: boolean;
  renderBlockAssets: boolean;
  accessibility: boolean;
  webVitals: boolean;
  storeText?: boolean;
  customJs?: string[];
}

interface UrlDetail {
  url: Record<string, unknown>;
  extractions: { name: string; value: string }[];
  customJs: { index: number; value: string }[];
  linkCounts: { inbound: number; outbound: number };
  screenshot: string | null;
}

interface LighthouseAudit {
  id: string;
  title: string;
  score: number | null;
  displayValue: string | null;
}

interface LighthouseSummary {
  performance: number | null;
  accessibility: number | null;
  bestPractices: number | null;
  seo: number | null;
  audits: LighthouseAudit[];
  finalUrl: string | null;
  fetchTime: string | null;
}

interface CrawlSummary {
  id: string;
  seed: string;
  seedKind: 'url' | 'sitemap' | 'list';
  status: CrawlStatus;
  config: CrawlConfig;
  createdAt: number;
  startedAt: number | null;
  finishedAt: number | null;
  totalUrls: number;
  blockedUrls: number;
}

interface UrlRow {
  id: number;
  crawlId: string;
  url: string;
  status: number | null;
  blocked: boolean;
  consoleErrors: string[];
  vitals: {
    lcpMs: number;
    cls: number;
    tbtMs: number;
    ttfbMs: number;
    fcpMs: number;
  } | null;
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
  missing_h1: 'Missing H1',
  canonical_mismatch: 'Canonical mismatch',
  missing_canonical: 'Missing canonical',
  duplicate_title: 'Duplicate title',
  duplicate_meta_description: 'Duplicate meta description',
  title_too_short: 'Title too short',
  meta_description_too_long: 'Meta description too long',
  hreflang_missing_return: 'Hreflang missing return link',
  hreflang_invalid_target: 'Hreflang target not reachable',
  pagination_broken: 'Broken pagination link',
  jsonld_malformed: 'Malformed structured data',
  jsonld_missing_required: 'Structured data missing required fields',
  jsonld_missing: 'No structured data',
  accessibility_violations: 'Accessibility violations',
  poor_lcp: 'Poor Largest Contentful Paint',
  layout_shift: 'Layout shift',
  slow_ttfb: 'Slow server response',
  long_blocking_time: 'Long blocking time',
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
  { key: 'lcp', label: 'LCP' },
  { key: 'cls', label: 'CLS' },
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
  lcp: false,
  cls: false,
};

function statusCodeClass(row: UrlRow): string {
  if (row.blocked) return 'blocked';
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
  const [seedKind, setSeedKind] = useState<'url' | 'sitemap' | 'list'>('url');
  const [includePatterns, setIncludePatterns] = useState('');
  const [excludePatterns, setExcludePatterns] = useState('');
  const [authUser, setAuthUser] = useState('');
  const [authPassword, setAuthPassword] = useState('');
  const [customHeaders, setCustomHeaders] = useState('');
  const [render, setRender] = useState(false);
  const [browserPath, setBrowserPath] = useState('');
  const [renderWaitMs, setRenderWaitMs] = useState(500);
  const [renderWorkers, setRenderWorkers] = useState(2);
  const [screenshots, setScreenshots] = useState(false);
  const [renderBlockAssets, setRenderBlockAssets] = useState(false);
  const [accessibility, setAccessibility] = useState(false);
  const [webVitals, setWebVitals] = useState(false);
  const [storeText, setStoreText] = useState(false);
  const [customJs, setCustomJs] = useState('');
  const [extractors, setExtractors] = useState<{ name: string; selector: string }[]>([]);
  const [extractorName, setExtractorName] = useState('');
  const [extractorSelector, setExtractorSelector] = useState('');
  const [profiles, setProfiles] = useState<{ name: string; updatedAt: number }[]>([]);
  const [profileName, setProfileName] = useState('');
  const [profileSelected, setProfileSelected] = useState('');
  const [profileError, setProfileError] = useState<string | null>(null);
  const [schedules, setSchedules] = useState<
    { id: number; seedKind: string; seed: string; intervalSeconds: number; nextRunAt: number }[]
  >([]);
  const [scheduleKind, setScheduleKind] = useState<'url' | 'sitemap' | 'list'>('url');
  const [scheduleSeed, setScheduleSeed] = useState('');
  const [scheduleEvery, setScheduleEvery] = useState(3600);
  const [scheduleError, setScheduleError] = useState<string | null>(null);
  const [scheduleResult, setScheduleResult] = useState<string | null>(null);
  const [settingsQuery, setSettingsQuery] = useState('');
  const [probe, setProbe] = useState<{ status: number; ms: number; finalUrl: string } | null>(null);
  const [probeError, setProbeError] = useState<string | null>(null);
  const [probeBusy, setProbeBusy] = useState(false);
  const [screenshotUrl, setScreenshotUrl] = useState<string | null>(null);
  const [detail, setDetail] = useState<UrlDetail | null>(null);
  const [detailError, setDetailError] = useState<string | null>(null);
  const [lighthouse, setLighthouse] = useState<LighthouseSummary | null>(null);
  const [lighthouseBusy, setLighthouseBusy] = useState(false);
  const [lighthouseError, setLighthouseError] = useState<string | null>(null);
  const [lighthouseSample, setLighthouseSample] = useState(0);

  const [crawl, setCrawl] = useState<CrawlSummary | null>(null);
  const [phase, setPhase] = useState<CrawlStatus | 'idle'>('idle');
  const [rows, setRows] = useState<UrlRow[]>([]);
  const [total, setTotal] = useState(0);
  const [page, setPage] = useState(1);
  const [progress, setProgress] = useState<CrawlProgress | null>(null);
  const [issues, setIssues] = useState<IssueCount[]>([]);
  const [statusCounts, setStatusCounts] = useState<{ status: number | null; count: number }[]>([]);
  const [activeIssue, setActiveIssue] = useState<string | null>(null);
  const [queryInput, setQueryInput] = useState('');
  const [statusFilter, setStatusFilter] = useState('');
  const [contentFilter, setContentFilter] = useState('');
  const [maxDepthFilter, setMaxDepthFilter] = useState('');
  const [indexFilter, setIndexFilter] = useState('');
  const [views, setViews] = useState<
    {
      name: string;
      search: string | null;
      status: number | null;
      issue: string | null;
      contentClass: string | null;
      maxDepth: number | null;
      indexability: string | null;
    }[]
  >([]);
  const [viewName, setViewName] = useState('');
  const [viewSelected, setViewSelected] = useState('');
  const [error, setError] = useState<string | null>(null);
  const [exportPath, setExportPath] = useState<string | null>(null);
  const [bulkView, setBulkView] = useState('accessibility');
  const [reportKind, setReportKind] = useState('summary');
  const [reportTop, setReportTop] = useState('');
  const [reportStatus, setReportStatus] = useState('');
  const [reportTitle, setReportTitle] = useState('');
  const [exportPattern, setExportPattern] = useState('');
  const [skipEmpty, setSkipEmpty] = useState(false);
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
        query: queryInput.trim() || null,
        status: /^\d{3}$/.test(statusFilter) ? Number(statusFilter) : null,
        statusClass: /^[2-5]xx$/.test(statusFilter) ? Number(statusFilter[0]) : null,
        contentClass: contentFilter || null,
        maxDepth: maxDepthFilter.trim() ? Number(maxDepthFilter) : null,
        indexability: indexFilter || null,
      });
      setRows(data.rows);
      setTotal(data.total);
    },
    [queryInput, statusFilter],
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
        const { invoke } = await import('@tauri-apps/api/core');
        const summary = await invoke<CrawlSummary>('get_crawl', {
          crawlId: event.payload.crawlId,
        });
        setCrawl(summary);
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

  useEffect(() => {
    if (!isTauri) return;
    void (async () => {
      try {
        const { invoke } = await import('@tauri-apps/api/core');
        const rows = await invoke<
          {
            name: string;
            search: string | null;
            status: number | null;
            issue: string | null;
            contentClass: string | null;
            maxDepth: number | null;
            indexability: string | null;
          }[]
        >('list_views');
        setViews(rows);
      } catch {
        // views are optional
      }
    })();
  }, [isTauri]);

  const loadViews = async () => {
    const { invoke } = await import('@tauri-apps/api/core');
    const rows = await invoke<
      {
            name: string;
            search: string | null;
            status: number | null;
            issue: string | null;
            contentClass: string | null;
            maxDepth: number | null;
            indexability: string | null;
          }[]
    >('list_views');
    setViews(rows);
  };
  const saveView = async () => {
    const name = viewName.trim();
    if (!name) return;
    const { invoke } = await import('@tauri-apps/api/core');
    await invoke('save_view', {
      name,
      search: queryInput.trim() || null,
      status: statusFilter ? Number(statusFilter) : null,
      issue: activeIssue,
    });
    setViewName('');
    setViewSelected(name);
    await loadViews();
  };
  const applyView = async (name: string) => {
    const view = views.find((entry) => entry.name === name);
    if (!view) return;
    setQueryInput(view.search ?? '');
    setStatusFilter(view.status ? String(view.status) : '');
    setActiveIssue(view.issue);
    setPage(1);
    if (crawl) await loadPage(crawl.id, 1, view.issue);
  };
  const removeView = async (name: string) => {
    const { invoke } = await import('@tauri-apps/api/core');
    await invoke('delete_view', { name });
    setViewSelected('');
    await loadViews();
  };

  useEffect(() => {
    if (!isTauri || !crawl) return;
    void (async () => {
      try {
        const { invoke } = await import('@tauri-apps/api/core');
        const rows = await invoke<{ status: number | null; count: number }[]>(
          'status_counts',
          { crawlId: crawl.id },
        );
        setStatusCounts(rows);
      } catch {
        // overview is best-effort
      }
    })();
  }, [isTauri, crawl?.id, phase]);

  const applyTab = (content: string, status: string) => {
    setContentFilter(content);
    setStatusFilter(status);
    setPage(1);
    if (crawl) void loadPage(crawl.id, 1, activeIssue);
  };

  const clearFilters = () => {
    setQueryInput('');
    setStatusFilter('');
    setContentFilter('');
    setMaxDepthFilter('');
    setIndexFilter('');
    setActiveIssue(null);
    setPage(1);
    if (crawl) void loadPage(crawl.id, 1, null);
  };

  const openDetail = async (row: UrlRow) => {
    try {
      const { invoke } = await import('@tauri-apps/api/core');
      const data = await invoke<UrlDetail>('get_url_detail', {
        crawlId: row.crawlId,
        urlId: row.id,
      });
      setDetail(data);
      setDetailError(null);
    } catch (cause) {
      setDetailError(cause instanceof Error ? cause.message : String(cause));
    }
  };

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
        includePatterns: splitLines(includePatterns),
        excludePatterns: splitLines(excludePatterns),
        authUser: authUser.trim() || null,
        authPassword: authPassword || null,
        customHeaders: splitLines(customHeaders),
        render,
        browserPath: browserPath.trim() || null,
        renderWaitMs,
        renderWorkers,
        screenshots,
        renderBlockAssets,
        accessibility,
        webVitals,
        storeText,
        customJs: splitLines(customJs),
      };
      const summary = await invoke<CrawlSummary>('create_crawl', {
        seedKind,
        seed,
        config,
      });
      setCrawl(summary);
      setPhase('running');
      for (const extractor of extractors) {
        await invoke('add_extractor', {
          crawlId: summary.id,
          name: extractor.name,
          selector: extractor.selector,
        });
      }
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
      const path = await invoke<string>('export_view', {
        crawlId: crawl.id,
                        pattern: exportPattern.trim() || null,
                        skipEmpty,
        view: 'urls',
        issue: activeIssue,
        columns: Object.entries(columns)
          .filter(([, enabled]) => enabled)
          .map(([key]) => key),
      });
      setExportPath(path);
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    }
  }

  async function exportIssues() {
    if (!crawl) return;
    try {
      const { invoke } = await import('@tauri-apps/api/core');
      const path = await invoke<string>('export_view', {
        crawlId: crawl.id,
                        pattern: exportPattern.trim() || null,
                        skipEmpty,
        view: 'issues',
        issue: null,
        columns: [],
      });
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
  const settingsSnapshot = () => ({
    seed,
    seedKind,
    maxDepth,
    maxUrls,
    concurrency,
    sameHostOnly,
    respectRobots,
    includePatterns,
    excludePatterns,
    authUser,
    authPassword,
    customHeaders,
    render,
    browserPath,
    renderWaitMs,
    renderWorkers,
    screenshots,
    renderBlockAssets,
    accessibility,
    webVitals,
    storeText,
    customJs,
  });
  const applySettings = (value: Record<string, unknown>) => {
    if (typeof value.seed === 'string') setSeed(value.seed);
    if (value.seedKind === 'url' || value.seedKind === 'sitemap' || value.seedKind === 'list') {
      setSeedKind(value.seedKind);
    }
    if (typeof value.maxDepth === 'number') setMaxDepth(value.maxDepth);
    if (typeof value.maxUrls === 'number') setMaxUrls(value.maxUrls);
    if (typeof value.concurrency === 'number') setConcurrency(value.concurrency);
    if (typeof value.sameHostOnly === 'boolean') setSameHostOnly(value.sameHostOnly);
    if (typeof value.respectRobots === 'boolean') setRespectRobots(value.respectRobots);
    if (typeof value.includePatterns === 'string') setIncludePatterns(value.includePatterns);
    if (typeof value.excludePatterns === 'string') setExcludePatterns(value.excludePatterns);
    if (typeof value.authUser === 'string') setAuthUser(value.authUser);
    if (typeof value.authPassword === 'string') setAuthPassword(value.authPassword);
    if (typeof value.customHeaders === 'string') setCustomHeaders(value.customHeaders);
    if (typeof value.render === 'boolean') setRender(value.render);
    if (typeof value.browserPath === 'string') setBrowserPath(value.browserPath);
    if (typeof value.renderWaitMs === 'number') setRenderWaitMs(value.renderWaitMs);
    if (typeof value.renderWorkers === 'number') setRenderWorkers(value.renderWorkers);
    if (typeof value.screenshots === 'boolean') setScreenshots(value.screenshots);
    if (typeof value.renderBlockAssets === 'boolean') setRenderBlockAssets(value.renderBlockAssets);
    if (typeof value.accessibility === 'boolean') setAccessibility(value.accessibility);
    if (typeof value.webVitals === 'boolean') setWebVitals(value.webVitals);
    if (typeof value.storeText === 'boolean') setStoreText(value.storeText);
    if (typeof value.customJs === 'string') setCustomJs(value.customJs);
  };
  const loadProfiles = async () => {
    try {
      const { invoke } = await import('@tauri-apps/api/core');
      const rows = await invoke<[string, number][]>('list_profiles');
      setProfiles(rows.map(([name, updatedAt]) => ({ name, updatedAt })));
    } catch (cause) {
      setProfileError(cause instanceof Error ? cause.message : String(cause));
    }
  };
  const saveProfile = async () => {
    if (!profileName.trim()) {
      setProfileError('Profile name is required.');
      return;
    }
    try {
      const { invoke } = await import('@tauri-apps/api/core');
      await invoke('save_profile', {
        name: profileName.trim(),
        configJson: JSON.stringify(settingsSnapshot()),
      });
      setProfileError(null);
      setProfileSelected(profileName.trim());
      await loadProfiles();
    } catch (cause) {
      setProfileError(cause instanceof Error ? cause.message : String(cause));
    }
  };
  const loadProfile = async (name: string) => {
    if (!name) return;
    try {
      const { invoke } = await import('@tauri-apps/api/core');
      const json = await invoke<string | null>('get_profile', { name });
      if (json) applySettings(JSON.parse(json) as Record<string, unknown>);
      setProfileError(null);
    } catch (cause) {
      setProfileError(cause instanceof Error ? cause.message : String(cause));
    }
  };
  const removeProfile = async () => {
    if (!profileSelected) return;
    try {
      const { invoke } = await import('@tauri-apps/api/core');
      await invoke('delete_profile', { name: profileSelected });
      setProfileSelected('');
      await loadProfiles();
    } catch (cause) {
      setProfileError(cause instanceof Error ? cause.message : String(cause));
    }
  };
  useEffect(() => {
    if (isTauri) void loadProfiles();
  }, [isTauri]);
  const loadSchedules = async () => {
    try {
      const { invoke } = await import('@tauri-apps/api/core');
      const rows = await invoke<
        { id: number; seedKind: string; seed: string; intervalSeconds: number; nextRunAt: number }[]
      >('list_schedules');
      setSchedules(rows);
    } catch (cause) {
      setScheduleError(cause instanceof Error ? cause.message : String(cause));
    }
  };
  const addSchedule = async () => {
    if (!scheduleSeed.trim() || scheduleEvery < 1) {
      setScheduleError('Seed and an interval of at least 1 second are required.');
      return;
    }
    try {
      const { invoke } = await import('@tauri-apps/api/core');
      await invoke('add_schedule', {
        seedKind: scheduleKind,
        seed: scheduleSeed.trim(),
        everySeconds: scheduleEvery,
      });
      setScheduleSeed('');
      setScheduleError(null);
      await loadSchedules();
    } catch (cause) {
      setScheduleError(cause instanceof Error ? cause.message : String(cause));
    }
  };
  const removeSchedule = async (id: number) => {
    try {
      const { invoke } = await import('@tauri-apps/api/core');
      await invoke('remove_schedule', { id });
      await loadSchedules();
    } catch (cause) {
      setScheduleError(cause instanceof Error ? cause.message : String(cause));
    }
  };
  const runDueSchedules = async () => {
    try {
      const { invoke } = await import('@tauri-apps/api/core');
      const results = await invoke<{ id: number; rows?: number; error?: string }[]>(
        'run_due_schedules',
      );
      setScheduleResult(
        results.length === 0
          ? 'No schedules due.'
          : results
              .map((result) =>
                result.error
                  ? `Schedule ${result.id} failed: ${result.error}`
                  : `Schedule ${result.id}: ${result.rows} rows`,
              )
              .join(' · '),
      );
      setScheduleError(null);
      await loadSchedules();
    } catch (cause) {
      setScheduleError(cause instanceof Error ? cause.message : String(cause));
    }
  };
  useEffect(() => {
    if (isTauri) void loadSchedules();
  }, [isTauri]);
  const settingEntries = [
    { label: 'Max depth', help: 'How many link levels to follow from the seed.', value: String(maxDepth), changed: maxDepth !== 3 },
    { label: 'Max URLs', help: 'Hard cap on crawled URLs.', value: String(maxUrls), changed: maxUrls !== 250 },
    { label: 'Concurrency', help: 'Parallel requests.', value: String(concurrency), changed: concurrency !== 8 },
    { label: 'Stay on seed host', help: 'Restrict the crawl to the seed host.', value: sameHostOnly ? 'on' : 'off', changed: !sameHostOnly },
    { label: 'Respect robots.txt', help: 'Honour robots.txt directives.', value: respectRobots ? 'on' : 'off', changed: !respectRobots },
    { label: 'Render with Chromium', help: 'Run JavaScript pages before extraction.', value: render ? 'on' : 'off', changed: render },
    { label: 'Render wait', help: 'Milliseconds to wait after load.', value: `${renderWaitMs} ms`, changed: renderWaitMs !== 500 },
    { label: 'Render workers', help: 'Concurrent rendering tabs.', value: String(renderWorkers), changed: renderWorkers !== 2 },
    { label: 'Screenshots', help: 'Save a JPEG per rendered page.', value: screenshots ? 'on' : 'off', changed: screenshots },
    { label: 'Block third-party assets', help: 'Skip third-party requests while rendering.', value: renderBlockAssets ? 'on' : 'off', changed: renderBlockAssets },
    { label: 'Accessibility (axe)', help: 'Run axe-core on rendered pages.', value: accessibility ? 'on' : 'off', changed: accessibility },
    { label: 'Web Vitals', help: 'Capture LCP, CLS, TBT, TTFB and FCP.', value: webVitals ? 'on' : 'off', changed: webVitals },
    { label: 'Store page text', help: 'Save extracted text and Markdown for exports.', value: storeText ? 'on' : 'off', changed: storeText },
    { label: 'Browser path', help: 'Chrome/Edge executable override.', value: browserPath || 'auto', changed: browserPath.length > 0 },
    { label: 'Include patterns', help: 'One regex per line; only matching URLs are crawled.', value: includePatterns ? 'set' : 'none', changed: includePatterns.length > 0 },
    { label: 'Exclude patterns', help: 'One regex per line; matching URLs are skipped.', value: excludePatterns ? 'set' : 'none', changed: excludePatterns.length > 0 },
    { label: 'Basic auth user', help: 'HTTP basic authentication username.', value: authUser || 'none', changed: authUser.length > 0 },
    { label: 'Custom headers', help: 'Header: value lines sent with every request.', value: customHeaders ? 'set' : 'none', changed: customHeaders.length > 0 },
  ];
  const validationErrors: string[] = [];
  if (seedKind !== 'list') {
    try {
      const parsed = new URL(seed);
      if (!['http:', 'https:'].includes(parsed.protocol)) {
        validationErrors.push('Seed must be an http(s) URL.');
      }
    } catch {
      validationErrors.push('Seed must be a valid URL.');
    }
  } else if (seed.trim().length === 0) {
    validationErrors.push('Provide a URL list path or contents.');
  }
  if (maxDepth < 0 || maxDepth > 100) validationErrors.push('Max depth must be 0–100.');
  if (maxUrls < 1 || maxUrls > 1_000_000) validationErrors.push('Max URLs must be 1–1,000,000.');
  if (concurrency < 1 || concurrency > 64) validationErrors.push('Concurrency must be 1–64.');
  if (renderWaitMs < 0 || renderWaitMs > 60_000) validationErrors.push('Render wait must be 0–60,000 ms.');
  if (renderWorkers < 1 || renderWorkers > 16) validationErrors.push('Render workers must be 1–16.');
  for (const line of customHeaders.split('\n')) {
    const trimmed = line.trim();
    if (trimmed.length > 0 && !trimmed.includes(':')) {
      validationErrors.push(`Header "${trimmed}" must be "Name: value".`);
    }
  }
  if (authUser.length > 0 !== authPassword.length > 0) {
    validationErrors.push('Basic auth needs both a user and a password.');
  }
  for (const [label, patterns] of [
    ['Include', includePatterns],
    ['Exclude', excludePatterns],
  ] as const) {
    for (const line of patterns.split('\n')) {
      const trimmed = line.trim();
      if (trimmed.length === 0) continue;
      try {
        new RegExp(trimmed);
      } catch {
        validationErrors.push(`${label} pattern is not a valid regex: ${trimmed}`);
      }
    }
  }
  const canStart = validationErrors.length === 0;
  const changedSettings = settingEntries.filter((entry) => entry.changed);
  const filteredSettings = settingEntries.filter((entry) =>
    `${entry.label} ${entry.help}`.toLowerCase().includes(settingsQuery.trim().toLowerCase()),
  );
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

  const classCounts = useMemo(() => {    const totals: Record<string, number> = {
      '2xx': 0,
      '3xx': 0,
      '4xx': 0,
      '5xx': 0,
      other: 0,
    };
    for (const row of statusCounts) {
      if (!row.status) {
        totals.other += row.count;
        continue;
      }
      const key = `${Math.floor(row.status / 100)}xx`;
      totals[key] = (totals[key] ?? 0) + row.count;
    }
    return totals;
  }, [statusCounts]);

  const severityTotals = useMemo(() => {
    const totals = { error: 0, warning: 0, opportunity: 0 };
    for (const issue of issues) {
      totals[issue.severity] += issue.count;
    }
    return totals;
  }, [issues]);

  const activeFilterCount = [
    queryInput.trim(),
    statusFilter,
    contentFilter,
    maxDepthFilter.trim(),
    indexFilter,
    activeIssue ?? '',
  ].filter(Boolean).length;

  const resultTabs = [
    { id: 'all', label: 'All', content: '', status: '' },
    { id: 'html', label: 'HTML', content: 'html', status: '' },
    { id: 'images', label: 'Images', content: 'image', status: '' },
    { id: 'xml', label: 'XML', content: 'xml', status: '' },
    { id: 'redirects', label: 'Redirects', content: '', status: '3xx' },
    { id: 'errors', label: 'Errors', content: '', status: '4xx' },
  ] as const;
  const activeTab =
    statusFilter === '3xx'
      ? 'redirects'
      : statusFilter === '4xx' || statusFilter === '5xx'
        ? 'errors'
        : contentFilter || 'all';

  function cell(row: UrlRow, key: ColumnKey) {
    switch (key) {
      case 'status':
        return (
          <div className="status-cell">
            <span
              className={`status-chip ${statusCodeClass(row)}`}
              title={row.blocked ? 'Blocked by the site edge (WAF challenge)' : undefined}
            >
              {row.blocked ? 'WAF' : row.error ? 'ERR' : (row.status ?? '—')}
            </span>
            {row.consoleErrors.length > 0 && (
              <span
                className="js-errors"
                title={row.consoleErrors.join('\n')}
              >
                JS {row.consoleErrors.length}
              </span>
            )}
            {crawl?.config.screenshots && (
              <button
                className="link"
                type="button"
                onClick={async () => {
                  const { invoke } = await import('@tauri-apps/api/core');
                  const data = await invoke<string | null>('get_screenshot', {
                    crawlId: row.crawlId,
                    urlId: row.id,
                  });
                  setScreenshotUrl(data);
                }}
              >
                Shot
              </button>
            )}
            <button
              className="link"
              type="button"
              disabled={lighthouseBusy}
              onClick={async () => {
                setLighthouseBusy(true);
                setLighthouseError(null);
                try {
                  const { invoke } = await import('@tauri-apps/api/core');
                  const report = await invoke<LighthouseSummary>('run_lighthouse', {
                    crawlId: row.crawlId,
                    urlId: row.id,
                  });
                  setLighthouse(report);
                } catch (cause) {
                  setLighthouseError(cause instanceof Error ? cause.message : String(cause));
                } finally {
                  setLighthouseBusy(false);
                }
              }}
            >
              {lighthouseBusy ? 'LH…' : 'LH'}
            </button>
          </div>
        );
      case 'lcp':
        return row.vitals ? `${(row.vitals.lcpMs / 1000).toFixed(2)}s` : '—';
      case 'cls':
        return row.vitals ? row.vitals.cls.toFixed(3) : '—';
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
          <span className="badge">M4 · crawl, audit &amp; render</span>
        </div>
        <div className="topbar-right">
          <span className="badge">{isTauri ? 'Desktop' : 'Browser preview'}</span>
        </div>
      </header>

      <div className="layout">
        <aside className="panel sidebar">
          <h2>New crawl</h2>
          <p className="hint">
            Crawl from a URL, sitemap or URL list with regex scope, basic auth, robots.txt,
            optional Chromium rendering, screenshots, axe accessibility, Web Vitals and 24 audit rules. Every row lands in a
            portable SQLite crawl file.
          </p>
          <form onSubmit={startCrawl}>
            <div className="field">
              <label htmlFor="seedKind">Seed type</label>
              <select
                id="seedKind"
                value={seedKind}
                onChange={(event) =>
                  setSeedKind(event.target.value as 'url' | 'sitemap' | 'list')
                }
                disabled={isRunning}
              >
                <option value="url">Single URL</option>
                <option value="sitemap">Sitemap URL</option>
                <option value="list">URL list file</option>
              </select>
            </div>
            <div className="field">
              <label htmlFor="seed">
                {seedKind === 'url'
                  ? 'Start URL'
                  : seedKind === 'sitemap'
                    ? 'Sitemap URL'
                    : 'Path to URL list'}
              </label>
              <input
                id="seed"
                type={seedKind === 'list' ? 'text' : 'url'}
                value={seed}
                onChange={(event) => setSeed(event.target.value)}
                placeholder={
                  seedKind === 'url'
                    ? 'https://example.com'
                    : seedKind === 'sitemap'
                      ? 'https://example.com/sitemap.xml'
                      : 'C:\\crawls\\urls.txt'
                }
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
            <details className="advanced">
              <summary>Scope, auth & headers</summary>
              <div className="field">
                <label>Custom extraction</label>
                <div style={{ display: 'flex', gap: 8 }}>
                  <input
                    type="text"
                    value={extractorName}
                    onChange={(event) => setExtractorName(event.target.value)}
                    placeholder="Name"
                    aria-label="Extractor name"
                  />
                  <input
                    type="text"
                    value={extractorSelector}
                    onChange={(event) => setExtractorSelector(event.target.value)}
                    placeholder="CSS selector or selector@attr"
                    aria-label="Extractor selector"
                  />
                  <button
                    className="secondary"
                    type="button"
                    onClick={() => {
                      const name = extractorName.trim();
                      const selector = extractorSelector.trim();
                      if (!name || !selector) return;
                      setExtractors((current) => [
                        ...current.filter((entry) => entry.name !== name),
                        { name, selector },
                      ]);
                      setExtractorName('');
                      setExtractorSelector('');
                    }}
                  >
                    Add
                  </button>
                </div>
                <ul className="settings-reference">
                  {extractors.map((extractor) => (
                    <li key={extractor.name}>
                      <strong>{extractor.name}</strong> — {extractor.selector}{' '}
                      <button
                        className="secondary"
                        type="button"
                        onClick={() =>
                          setExtractors((current) =>
                            current.filter((entry) => entry.name !== extractor.name),
                          )
                        }
                      >
                        Remove
                      </button>
                    </li>
                  ))}
                  {extractors.length === 0 && <li>No extractors yet.</li>}
                </ul>
              </div>
              <div className="field">
                <label htmlFor="profileName">Profiles</label>
                <div style={{ display: 'flex', gap: 8 }}>
                  <input
                    id="profileName"
                    type="text"
                    value={profileName}
                    onChange={(event) => setProfileName(event.target.value)}
                    placeholder="Profile name"
                  />
                  <button className="secondary" type="button" onClick={saveProfile}>
                    Save
                  </button>
                </div>
                <div style={{ display: 'flex', gap: 8, marginTop: 8 }}>
                  <select
                    value={profileSelected}
                    onChange={(event) => {
                      setProfileSelected(event.target.value);
                      void loadProfile(event.target.value);
                    }}
                    aria-label="Load profile"
                  >
                    <option value="">Load profile…</option>
                    {profiles.map((profile) => (
                      <option key={profile.name} value={profile.name}>
                        {profile.name}
                      </option>
                    ))}
                  </select>
                  <button
                    className="secondary"
                    type="button"
                    onClick={removeProfile}
                    disabled={!profileSelected}
                  >
                    Delete
                  </button>
                </div>
                {profileError && <p className="settings-errors">{profileError}</p>}
              </div>
              <div className="field">
                <label>Schedules</label>
                <div style={{ display: 'flex', gap: 8 }}>
                  <select
                    value={scheduleKind}
                    onChange={(event) =>
                      setScheduleKind(event.target.value as 'url' | 'sitemap' | 'list')
                    }
                    aria-label="Schedule seed kind"
                  >
                    <option value="url">URL</option>
                    <option value="sitemap">Sitemap</option>
                    <option value="list">List</option>
                  </select>
                  <input
                    type="text"
                    value={scheduleSeed}
                    onChange={(event) => setScheduleSeed(event.target.value)}
                    placeholder="Seed"
                    aria-label="Schedule seed"
                  />
                  <input
                    type="number"
                    min={1}
                    value={scheduleEvery}
                    onChange={(event) => setScheduleEvery(Number(event.target.value))}
                    aria-label="Schedule interval seconds"
                  />
                  <button className="secondary" type="button" onClick={addSchedule}>
                    Add schedule
                  </button>
                  <button className="secondary" type="button" onClick={runDueSchedules}>
                    Run due now
                  </button>
                </div>
                <ul className="settings-reference">
                  {schedules.map((schedule) => (
                    <li key={schedule.id}>
                      <strong>{schedule.seedKind}</strong> {schedule.seed} · every{' '}
                      {schedule.intervalSeconds}s · next{' '}
                      {new Date(schedule.nextRunAt).toLocaleString()}{' '}
                      <button
                        className="secondary"
                        type="button"
                        onClick={() => removeSchedule(schedule.id)}
                      >
                        Remove
                      </button>
                    </li>
                  ))}
                  {schedules.length === 0 && <li>No schedules yet.</li>}
                </ul>
                {scheduleError && <p className="settings-errors">{scheduleError}</p>}
                {scheduleResult && <p className="settings-diff">{scheduleResult}</p>}
              </div>
              <div className="field">
                <label htmlFor="settingsSearch">Find a setting</label>
                <input
                  id="settingsSearch"
                  type="search"
                  value={settingsQuery}
                  onChange={(event) => setSettingsQuery(event.target.value)}
                  placeholder="depth, render, vitals, auth…"
                />
              </div>
              {settingsQuery.trim().length > 0 && (
                <ul className="settings-reference">
                  {filteredSettings.map((entry) => (
                    <li key={entry.label}>
                      <strong>{entry.label}</strong> — {entry.help}{' '}
                      <span className="muted">({entry.value})</span>
                    </li>
                  ))}
                  {filteredSettings.length === 0 && <li>No settings match.</li>}
                </ul>
              )}
              <div className="field">
                <label htmlFor="include">Include patterns (one regex per line)</label>
                <textarea
                  id="include"
                  rows={2}
                  value={includePatterns}
                  onChange={(event) => setIncludePatterns(event.target.value)}
                  placeholder="/blog/.*"
                  disabled={isRunning}
                />
              </div>
              <div className="field">
                <label htmlFor="exclude">Exclude patterns</label>
                <textarea
                  id="exclude"
                  rows={2}
                  value={excludePatterns}
                  onChange={(event) => setExcludePatterns(event.target.value)}
                  placeholder="/tag/.*"
                  disabled={isRunning}
                />
              </div>
              <div className="grid-2">
                <div className="field">
                  <label htmlFor="authUser">Basic auth user</label>
                  <input
                    id="authUser"
                    type="text"
                    value={authUser}
                    onChange={(event) => setAuthUser(event.target.value)}
                    disabled={isRunning}
                    autoComplete="off"
                  />
                </div>
                <div className="field">
                  <label htmlFor="authPassword">Password</label>
                  <input
                    id="authPassword"
                    type="password"
                    value={authPassword}
                    onChange={(event) => setAuthPassword(event.target.value)}
                    disabled={isRunning}
                    autoComplete="off"
                  />
                </div>
              </div>
              <div className="field">
                <label htmlFor="headers">Custom headers (one per line)</label>
                <textarea
                  id="headers"
                  rows={2}
                  value={customHeaders}
                  onChange={(event) => setCustomHeaders(event.target.value)}
                  placeholder="X-Robots-Tag: noindex"
                  disabled={isRunning}
                />
              </div>
              <label className="checkbox">
                <input
                  type="checkbox"
                  checked={render}
                  onChange={(event) => setRender(event.target.checked)}
                  disabled={isRunning}
                />
                Render JavaScript in headless Chrome/Edge
              </label>
              {render && (
                <div className="grid-2">
                  <div className="field">
                    <label htmlFor="browserPath">Browser path (optional)</label>
                    <input
                      id="browserPath"
                      type="text"
                      value={browserPath}
                      onChange={(event) => setBrowserPath(event.target.value)}
                      placeholder="Auto-detect Chrome/Edge"
                      disabled={isRunning}
                    />
                  </div>
                  <div className="field">
                    <label htmlFor="renderWait">Wait after load (ms)</label>
                    <input
                      id="renderWait"
                      type="number"
                      min={0}
                      step={100}
                      value={renderWaitMs}
                      onChange={(event) => setRenderWaitMs(Number(event.target.value))}
                      disabled={isRunning}
                    />
                  </div>
                  <div className="field">
                    <label htmlFor="renderWorkers">Render workers</label>
                    <input
                      id="renderWorkers"
                      type="number"
                      min={1}
                      max={8}
                      value={renderWorkers}
                      onChange={(event) => setRenderWorkers(Number(event.target.value))}
                      disabled={isRunning}
                    />
                  </div>
                </div>
              )}
              {render && (
                <>
                  <label className="checkbox">
                    <input
                      type="checkbox"
                      checked={screenshots}
                      onChange={(event) => setScreenshots(event.target.checked)}
                      disabled={isRunning}
                    />
                    Capture a screenshot per rendered page
                  </label>
                  <label className="checkbox">
                    <input
                      type="checkbox"
                      checked={renderBlockAssets}
                      onChange={(event) => setRenderBlockAssets(event.target.checked)}
                      disabled={isRunning}
                    />
                    Block fonts and media while rendering
                  </label>
                  <label className="checkbox">
                    <input
                      type="checkbox"
                      checked={accessibility}
                      onChange={(event) => setAccessibility(event.target.checked)}
                      disabled={isRunning}
                    />
                    Run axe accessibility audit
                  </label>
                  <label className="checkbox">
                    <input
                      type="checkbox"
                      checked={webVitals}
                      onChange={(event) => setWebVitals(event.target.checked)}
                      disabled={isRunning}
                    />
                    Capture Web Vitals (LCP, CLS, TBT)
                  </label>
                  <label className="checkbox">
                    <input
                      type="checkbox"
                      checked={storeText}
                      onChange={(event) => setStoreText(event.target.checked)}
                      disabled={isRunning}
                    />
                    Store page text + Markdown
                  </label>
                  <div className="field">
                    <label htmlFor="customJs">Custom JavaScript (one expression per line)</label>
                    <textarea
                      id="customJs"
                      rows={3}
                      value={customJs}
                      onChange={(event) => setCustomJs(event.target.value)}
                      placeholder="document.title"
                      disabled={isRunning || !render}
                    />
                  </div>
                </>
              )}
            </details>
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
            {validationErrors.length > 0 && (
              <ul className="settings-errors">
                {validationErrors.map((message) => (
                  <li key={message}>{message}</li>
                ))}
              </ul>
            )}
            <button
              className="secondary"
              type="button"
              onClick={async () => {
                setProbeBusy(true);
                setProbeError(null);
                setProbe(null);
                try {
                  const { invoke } = await import('@tauri-apps/api/core');
                  const result = await invoke<{ status: number; ms: number; finalUrl: string }>(
                    'test_connection',
                    {
                      seed,
                      headers: customHeaders
                        .split('\n')
                        .map((line) => line.trim())
                        .filter(Boolean),
                    },
                  );
                  setProbe(result);
                } catch (cause) {
                  setProbeError(cause instanceof Error ? cause.message : String(cause));
                } finally {
                  setProbeBusy(false);
                }
              }}
              disabled={!isTauri || busy || isRunning || probeBusy}
            >
              {probeBusy ? 'Testing…' : 'Test connection'}
            </button>
            {probe && (
              <p className="settings-diff">
                Connected: HTTP {probe.status} · {probe.ms} ms · {probe.finalUrl}
              </p>
            )}
            {probeError && <p className="settings-errors">{probeError}</p>}
            <p className="settings-diff">
              {changedSettings.length === 0
                ? 'Default configuration.'
                : `Diff from defaults: ${changedSettings
                    .map((entry) => `${entry.label} ${entry.value}`)
                    .join(' · ')}`}
            </p>
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
              <button
                className="primary"
                type="submit"
                disabled={!isTauri || busy || isRunning || !canStart}
              >
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
            Next up: bundled Node + Lighthouse for offline audits.
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

          {lighthouseError && <div className="error">{lighthouseError}</div>}

          {lighthouse && (
            <div
              className="modal-backdrop"
              role="presentation"
              onClick={() => setLighthouse(null)}
            >
              <div className="modal" role="dialog" aria-label="Lighthouse report">
                <h3>
                  Lighthouse
                  {lighthouseSample > 1 ? ` (sampled ${lighthouseSample} pages)` : ''}
                </h3>
                <div className="lh-scores">
                  {(
                    [
                      ['Performance', lighthouse.performance],
                      ['Accessibility', lighthouse.accessibility],
                      ['Best practices', lighthouse.bestPractices],
                      ['SEO', lighthouse.seo],
                    ] as const
                  ).map(([label, score]) => (
                    <div key={label} className="lh-score">
                      <span>{label}</span>
                      <strong>{score == null ? '—' : Math.round(score * 100)}</strong>
                    </div>
                  ))}
                </div>
                <ul className="lh-audits">
                  {lighthouse.audits.map((audit) => (
                    <li key={audit.id}>
                      <strong>{audit.title}</strong>
                      {audit.displayValue ? ` — ${audit.displayValue}` : ''}
                      <span> ({audit.score == null ? '—' : Math.round(audit.score * 100)})</span>
                    </li>
                  ))}
                </ul>
                <button className="secondary" type="button" onClick={() => setLighthouse(null)}>
                  Close
                </button>
              </div>
            </div>
          )}

          {detail && (
            <div
              className="modal-backdrop"
              role="presentation"
              onClick={() => setDetail(null)}
            >
              <div
                className="modal"
                role="dialog"
                aria-label="URL detail"
                onClick={(event) => event.stopPropagation()}
              >
                <h3>{String(detail.url.url ?? '')}</h3>
                <ul className="settings-reference">
                  <li>Status: {String(detail.url.status ?? '—')}</li>
                  <li>Title: {String(detail.url.title ?? '—')}</li>
                  <li>H1: {String(detail.url.h1 ?? '—')}</li>
                  <li>Canonical: {String(detail.url.canonical ?? '—')}</li>
                  <li>Robots: {String(detail.url.metaRobots ?? '—')}</li>
                  <li>Depth: {String(detail.url.depth ?? '—')}</li>
                  <li>Response: {String(detail.url.responseTimeMs ?? '—')} ms</li>
                  <li>Words: {String(detail.url.wordCount ?? '—')}</li>
                  <li>
                    Links: {detail.linkCounts.inbound} in / {detail.linkCounts.outbound} out
                  </li>
                </ul>
                {Array.isArray(detail.url.axe) && detail.url.axe.length > 0 && (
                  <>
                    <h4>Axe</h4>
                    <ul className="settings-reference">
                      {(detail.url.axe as { id: string; help: string; impact?: string }[]).map(
                        (violation) => (
                          <li key={violation.id}>
                            {violation.id} ({violation.impact ?? 'n/a'}): {violation.help}
                          </li>
                        ),
                      )}
                    </ul>
                  </>
                )}
                {Array.isArray(detail.url.headers) && detail.url.headers.length > 0 && (
                  <>
                    <h4>Headers</h4>
                    <ul className="settings-reference">
                      {(detail.url.headers as [string, string][]).map(([name, value]) => (
                        <li key={name}>
                          {name}: {value}
                        </li>
                      ))}
                    </ul>
                  </>
                )}
                {detail.extractions.length > 0 && (
                  <>
                    <h4>Extractions</h4>
                    <ul className="settings-reference">
                      {detail.extractions.map((entry) => (
                        <li key={entry.name}>
                          {entry.name}: {entry.value}
                        </li>
                      ))}
                    </ul>
                  </>
                )}
                {detail.customJs.length > 0 && (
                  <>
                    <h4>Custom JS</h4>
                    <ul className="settings-reference">
                      {detail.customJs.map((entry) => (
                        <li key={entry.index}>
                          #{entry.index}: {entry.value}
                        </li>
                      ))}
                    </ul>
                  </>
                )}
                {Array.isArray(
                  (detail.url.structuredData as { types?: string[] } | undefined)?.types,
                ) &&
                  ((detail.url.structuredData as { types?: string[] }).types ?? []).length >
                    0 && (
                    <>
                      <h4>Structured data</h4>
                      <ul className="settings-reference">
                        {((detail.url.structuredData as { types?: string[] }).types ?? []).map(
                          (type) => (
                            <li key={type}>{type}</li>
                          ),
                        )}
                      </ul>
                    </>
                  )}
                {Array.isArray(detail.url.hreflang) && detail.url.hreflang.length > 0 && (
                  <>
                    <h4>Hreflang</h4>
                    <ul className="settings-reference">
                      {(detail.url.hreflang as { lang?: string; url: string }[]).map((link) => (
                        <li key={link.url}>
                          {link.lang ?? '—'}: {link.url}
                        </li>
                      ))}
                    </ul>
                  </>
                )}
                {detail.screenshot && (
                  <img
                    src={detail.screenshot}
                    alt="Page screenshot"
                    style={{ maxWidth: '100%', borderRadius: 8, marginTop: 12 }}
                  />
                )}
                {detailError && <p className="settings-errors">{detailError}</p>}
                <button className="secondary" type="button" onClick={() => setDetail(null)}>
                  Close
                </button>
              </div>
            </div>
          )}

          {screenshotUrl && (
            <div
              className="modal-backdrop"
              role="presentation"
              onClick={() => setScreenshotUrl(null)}
            >
              <div className="modal" role="dialog" aria-label="Rendered screenshot">
                <img src={screenshotUrl} alt="Rendered page screenshot" />
                <button className="secondary" type="button" onClick={() => setScreenshotUrl(null)}>
                  Close
                </button>
              </div>
            </div>
          )}

          {crawl && crawl.blockedUrls > 0 && (
            <div className="notice">
              {crawl.blockedUrls.toLocaleString()} URLs were blocked by the site edge
              (WAF/challenge) and are excluded from content results. Coverage is incomplete:
              refresh the session and resume the crawl.
            </div>
          )}

          <section className="panel status-card">
            <div>
              <div className="status-title">
                <span className={`dot ${phase}`} />
                {STATUS_LABEL[phase] ?? phase}
                {crawl && <span className="badge">{seedLabel(crawl)}</span>}
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
              {crawl && crawl.blockedUrls > 0 && (
                <div>
                  Blocked
                  <strong>{crawl.blockedUrls}</strong>
                </div>
              )}
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
                {crawl && (
                  <button
                    className="chip"
                    type="button"
                    disabled={lighthouseBusy}
                    onClick={async () => {
                      setLighthouseBusy(true);
                      setLighthouseError(null);
                      try {
                        const { invoke } = await import('@tauri-apps/api/core');
                        const reports = await invoke<LighthouseSummary[]>(
                          'run_lighthouse_sample',
                          { crawlId: crawl.id,
                        pattern: exportPattern.trim() || null,
                        skipEmpty, sample: 3 },
                        );
                        setLighthouseSample(reports.length);
                        setLighthouse(reports[0] ?? null);
                      } catch (cause) {
                        setLighthouseError(
                          cause instanceof Error ? cause.message : String(cause),
                        );
                      } finally {
                        setLighthouseBusy(false);
                      }
                    }}
                  >
                    {lighthouseBusy ? 'LH…' : 'LH ×3'}
                  </button>
                )}
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

          <div className="result-tabs">
            {resultTabs.map((tab) => (
              <button
                key={tab.id}
                type="button"
                className={`tab${activeTab === tab.id ? ' active' : ''}`}
                onClick={() => applyTab(tab.content, tab.status)}
              >
                {tab.label}
              </button>
            ))}
          </div>

          <section className="overview-strip">
            <div>
              <span>Crawled</span>
              <strong>{crawl?.totalUrls ?? 0}</strong>
            </div>
            {(['2xx', '3xx', '4xx', '5xx', 'other'] as const).map((key) => (
              <button
                key={key}
                type="button"
                className={`overview-card${statusFilter === key ? ' active' : ''}`}
                onClick={() => {
                  const next = statusFilter === key ? '' : key;
                  setStatusFilter(next);
                  setPage(1);
                  if (crawl) void loadPage(crawl.id, 1, activeIssue);
                }}
              >
                <span>{key === 'other' ? 'No response' : key}</span>
                <strong>{classCounts[key] ?? 0}</strong>
              </button>
            ))}
            <div>
              <span>Issues</span>
              <strong>{issueTotal}</strong>
            </div>
            <div>
              <span>Errors</span>
              <strong>{severityTotals.error}</strong>
            </div>
            <div>
              <span>Warnings</span>
              <strong>{severityTotals.warning}</strong>
            </div>
            <div>
              <span>Opportunities</span>
              <strong>{severityTotals.opportunity}</strong>
            </div>
          </section>

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
                <input
                  type="search"
                  value={queryInput}
                  onChange={(event) => setQueryInput(event.target.value)}
                  onKeyDown={(event) => {
                    if (event.key === 'Enter' && crawl) {
                      setPage(1);
                      void loadPage(crawl.id, 1, activeIssue);
                    }
                  }}
                  placeholder="Search URL or title"
                  aria-label="Search URLs"
                  disabled={!isTauri || !crawl}
                />
                <select
                  value={statusFilter}
                  onChange={(event) => {
                    setStatusFilter(event.target.value);
                    setPage(1);
                    if (crawl) void loadPage(crawl.id, 1, activeIssue);
                  }}
                  aria-label="Status filter"
                  disabled={!isTauri || !crawl}
                >
                  <option value="">All statuses</option>
                  <option value="2xx">2xx</option>
                  <option value="3xx">3xx</option>
                  <option value="4xx">4xx</option>
                  <option value="5xx">5xx</option>
                  <option value="200">200</option>
                  <option value="301">301</option>
                  <option value="302">302</option>
                  <option value="404">404</option>
                  <option value="500">500</option>
                </select>
                <select
                  value={contentFilter}
                  onChange={(event) => {
                    setContentFilter(event.target.value);
                    setPage(1);
                    if (crawl) void loadPage(crawl.id, 1, activeIssue);
                  }}
                  aria-label="Content type filter"
                  disabled={!isTauri || !crawl}
                >
                  <option value="">All content</option>
                  <option value="html">HTML</option>
                  <option value="xml">XML</option>
                  <option value="image">Images</option>
                  <option value="other">Other</option>
                </select>
                <input
                  type="number"
                  min={0}
                  value={maxDepthFilter}
                  onChange={(event) => {
                    setMaxDepthFilter(event.target.value);
                    setPage(1);
                    if (crawl) void loadPage(crawl.id, 1, activeIssue);
                  }}
                  placeholder="Max depth"
                  aria-label="Max depth filter"
                  disabled={!isTauri || !crawl}
                />
                <select
                  value={indexFilter}
                  onChange={(event) => {
                    setIndexFilter(event.target.value);
                    setPage(1);
                    if (crawl) void loadPage(crawl.id, 1, activeIssue);
                  }}
                  aria-label="Indexability filter"
                  disabled={!isTauri || !crawl}
                >
                  <option value="">Any indexability</option>
                  <option value="indexable">Indexable</option>
                  <option value="noindex">Noindex</option>
                  <option value="blocked">Blocked</option>
                </select>
                <input
                  type="text"
                  value={viewName}
                  onChange={(event) => setViewName(event.target.value)}
                  placeholder="View name"
                  aria-label="Saved view name"
                  disabled={!isTauri || !crawl}
                />
                <button className="secondary" type="button" onClick={saveView}>
                  Save view
                </button>
                <select
                  value={viewSelected}
                  onChange={(event) => {
                    setViewSelected(event.target.value);
                    void applyView(event.target.value);
                  }}
                  aria-label="Saved views"
                  disabled={!isTauri || !crawl}
                >
                  <option value="">Saved views…</option>
                  {views.map((view) => (
                    <option key={view.name} value={view.name}>
                      {view.name}
                    </option>
                  ))}
                </select>
                <button
                  className="secondary"
                  type="button"
                  onClick={() => removeView(viewSelected)}
                  disabled={!viewSelected}
                >
                  Delete view
                </button>
                <button
                  className="secondary"
                  type="button"
                  onClick={clearFilters}
                  disabled={activeFilterCount === 0}
                >
                  Clear filters ({activeFilterCount})
                </button>
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
                <select
                  value={bulkView}
                  onChange={(event) => setBulkView(event.target.value)}
                  aria-label="Export view"
                  disabled={!isTauri || !crawl}
                >
                  <option value="urls">URLs</option>
                  <option value="issues">Issues</option>
                  <option value="links">Links</option>
                  <option value="headers">Response headers</option>
                  <option value="images">Images</option>
                  <option value="accessibility">Accessibility</option>
                  <option value="structured">Structured data</option>
                  <option value="vitals">Web Vitals</option>
                  <option value="console">Console errors</option>
                  <option value="text">Page text</option>
                  <option value="markdown">Page Markdown</option>
                  <option value="extractions">Custom extraction</option>
                  <option value="custom-js">Custom JS results</option>
                  <option value="sitemap">Sitemap XML</option>
                  <option value="screenshots">Screenshots (folder)</option>
                  <option value="sitemap-bundle">Sitemap bundle (gzip)</option>
                </select>
                <button
                  className="secondary"
                  type="button"
                  onClick={async () => {
                    if (!crawl) return;
                    try {
                      const { invoke } = await import('@tauri-apps/api/core');
                      const path = await invoke<string>('export_view', {
                        crawlId: crawl.id,
                        view: bulkView,
                        issue: null,
                        columns: [],
                        format: bulkView === 'sitemap' ? 'xml' : 'csv',
                        pattern: exportPattern.trim() || null,
                        skipEmpty,
                      });
                      setExportPath(path);
                    } catch (cause) {
                      setError(cause instanceof Error ? cause.message : String(cause));
                    }
                  }}
                  disabled={!isTauri || !crawl}
                >
                  Export data
                </button>
                <button
                  className="secondary"
                  type="button"
                  onClick={exportCsv}
                  disabled={!isTauri || !crawl || total === 0}
                >
                  Export CSV
                </button>
                <select
                  value={reportKind}
                  onChange={(event) => setReportKind(event.target.value)}
                  aria-label="Report type"
                  disabled={!isTauri || !crawl}
                >
                  <option value="summary">Crawl summary</option>
                  <option value="issues">Issue breakdown</option>
                  <option value="status">Status codes</option>
                  <option value="performance">Performance</option>
                  <option value="redirects">Redirects</option>
                  <option value="broken-links">Broken links</option>
                </select>
                <input
                  type="number"
                  min={1}
                  max={500}
                  value={reportTop}
                  onChange={(event) => setReportTop(event.target.value)}
                  placeholder="Top N"
                  aria-label="Report row limit"
                  disabled={!isTauri || !crawl}
                />
                <input
                  type="number"
                  min={100}
                  max={599}
                  value={reportStatus}
                  onChange={(event) => setReportStatus(event.target.value)}
                  placeholder="Status"
                  aria-label="Report status filter"
                  disabled={!isTauri || !crawl}
                />
                <input
                  type="text"
                  value={reportTitle}
                  onChange={(event) => setReportTitle(event.target.value)}
                  placeholder="Report title"
                  aria-label="Report title"
                  disabled={!isTauri || !crawl}
                />
                <button
                  className="secondary"
                  type="button"
                  onClick={async () => {
                    if (!crawl) return;
                    try {
                      const { invoke } = await import('@tauri-apps/api/core');
                      const path = await invoke<string>('export_html_report', {
                        crawlId: crawl.id,
                        pattern: exportPattern.trim() || null,
                        report: reportKind,
                        top: reportTop.trim() ? Number(reportTop) : null,
                        status: reportStatus.trim() ? Number(reportStatus) : null,
                        title: reportTitle.trim() || null,
                      });
                      setExportPath(path);
                    } catch (cause) {
                      setError(cause instanceof Error ? cause.message : String(cause));
                    }
                  }}
                  disabled={!isTauri || !crawl}
                >
                  HTML report
                </button>
                <button
                  className="secondary"
                  type="button"
                  onClick={async () => {
                    if (!crawl) return;
                    try {
                      const { invoke } = await import('@tauri-apps/api/core');
                      const path = await invoke<string>('export_pdf_report', {
                        crawlId: crawl.id,
                        pattern: exportPattern.trim() || null,
                        report: reportKind,
                        top: reportTop.trim() ? Number(reportTop) : null,
                        status: reportStatus.trim() ? Number(reportStatus) : null,
                        title: reportTitle.trim() || null,
                      });
                      setExportPath(path);
                    } catch (cause) {
                      setError(cause instanceof Error ? cause.message : String(cause));
                    }
                  }}
                  disabled={!isTauri || !crawl}
                >
                  PDF report
                </button>
                <button
                  className="secondary"
                  type="button"
                  onClick={exportIssues}
                  disabled={!isTauri || !crawl || issueTotal === 0}
                >
                  Issues CSV
                </button>
                <button
                  className="secondary"
                  type="button"
                  onClick={async () => {
                    if (!crawl) return;
                    try {
                      const { invoke } = await import('@tauri-apps/api/core');
                      const path = await invoke<string>('export_view', {
                        crawlId: crawl.id,
                        pattern: exportPattern.trim() || null,
                        skipEmpty,
                        view: 'links',
                        issue: null,
                        columns: [],
                      });
                      setExportPath(path);
                    } catch (cause) {
                      setError(cause instanceof Error ? cause.message : String(cause));
                    }
                  }}
                  disabled={!isTauri || !crawl}
                >
                  Links CSV
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
                      <tr key={row.id} onClick={() => void openDetail(row)} style={{ cursor: 'pointer' }}>
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
            M4 scope: URL/sitemap/list seeds, regex scope, auth, robots.txt, redirects, Chromium
            rendering with screenshots and console capture, structured data, axe accessibility, Web Vitals, 24 audit rules and
            pause/resume. Next: bundled Node + Lighthouse for offline audits.
          </p>
        </main>
      </div>
    </div>
  );
}

































