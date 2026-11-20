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
  storeHtml?: boolean;
  customJs?: string[];
  webhookUrl?: string | null;
  crawlDelayMs?: number;
  maxRps?: number;
  proxyUrl?: string | null;
  cookie?: string | null;
}

interface UrlDetail {
  url: Record<string, unknown>;
  extractions: { name: string; value: string }[];
  customJs: { index: number; value: string }[];
  linkCounts: { inbound: number; outbound: number };
  inboundLinks: { url: string; anchor: string }[];
  outboundLinks: { url: string; anchor: string }[];
  images: { src: string; alt: string | null; width: number | null; height: number | null }[];
  screenshot: string | null;
}

interface AppInfo {
  version: string;
  dataDir: string;
  node: string | null;
  lighthouse: string | null;
  browser: string | null;
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
  duplicate_content: 'Duplicate content',
  duplicate_h1: 'Duplicate H1',
  nofollow_internal_link: 'Nofollow internal link',
  mixed_content: 'Mixed content',
  misspelling: 'Possible misspelling',
  title_too_short: 'Title too short',
  meta_description_too_long: 'Meta description too long',
  meta_description_too_short: 'Meta description too short',
  url_too_long: 'URL too long',
  image_missing_alt: 'Image missing alt text',
  image_missing_dimensions: 'Image missing dimensions',
  noindex_page: 'Noindex page',
  non_https: 'Non-HTTPS URL',
  blocked_by_robots: 'Blocked by robots.txt',
  long_redirect_chain: 'Long redirect chain',
  canonical_to_non_200: 'Canonical to non-200',
  slow_response: 'Slow response',
  large_page: 'Large page',
  client_error: '4xx client error',
  server_error: '5xx server error',
  redirected_url: 'Redirected URL',
  orphan_url: 'Orphan URL',
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

const ISSUE_DESCRIPTIONS: Record<string, string> = {
  missing_title: 'The page has no <title>.',
  title_too_long: 'Title exceeds the recommended pixel width.',
  title_too_short: 'Title is shorter than the recommended length.',
  missing_meta_description: 'No meta description is present.',
  meta_description_too_long: 'Meta description exceeds the recommended length.',
  meta_description_too_short: 'Meta description is shorter than the recommended length.',
  url_too_long: 'URL exceeds the recommended length.',
  image_missing_alt: 'One or more images have no alt text.',
  image_missing_dimensions: 'One or more images have no width/height.',
  noindex_page: 'The page is blocked from indexing by meta robots.',
  non_https: 'The URL uses HTTP rather than HTTPS.',
  blocked_by_robots: 'robots.txt blocked this URL from being crawled.',
  long_redirect_chain: 'The redirect chain has more than two hops.',
  canonical_to_non_200: 'The canonical target is missing, uncrawled or not 2xx.',
  slow_response: 'Response took longer than one second.',
  large_page: 'The HTML page is larger than 1 MB.',
  client_error: 'The URL returned a 4xx status.',
  server_error: 'The URL returned a 5xx status.',
  redirected_url: 'The URL redirects with a 3xx status.',
  orphan_url: 'No internal page links to this URL.',
  multiple_h1: 'More than one H1 heading was found.',
  missing_h1: 'No H1 heading was found.',
  thin_content: 'The page has very little body text.',
  broken_internal_link: 'An internal link points to a 4xx/5xx response.',
  canonical_mismatch: 'The canonical URL does not match the crawled URL.',
  missing_canonical: 'No canonical link element was found.',
  duplicate_title: 'Another page uses the same title.',
  duplicate_meta_description: 'Another page uses the same meta description.',
  duplicate_content: 'Another page has near-identical body content.',
  duplicate_h1: 'Another page uses the same H1 heading.',
  nofollow_internal_link: 'An internal link is marked nofollow.',
  mixed_content: 'An HTTPS page loads an HTTP image.',
  misspelling: 'Stored page text contains a common misspelling.',
  hreflang_missing_return: 'The hreflang target does not link back.',
  hreflang_invalid_target: 'An hreflang target could not be reached.',
  pagination_broken: 'A pagination link is broken.',
  jsonld_malformed: 'The JSON-LD block could not be parsed.',
  jsonld_missing_required: 'Structured data is missing required properties.',
  jsonld_missing: 'No JSON-LD structured data was found.',
  accessibility_violations: 'axe reported accessibility violations.',
  poor_lcp: 'Largest Contentful Paint is above the good threshold.',
  layout_shift: 'Cumulative Layout Shift is above the good threshold.',
  slow_ttfb: 'Time to First Byte is above the good threshold.',
  long_blocking_time: 'Total Blocking Time is above the good threshold.',
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
  const [patternTestUrl, setPatternTestUrl] = useState('');
  const [authUser, setAuthUser] = useState('');
  const [authPassword, setAuthPassword] = useState('');
  const [customHeaders, setCustomHeaders] = useState('');
  const [userAgent, setUserAgent] = useState(USER_AGENT);
  const [acceptLanguage, setAcceptLanguage] = useState('');
  const [render, setRender] = useState(false);
  const [browserPath, setBrowserPath] = useState('');
  const [renderWaitMs, setRenderWaitMs] = useState(500);
  const [renderWorkers, setRenderWorkers] = useState(2);
  const [screenshots, setScreenshots] = useState(false);
  const [renderBlockAssets, setRenderBlockAssets] = useState(false);
  const [accessibility, setAccessibility] = useState(false);
  const [webVitals, setWebVitals] = useState(false);
  const [storeText, setStoreText] = useState(false);
  const [storeHtml, setStoreHtml] = useState(false);
  const [customJs, setCustomJs] = useState('');
  const [webhookUrl, setWebhookUrl] = useState('');
  const [crawlDelayMs, setCrawlDelayMs] = useState(0);
  const [maxRps, setMaxRps] = useState(0);
  const [proxyUrl, setProxyUrl] = useState('');
  const [cookie, setCookie] = useState('');
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
  const [appInfo, setAppInfo] = useState<AppInfo | null>(null);
  const [history, setHistory] = useState<CrawlSummary[]>([]);
  const [compareLeft, setCompareLeft] = useState('');
  const [compareRight, setCompareRight] = useState('');
  const [compareRows, setCompareRows] = useState<
    { rule: string; left: number; right: number }[]
  >([]);
  const [compareError, setCompareError] = useState<string | null>(null);
  const [detail, setDetail] = useState<UrlDetail | null>(null);
  const [detailError, setDetailError] = useState<string | null>(null);
  const [sourceHtml, setSourceHtml] = useState<string | null>(null);
  const [sourceError, setSourceError] = useState<string | null>(null);
  const [sourceLoading, setSourceLoading] = useState(false);
  const [sourceKind, setSourceKind] = useState<'raw' | 'rendered'>('raw');
  const [sourceQuery, setSourceQuery] = useState('');
  const [sourceLimit, setSourceLimit] = useState(200_000);
  const [diffSummary, setDiffSummary] = useState<string | null>(null);
  const [copied, setCopied] = useState<string | null>(null);
  const [linkFilter, setLinkFilter] = useState('');
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
  const [suppressed, setSuppressed] = useState<{ rule: string; reason: string | null }[]>([]);
  const [overrides, setOverrides] = useState<Record<string, string>>({});
  const [auditLog, setAuditLog] = useState<
    {
      rule: string;
      action: string;
      reason: string | null;
      createdAt: number;
      actor: string | null;
    }[]
  >([]);
  const [issueDetails, setIssueDetails] = useState<
    { id: number; url: string; detail: string | null }[]
  >([]);
  const [statusCounts, setStatusCounts] = useState<{ status: number | null; count: number }[]>([]);
  const [responseStats, setResponseStats] = useState<{ averageMs: number; p95Ms: number } | null>(
    null,
  );
  const [indexStats, setIndexStats] = useState<{
    indexable: number;
    noindex: number;
    blocked: number;
  } | null>(null);
  const [depthCounts, setDepthCounts] = useState<{ depth: number; count: number }[]>([]);
  const [segments, setSegments] = useState<{ segment: string; count: number }[]>([]);
  const [timeline, setTimeline] = useState<{ second: number; count: number }[]>([]);
  const [duplicateKind, setDuplicateKind] = useState<'title' | 'meta_description' | 'h1'>('title');
  const [duplicateGroups, setDuplicateGroups] = useState<
    { value: string; count: number; urls: { id: number; url: string }[] }[]
  >([]);
  const [contentStats, setContentStats] = useState<{
    html: number;
    xml: number;
    image: number;
    other: number;
  } | null>(null);
  const [responseBuckets, setResponseBuckets] = useState<{ label: string; count: number }[]>([]);
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
            sortKey: string | null;
            sortDesc: boolean;
            pageSize: number | null;
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
  const [sortKey, setSortKey] = useState<string | null>(() => {
    if (typeof window === 'undefined') return null;
    return window.localStorage.getItem('coqui.sortKey');
  });
  const [sortDesc, setSortDesc] = useState(() => {
    if (typeof window === 'undefined') return false;
    return window.localStorage.getItem('coqui.sortDesc') === '1';
  });
  const [columnsOpen, setColumnsOpen] = useState(false);
  const [columns, setColumns] = useState<Record<ColumnKey, boolean>>(() => {
    const fallback = { ...DEFAULT_COLUMNS };
    if (typeof window === 'undefined') return fallback;
    try {
      const raw = window.localStorage.getItem('coqui.columns');
      if (!raw) return fallback;
      const parsed = JSON.parse(raw) as Partial<Record<ColumnKey, boolean>>;
      return { ...fallback, ...parsed };
    } catch {
      return fallback;
    }
  });
  const [pageSize, setPageSize] = useState(() => {
    if (typeof window === 'undefined') return PAGE_SIZE;
    const stored = Number(window.localStorage.getItem('coqui.pageSize'));
    return [25, 50, 100, 200].includes(stored) ? stored : PAGE_SIZE;
  });

  useEffect(() => {
    setIsTauri(typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window);
  }, []);

  useEffect(() => {
    if (!isTauri) return;
    void (async () => {
      try {
        const { invoke } = await import('@tauri-apps/api/core');
        setAppInfo(await invoke<AppInfo>('app_info'));
      } catch {
        // info is optional
      }
    })();
  }, [isTauri]);

  const loadPage = useCallback(
    async (
      crawlId: string,
      pageNumber: number,
      issue: string | null,
      size = pageSize,
      order: { key: string | null; desc: boolean } = { key: sortKey, desc: sortDesc },
    ) => {
      const { invoke } = await import('@tauri-apps/api/core');
      const data = await invoke<UrlPage>('list_urls', {
        crawlId,
        page: pageNumber,
        pageSize: size,
        issue,
        query: queryInput.trim() || null,
        status: /^\d{3}$/.test(statusFilter) ? Number(statusFilter) : null,
        statusClass: /^[2-5]xx$/.test(statusFilter) ? Number(statusFilter[0]) : null,
        contentClass: contentFilter || null,
        maxDepth: maxDepthFilter.trim() ? Number(maxDepthFilter) : null,
        indexability: indexFilter || null,
        sort: order.key,
        descending: order.desc,
      });
      setRows(data.rows);
      setTotal(data.total);
    },
    [queryInput, statusFilter, pageSize, sortKey, sortDesc],
  );

  const loadIssues = useCallback(async (crawlId: string) => {
    const { invoke } = await import('@tauri-apps/api/core');
    const counts = await invoke<IssueCount[]>('get_issue_counts', { crawlId });
    setIssues(counts);
    const ignored = await invoke<{ rule: string; reason: string | null }[]>(
      'suppressed_rules',
      { crawlId },
    );
    setSuppressed(ignored);
    const rules = await invoke<{ rule: string; severity: string }[]>('overrides', { crawlId });
    setOverrides(Object.fromEntries(rules.map((row) => [row.rule, row.severity])));
    setAuditLog(
      await invoke<
        {
          rule: string;
          action: string;
          reason: string | null;
          createdAt: number;
          actor: string | null;
        }[]
      >('issue_audit_log', { crawlId }),
    );
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
            sortKey: string | null;
            sortDesc: boolean;
            pageSize: number | null;
          }[]
        >('list_views');
        setViews(rows);
      } catch {
        // views are optional
      }
    })();
  }, [isTauri]);

  const suppressIssueUrl = async (rule: string, url: string) => {
    if (!crawl) return;
    const reason = window.prompt('Reason for ignoring this URL for this rule?') ?? '';
    if (!reason.trim()) {
      window.alert('A reason is required to suppress an issue.');
      return;
    }
    const days = window.prompt('Expire after how many days? Leave blank for no expiry.') ?? '';
    const expiresAt =
      days.trim() && Number(days) > 0 ? Date.now() + Number(days) * 86_400_000 : null;
    const { invoke } = await import('@tauri-apps/api/core');
    await invoke('suppress_url', {
      crawlId: crawl.id,
      rule,
      url,
      reason: reason.trim(),
      expiresAt,
    });
    setIssueDetails((current) => current.filter((row) => row.url !== url));
    await loadIssues(crawl.id);
  };

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
            sortKey: string | null;
            sortDesc: boolean;
            pageSize: number | null;
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
        const stats = await invoke<{ averageMs: number; p95Ms: number }>('response_stats', {
          crawlId: crawl.id,
        });
        setResponseStats(stats);
        const index = await invoke<{ indexable: number; noindex: number; blocked: number }>(
          'indexability_stats',
          { crawlId: crawl.id },
        );
        setIndexStats(index);
        const depths = await invoke<{ depth: number; count: number }[]>('depth_counts', {
          crawlId: crawl.id,
        });
        setDepthCounts(depths);
        setSegments(await invoke<{ segment: string; count: number }[]>('segments', { crawlId: crawl.id }));
        setTimeline(
          await invoke<{ second: number; count: number }[]>('throughput_buckets', {
            crawlId: crawl.id,
          }),
        );
        const content = await invoke<{
          html: number;
          xml: number;
          image: number;
          other: number;
        }>('content_stats', { crawlId: crawl.id });
        setContentStats(content);
        const buckets = await invoke<{ label: string; count: number }[]>('response_buckets', {
          crawlId: crawl.id,
        });
        setResponseBuckets(buckets);
      } catch {
        // overview is best-effort
      }
    })();
  }, [isTauri, crawl?.id, phase]);

  useEffect(() => {
    if (!isTauri || !crawl || !activeIssue) {
      setIssueDetails([]);
      return;
    }
    void (async () => {
      try {
        const { invoke } = await import('@tauri-apps/api/core');
        const rows = await invoke<{ id: number; url: string; detail: string | null }[]>(
          'issues_for_rule',
          { crawlId: crawl.id, rule: activeIssue },
        );
        setIssueDetails(rows);
      } catch {
        setIssueDetails([]);
      }
    })();
  }, [isTauri, crawl?.id, activeIssue]);

  const ignoreIssue = async (rule: string) => {
    if (!crawl) return;
    const reason = window.prompt('Reason for ignoring this rule?') ?? '';
    if (!reason.trim()) {
      window.alert('A reason is required to suppress an issue.');
      return;
    }
    const days = window.prompt('Expire after how many days? Leave blank for no expiry.') ?? '';
    const expiresAt =
      days.trim() && Number(days) > 0 ? Date.now() + Number(days) * 86_400_000 : null;
    const { invoke } = await import('@tauri-apps/api/core');
    await invoke('suppress_rule', {
      crawlId: crawl.id,
      rule,
      reason: reason.trim(),
      expiresAt,
    });
    await loadIssues(crawl.id);
  };

  const restoreIssue = async (rule: string) => {
    if (!crawl) return;
    const { invoke } = await import('@tauri-apps/api/core');
    await invoke('restore_rule', { crawlId: crawl.id, rule });
    await loadIssues(crawl.id);
  };

  const setIssueSeverity = async (rule: string, severity: string) => {
    if (!crawl) return;
    const { invoke } = await import('@tauri-apps/api/core');
    if (severity) {
      await invoke('set_override', { crawlId: crawl.id, rule, severity });
    } else {
      await invoke('clear_override', { crawlId: crawl.id, rule });
    }
    await loadIssues(crawl.id);
  };

  const applyTab = (content: string, status: string) => {
    setContentFilter(content);
    setStatusFilter(status);
    setPage(1);
    if (crawl) void loadPage(crawl.id, 1, activeIssue);
  };

  useEffect(() => {
    if (typeof window === 'undefined') return;
    try {
      window.localStorage.setItem('coqui.columns', JSON.stringify(columns));
    } catch {
      // storage may be unavailable
    }
  }, [columns]);

  useEffect(() => {
    try {
      window.localStorage.setItem('coqui.pageSize', String(pageSize));
    } catch {
      // storage may be unavailable
    }
  }, [pageSize]);

  useEffect(() => {
    try {
      window.localStorage.setItem('coqui.sortKey', sortKey ?? '');
      window.localStorage.setItem('coqui.sortDesc', sortDesc ? '1' : '0');
    } catch {
      // storage may be unavailable
    }
  }, [sortKey, sortDesc]);

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

  const refreshHistory = async () => {
    try {
      const { invoke } = await import('@tauri-apps/api/core');
      setHistory(await invoke<CrawlSummary[]>('list_crawls'));
    } catch {
      // history is optional
    }
  };

  useEffect(() => {
    if (!isTauri) return;
    void refreshHistory();
  }, [isTauri, phase]);

  const removeCrawl = async (entry: CrawlSummary) => {
    if (!window.confirm(`Delete crawl “${seedLabel(entry)}”?`)) return;
    const { invoke } = await import('@tauri-apps/api/core');
    await invoke('delete_crawl', { crawlId: entry.id });
    if (crawl?.id === entry.id) {
      setCrawl(null);
      setPhase('idle');
      setRows([]);
      setTotal(0);
      setIssues([]);
      setSuppressed([]);
      setStatusCounts([]);
      setResponseStats(null);
      setIndexStats(null);
      setDepthCounts([]);
      setContentStats(null);
      setResponseBuckets([]);
      setPage(1);
    }
    await refreshHistory();
  };

  const openCrawl = async (summary: CrawlSummary) => {
    setCrawl(summary);
    setPhase(summary.status);
    setPage(1);
    setActiveIssue(null);
    await Promise.all([loadPage(summary.id, 1, null), loadIssues(summary.id)]);
  };

  const copyValue = async (label: string, value: string) => {
    try {
      await navigator.clipboard.writeText(value);
      setCopied(label);
      window.setTimeout(() => setCopied(null), 1500);
    } catch {
      setCopied(`copy failed: ${label}`);
    }
  };

  useEffect(() => {
    if (!isTauri) return;
    const params = new URLSearchParams(window.location.hash.replace(/^#/, ''));
    const crawlId = params.get('crawl');
    const urlId = params.get('url');
    if (!crawlId) return;
    void (async () => {
      try {
        const { invoke } = await import('@tauri-apps/api/core');
        const summary = await invoke<CrawlSummary>('get_crawl', { crawlId });
        await openCrawl(summary);
        if (urlId) {
          const data = await invoke<UrlDetail>('get_url_detail', {
            crawlId,
            urlId: Number(urlId),
          });
          setDetail(data);
        }
      } catch {
        // stale or invalid link
      }
    })();
  }, [isTauri]);

  const matchesLink = (link: { url: string; anchor: string }) => {
    const needle = linkFilter.trim().toLowerCase();
    if (!needle) return true;
    return `${link.url} ${link.anchor}`.toLowerCase().includes(needle);
  };

  const exportLinks = () => {
    if (!detail) return;
    const cell = (value: string) => `"${value.replace(/"/g, '""')}"`;
    const page = String(detail.url.url ?? '');
    const rows: string[][] = [['direction', 'url', 'anchor', 'page']];
    for (const link of detail.inboundLinks.filter(matchesLink)) {
      rows.push(['in', link.url, link.anchor, page]);
    }
    for (const link of detail.outboundLinks.filter(matchesLink)) {
      rows.push(['out', link.url, link.anchor, page]);
    }
    const csv = rows.map((row) => row.map(cell).join(',')).join('\n');
    const blob = new Blob([csv], { type: 'text/csv;charset=utf-8' });
    const url = URL.createObjectURL(blob);
    const anchor = document.createElement('a');
    anchor.href = url;
    anchor.download = `links-${String(detail.url.id)}.csv`;
    anchor.click();
    URL.revokeObjectURL(url);
  };

  const runCompare = async () => {
    if (!compareLeft || !compareRight) return;
    try {
      const { invoke } = await import('@tauri-apps/api/core');
      const [left, right] = await Promise.all([
        invoke<IssueCount[]>('get_issue_counts', { crawlId: compareLeft }),
        invoke<IssueCount[]>('get_issue_counts', { crawlId: compareRight }),
      ]);
      const merged = new Map<string, { left: number; right: number }>();
      for (const issue of left) {
        merged.set(issue.rule, { left: issue.count, right: 0 });
      }
      for (const issue of right) {
        const entry = merged.get(issue.rule) ?? { left: 0, right: 0 };
        entry.right = issue.count;
        merged.set(issue.rule, entry);
      }
      setCompareRows(
        [...merged.entries()]
          .map(([rule, value]) => ({ rule, ...value }))
          .sort(
            (a, b) =>
              Math.abs(b.left - b.right) - Math.abs(a.left - a.right) ||
              a.rule.localeCompare(b.rule),
          ),
      );
      setCompareError(null);
    } catch (cause) {
      setCompareError(cause instanceof Error ? cause.message : String(cause));
    }
  };

  const highlightSource = (text: string) => {
    const query = sourceQuery.trim();
    if (!query) return text;
    const pattern = query.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
    const parts = text.split(new RegExp(`(${pattern})`, 'gi'));
    return parts.map((part, index) =>
      index % 2 === 1 ? <mark key={index}>{part}</mark> : part,
    );
  };

  const compareSources = async () => {
    if (!crawl || !detail) return;
    try {
      const { invoke } = await import('@tauri-apps/api/core');
      const [raw, rendered] = await Promise.all([
        invoke<string | null>('page_html', { crawlId: crawl.id, urlId: detail.url.id }),
        invoke<string | null>('page_rendered_html', {
          crawlId: crawl.id,
          urlId: detail.url.id,
        }),
      ]);
      if (!raw || !rendered) {
        setDiffSummary(
          'Both raw and rendered HTML must be stored: enable Store raw HTML and crawl with rendering.',
        );
        return;
      }
      if (raw === rendered) {
        setDiffSummary(`Identical (${raw.length} bytes).`);
        return;
      }
      const limit = Math.min(raw.length, rendered.length);
      let index = 0;
      while (index < limit && raw[index] === rendered[index]) index += 1;
      setDiffSummary(
        `Different: raw ${raw.length} B, rendered ${rendered.length} B; first difference at byte ${index}.\nraw: ${raw.slice(
          Math.max(0, index - 40),
          index + 60,
        )}\nrendered: ${rendered.slice(Math.max(0, index - 40), index + 60)}`,
      );
    } catch (cause) {
      setDiffSummary(cause instanceof Error ? cause.message : String(cause));
    }
  };

  const loadSource = async (kind: 'raw' | 'rendered' = sourceKind) => {
    if (!crawl || !detail) return;
    setSourceLoading(true);
    setSourceError(null);
    setSourceKind(kind);
    try {
      const { invoke } = await import('@tauri-apps/api/core');
      const html = await invoke<string | null>(
        kind === 'raw' ? 'page_html' : 'page_rendered_html',
        { crawlId: crawl.id, urlId: detail.url.id },
      );
      if (html === null) {
        setSourceHtml(null);
        setSourceError(
          kind === 'raw'
            ? 'Raw HTML was not stored for this crawl. Enable "Store raw HTML" before crawling.'
            : 'Rendered DOM was not stored (crawl without render, or the render failed).',
        );
      } else {
        setSourceHtml(html);
        setSourceLimit(200_000);
      }
    } catch (cause) {
      setSourceError(cause instanceof Error ? cause.message : String(cause));
    } finally {
      setSourceLoading(false);
    }
  };

  const openDetailById = async (urlId: number) => {
    if (!crawl) return;
    try {
      const { invoke } = await import('@tauri-apps/api/core');
      const data = await invoke<UrlDetail>('get_url_detail', {
        crawlId: crawl.id,
        urlId,
      });
      setDetail(data);
      setSourceHtml(null);
      setSourceError(null);
      setSourceQuery('');
      setDiffSummary(null);
      setLinkFilter('');
      setDetailError(null);
    } catch (cause) {
      setDetailError(cause instanceof Error ? cause.message : String(cause));
    }
  };

  const openDetail = async (row: UrlRow) => {
    try {
      const { invoke } = await import('@tauri-apps/api/core');
      const data = await invoke<UrlDetail>('get_url_detail', {
        crawlId: row.crawlId,
        urlId: row.id,
      });
      setDetail(data);
      setLinkFilter('');
      setDetailError(null);
    } catch (cause) {
      setDetailError(cause instanceof Error ? cause.message : String(cause));
    }
  };

  async function startCrawl(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    for (const [label, patterns] of [
      ['Include', splitLines(includePatterns)],
      ['Exclude', splitLines(excludePatterns)],
    ] as const) {
      for (const pattern of patterns) {
        try {
          new RegExp(pattern);
        } catch (cause) {
          setError(
            `${label} pattern "${pattern}" is invalid: ${
              cause instanceof Error ? cause.message : String(cause)
            }`,
          );
          return;
        }
      }
    }
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
        userAgent: userAgent.trim() || USER_AGENT,
        sameHostOnly,
        respectRobots,
        includePatterns: splitLines(includePatterns),
        excludePatterns: splitLines(excludePatterns),
        authUser: authUser.trim() || null,
        authPassword: authPassword || null,
        customHeaders: [
          ...splitLines(customHeaders),
          ...(acceptLanguage.trim() ? [`Accept-Language: ${acceptLanguage.trim()}`] : []),
        ],
        render,
        browserPath: browserPath.trim() || null,
        renderWaitMs,
        renderWorkers,
        screenshots,
        renderBlockAssets,
        accessibility,
        webVitals,
        storeText,
        storeHtml,
        customJs: splitLines(customJs),
        webhookUrl: webhookUrl.trim() || null,
        crawlDelayMs: Math.max(0, Math.round(crawlDelayMs)),
        maxRps: Math.max(0, Math.round(maxRps)),
        proxyUrl: proxyUrl.trim() || null,
        cookie: cookie.trim() || null,
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

  const totalPages = Math.max(1, Math.ceil(total / pageSize));
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
    userAgent,
    acceptLanguage,
    proxyUrl,
    cookie,
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
    webhookUrl,
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
    if (typeof value.userAgent === 'string') setUserAgent(value.userAgent);
    if (typeof value.acceptLanguage === 'string') setAcceptLanguage(value.acceptLanguage);
    if (typeof value.proxyUrl === 'string') setProxyUrl(value.proxyUrl);
    if (typeof value.cookie === 'string') setCookie(value.cookie);
    if (typeof value.render === 'boolean') setRender(value.render);
    if (typeof value.browserPath === 'string') setBrowserPath(value.browserPath);
    if (typeof value.renderWaitMs === 'number') setRenderWaitMs(value.renderWaitMs);
    if (typeof value.renderWorkers === 'number') setRenderWorkers(value.renderWorkers);
    if (typeof value.screenshots === 'boolean') setScreenshots(value.screenshots);
    if (typeof value.renderBlockAssets === 'boolean') setRenderBlockAssets(value.renderBlockAssets);
    if (typeof value.accessibility === 'boolean') setAccessibility(value.accessibility);
    if (typeof value.webVitals === 'boolean') setWebVitals(value.webVitals);
    if (typeof value.storeText === 'boolean') setStoreText(value.storeText);
    if (typeof value.storeHtml === 'boolean') setStoreHtml(value.storeHtml);
    if (typeof value.customJs === 'string') setCustomJs(value.customJs);
    if (typeof value.webhookUrl === 'string') setWebhookUrl(value.webhookUrl);
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
    { label: 'Store raw HTML', help: 'Keep raw HTML for the source viewer.', value: storeHtml ? 'on' : 'off', changed: storeHtml },
    { label: 'Completion webhook', help: 'POST crawl stats to this URL when the crawl finishes.', value: webhookUrl.trim() || 'none', changed: webhookUrl.trim().length > 0 },
    { label: 'Browser path', help: 'Chrome/Edge executable override.', value: browserPath || 'auto', changed: browserPath.length > 0 },
    { label: 'Include patterns', help: 'One regex per line; only matching URLs are crawled.', value: includePatterns ? 'set' : 'none', changed: includePatterns.length > 0 },
    { label: 'Exclude patterns', help: 'One regex per line; matching URLs are skipped.', value: excludePatterns ? 'set' : 'none', changed: excludePatterns.length > 0 },
    { label: 'Basic auth user', help: 'HTTP basic authentication username.', value: authUser || 'none', changed: authUser.length > 0 },
    { label: 'Cookie header', help: 'Sent as the Cookie header for logged-in crawling.', value: cookie ? 'set' : 'none', changed: cookie.length > 0 },
    { label: 'Custom headers', help: 'Header: value lines sent with every request.', value: customHeaders ? 'set' : 'none', changed: customHeaders.length > 0 },
    { label: 'User-Agent', help: 'HTTP User-Agent for every request.', value: userAgent, changed: userAgent !== USER_AGENT },
    { label: 'Accept-Language', help: 'Optional Accept-Language header.', value: acceptLanguage || 'none', changed: acceptLanguage.length > 0 },
    { label: 'Proxy', help: 'HTTP/SOCKS proxy for crawl requests.', value: proxyUrl || 'none', changed: proxyUrl.length > 0 },
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

  const throughput = useMemo(() => {
    if (!crawl?.startedAt || !crawl.finishedAt) return null;
    const seconds = (crawl.finishedAt - crawl.startedAt) / 1000;
    if (seconds <= 0) return null;
    return crawl.totalUrls / seconds;
  }, [crawl]);

  useEffect(() => {
    if (!crawl) {
      setDuplicateGroups([]);
      return;
    }
    let cancelled = false;
    void (async () => {
      const { invoke } = await import('@tauri-apps/api/core');
      try {
        const rows = await invoke<
          { value: string; count: number; urls: { id: number; url: string }[] }[]
        >('duplicate_groups', { crawlId: crawl.id, field: duplicateKind });
        if (!cancelled) setDuplicateGroups(rows);
      } catch {
        if (!cancelled) setDuplicateGroups([]);
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [crawl, duplicateKind]);

  const patternTest = useMemo(() => {
    const url = patternTestUrl.trim();
    if (!url) return null;
    const check = (text: string) =>
      splitLines(text).map((pattern) => {
        try {
          return { pattern, valid: true, match: new RegExp(pattern).test(url) };
        } catch {
          return { pattern, valid: false, match: false };
        }
      });
    return { include: check(includePatterns), exclude: check(excludePatterns) };
  }, [patternTestUrl, includePatterns, excludePatterns]);

  const depthBuckets = useMemo(() => {
    const buckets = [0, 0, 0, 0];
    for (const row of depthCounts) {
      const index = Math.min(Math.max(row.depth, 0), 3);
      buckets[index] += row.count;
    }
    return buckets;
  }, [depthCounts]);

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
              <div className="field">
                <label htmlFor="patternTest">Test a URL against the patterns</label>
                <input
                  id="patternTest"
                  type="url"
                  value={patternTestUrl}
                  onChange={(event) => setPatternTestUrl(event.target.value)}
                  placeholder="https://example.com/blog/post"
                  disabled={isRunning}
                />
                {patternTest && (
                  <div className="pattern-test">
                    {(['include', 'exclude'] as const).map((kind) => (
                      <div key={kind}>
                        <strong>{kind === 'include' ? 'Include' : 'Exclude'}:</strong>{' '}
                        {patternTest[kind].length === 0
                          ? 'no patterns'
                          : patternTest[kind]
                              .map(
                                (entry) =>
                                  `${entry.pattern} ${entry.valid ? (entry.match ? '✓ matches' : '✗ no match') : 'invalid'}`,
                              )
                              .join(' · ')}
                      </div>
                    ))}
                  </div>
                )}
              </div>
              <div className="grid-2">
                <div className="field">
                  <label htmlFor="cookie">Cookie header</label>
                  <input
                    id="cookie"
                    type="text"
                    value={cookie}
                    onChange={(event) => setCookie(event.target.value)}
                    placeholder="session=abc; other=1"
                    disabled={isRunning}
                    autoComplete="off"
                  />
                </div>
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
                  <div className="field checkbox-field">
                    <label htmlFor="storeHtml">
                      <input
                        id="storeHtml"
                        type="checkbox"
                        checked={storeHtml}
                        onChange={(event) => setStoreHtml(event.target.checked)}
                        disabled={isRunning}
                      />
                      Store raw HTML
                    </label>
                  </div>
                  <div className="field">
                    <label htmlFor="crawlDelayMs">Delay between requests (ms)</label>
                    <input
                      id="crawlDelayMs"
                      type="number"
                      min={0}
                      step={50}
                      value={crawlDelayMs}
                      onChange={(event) => setCrawlDelayMs(Number(event.target.value) || 0)}
                      disabled={isRunning}
                    />
                  </div>
                  <div className="field">
                    <label htmlFor="proxyUrl">Proxy URL (http/https/socks5)</label>
                    <input
                      id="proxyUrl"
                      type="url"
                      value={proxyUrl}
                      onChange={(event) => setProxyUrl(event.target.value)}
                      placeholder="http://127.0.0.1:8080"
                      disabled={isRunning}
                    />
                  </div>
                  <div className="field">
                    <label htmlFor="maxRps">Max requests/second per host (0 = unlimited)</label>
                    <input
                      id="maxRps"
                      type="number"
                      min={0}
                      step={1}
                      value={maxRps}
                      onChange={(event) => setMaxRps(Number(event.target.value) || 0)}
                      disabled={isRunning}
                    />
                  </div>
                  <div className="field">
                    <label htmlFor="webhookUrl">Completion webhook URL</label>
                    <input
                      id="webhookUrl"
                      type="url"
                      value={webhookUrl}
                      onChange={(event) => setWebhookUrl(event.target.value)}
                      placeholder="https://example.com/hooks/coqui"
                      disabled={isRunning}
                    />
                  </div>
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

          {history.length > 0 && (
            <div className="history">
              <h3>Recent crawls</h3>
              <div className="history-list">
                {history.slice(0, 8).map((entry) => (
                  <div
                    key={entry.id}
                    className={`history-item${crawl?.id === entry.id ? ' active' : ''}`}
                  >
                    <button
                      type="button"
                      className="history-open"
                      onClick={() => void openCrawl(entry)}
                    >
                      <span>{seedLabel(entry)}</span>
                      <small>
                        {entry.totalUrls} URLs · {entry.status}
                      </small>
                    </button>
                    <button
                      type="button"
                      className="history-delete"
                      title="Delete crawl"
                      onClick={() => void removeCrawl(entry)}
                    >
                      ×
                    </button>
                  </div>
                ))}
              </div>
            </div>
          )}

          {history.length > 1 && (
            <div className="history compare-panel">
              <h3>Compare crawls</h3>
              <div className="compare-controls">
                <select
                  value={compareLeft}
                  onChange={(event) => setCompareLeft(event.target.value)}
                  aria-label="Left crawl"
                >
                  <option value="">Left crawl…</option>
                  {history.map((entry) => (
                    <option key={entry.id} value={entry.id}>
                      {seedLabel(entry)}
                    </option>
                  ))}
                </select>
                <select
                  value={compareRight}
                  onChange={(event) => setCompareRight(event.target.value)}
                  aria-label="Right crawl"
                >
                  <option value="">Right crawl…</option>
                  {history.map((entry) => (
                    <option key={entry.id} value={entry.id}>
                      {seedLabel(entry)}
                    </option>
                  ))}
                </select>
                <button
                  className="secondary"
                  type="button"
                  onClick={() => void runCompare()}
                  disabled={!compareLeft || !compareRight}
                >
                  Compare
                </button>
              </div>
              {compareError && <p className="settings-errors">{compareError}</p>}
              {compareRows.length > 0 && (
                <table className="compare-table">
                  <thead>
                    <tr>
                      <th>Rule</th>
                      <th>Left</th>
                      <th>Right</th>
                      <th>Δ</th>
                    </tr>
                  </thead>
                  <tbody>
                    {compareRows.map((row) => {
                      const delta = row.right - row.left;
                      return (
                        <tr key={row.rule}>
                          <td>{ISSUE_LABELS[row.rule] ?? row.rule}</td>
                          <td>{row.left}</td>
                          <td>{row.right}</td>
                          <td>{delta > 0 ? `+${delta}` : delta}</td>
                        </tr>
                      );
                    })}
                  </tbody>
                </table>
              )}
            </div>
          )}

          <p className="footer-note" style={{ marginTop: 18, marginBottom: 0 }}>
            Coqui {appInfo?.version ?? ''} · Node {appInfo?.node ? 'bundled' : 'missing'} ·
            Lighthouse {appInfo?.lighthouse ? 'bundled' : 'missing'}
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
                <div className="detail-actions">
                  <button
                    className="secondary"
                    type="button"
                    onClick={() => void copyValue('URL', String(detail.url.url ?? ''))}
                  >
                    {copied === 'URL' ? 'Copied URL' : 'Copy URL'}
                  </button>
                  <button
                    className="secondary"
                    type="button"
                    onClick={() => void copyValue('Detail', JSON.stringify(detail, null, 2))}
                  >
                    {copied === 'Detail' ? 'Copied JSON' : 'Copy JSON'}
                  </button>
                  <button
                    className="secondary"
                    type="button"
                    disabled={!detail.url.title}
                    onClick={() => void copyValue('title', String(detail.url.title ?? ''))}
                  >
                    {copied === 'title' ? 'Copied title' : 'Copy title'}
                  </button>
                  <button
                    className="secondary"
                    type="button"
                    onClick={() =>
                      void copyValue(
                        'link',
                        `${window.location.origin}${window.location.pathname}#crawl=${String(
                          detail.url.crawlId,
                        )}&url=${String(detail.url.id)}`,
                      )
                    }
                  >
                    {copied === 'link' ? 'Copied link' : 'Copy link'}
                  </button>
                  <button
                    className="secondary"
                    type="button"
                    disabled={
                      detail.inboundLinks.length + detail.outboundLinks.length === 0
                    }
                    onClick={exportLinks}
                  >
                    Export links CSV
                  </button>
                </div>
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
                <h4>Source</h4>
                <button
                  type="button"
                  className="link-button"
                  onClick={() => void loadSource()}
                  disabled={sourceLoading}
                >
                    {sourceLoading ? 'Loading…' : 'View raw HTML'}
                </button>
                {sourceHtml && (
                  <button
                    type="button"
                    className="link-button"
                    style={{ marginLeft: 8 }}
                    onClick={() => void copyValue('Source', sourceHtml)}
                  >
                    Copy HTML
                  </button>
                )}
                {sourceError && <p className="error">{sourceError}</p>}
                {sourceHtml && (
                  <input
                    className="source-find"
                    type="search"
                    value={sourceQuery}
                    onChange={(event) => setSourceQuery(event.target.value)}
                    placeholder="Find in source"
                    style={{ marginTop: 8, width: '100%' }}
                  />
                )}
                {sourceHtml && (
                  <span className="muted" style={{ marginLeft: 8 }}>
                    {(sourceHtml.length / 1024).toFixed(0)} KB
                  </span>
                )}
                {sourceHtml && (
                  <pre className="source-view">
                    {highlightSource(sourceHtml.slice(0, sourceLimit))}
                  </pre>
                )}
                {sourceHtml && sourceHtml.length > sourceLimit && (
                  <button
                    type="button"
                    className="link-button"
                    style={{ marginTop: 8 }}
                    onClick={() => setSourceLimit(sourceLimit + 200_000)}
                  >
                    Load more ({Math.round(sourceLimit / 1024)} KB of{' '}
                    {Math.round(sourceHtml.length / 1024)} KB)
                  </button>
                )}
                {detail.screenshot && (
                  <img
                    src={detail.screenshot}
                    alt="Page screenshot"
                    style={{ maxWidth: '100%', borderRadius: 8, marginTop: 12 }}
                  />
                )}
                <input
                  type="search"
                  value={linkFilter}
                  onChange={(event) => setLinkFilter(event.target.value)}
                  placeholder="Filter links"
                  aria-label="Filter links"
                />
                {detail.inboundLinks.length > 0 && (
                  <>
                    <h4>Inbound links</h4>
                    <ul className="settings-reference">
                      {detail.inboundLinks.filter(matchesLink).map((link) => (
                        <li key={link.url}>
                          {link.url} — “{link.anchor}”
                        </li>
                      ))}
                    </ul>
                  </>
                )}
                {detail.outboundLinks.length > 0 && (
                  <>
                    <h4>Outbound links</h4>
                    <ul className="settings-reference">
                      {detail.outboundLinks.filter(matchesLink).map((link) => (
                        <li key={link.url}>
                          {link.url} — “{link.anchor}”
                        </li>
                      ))}
                    </ul>
                  </>
                )}
                {detail.images.length > 0 && (
                  <>
                    <h4>Images</h4>
                    <ul className="settings-reference">
                      {detail.images.map((image) => (
                        <li key={image.src}>
                          {image.src}
                          {image.alt ? ` — alt “${image.alt}”` : ' — no alt'}
                          {image.width && image.height
                            ? ` (${image.width}×${image.height})`
                            : ''}
                        </li>
                      ))}
                    </ul>
                  </>
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
                  <div key={`${issue.rule}-${issue.severity}`} className="issue-card-wrap">
                    <button
                      type="button"
                      className={`issue-card ${issue.severity} ${
                        activeIssue === issue.rule ? 'active' : ''
                      }`}
                      title={ISSUE_DESCRIPTIONS[issue.rule] ?? issue.rule}
                      onClick={() => filterByIssue(issue.rule)}
                    >
                      <span className="issue-count">{issue.count}</span>
                      <span className="issue-label">
                        {ISSUE_LABELS[issue.rule] ?? issue.rule}
                      </span>
                      <span className="issue-sev">{issue.severity}</span>
                    </button>
                    <button
                      type="button"
                      className="issue-ignore"
                      title="Ignore this rule"
                      onClick={(event) => {
                        event.stopPropagation();
                        void ignoreIssue(issue.rule);
                      }}
                    >
                      Ignore
                    </button>
                  </div>
                ))}
              </div>
              {suppressed.length > 0 && (
                <div className="suppressed-rules">
                  <span className="muted">Ignored:</span>
                  {suppressed.map((entry) => (
                    <button
                      key={entry.rule}
                      type="button"
                      className="suppressed-rule"
                      title={entry.reason ? `Restore: ${entry.reason}` : 'Restore this rule'}
                      onClick={() => void restoreIssue(entry.rule)}
                    >
                      {ISSUE_LABELS[entry.rule] ?? entry.rule} ×
                    </button>
                  ))}
                </div>
              )}
              {auditLog.length > 0 && (
                <div className="issue-details">
                  <h4>Recent issue actions</h4>
                  <ul className="settings-reference">
                    {auditLog.slice(0, 8).map((entry, index) => (
                      <li key={`${entry.createdAt}-${index}`}>
                        {new Date(entry.createdAt).toLocaleString()} — {entry.action}{' '}
                        {ISSUE_LABELS[entry.rule] ?? entry.rule}
                        {entry.reason ? ` (${entry.reason})` : ''}
                        {entry.actor ? ` — ${entry.actor}` : ''}
                      </li>
                    ))}
                  </ul>
                </div>
              )}
              {activeIssue && issueDetails.length > 0 && (
                <div className="issue-details">
                  <h4>
                    {ISSUE_LABELS[activeIssue] ?? activeIssue} ({issueDetails.length})
                    <select
                      value={overrides[activeIssue] ?? ''}
                      onChange={(event) => void setIssueSeverity(activeIssue, event.target.value)}
                      aria-label="Severity override"
                    >
                      <option value="">Default severity</option>
                      <option value="error">Error</option>
                      <option value="warning">Warning</option>
                      <option value="opportunity">Opportunity</option>
                    </select>
                  </h4>
                  <ul className="settings-reference">
                    {issueDetails.slice(0, 50).map((row) => (
                      <li key={row.url}>
                        <code>{row.url}</code>
                        {row.detail ? ` — ${row.detail}` : ''}
                        <button
                          type="button"
                          className="link-button"
                          style={{ marginLeft: 8 }}
                          onClick={() => void suppressIssueUrl(activeIssue, row.url)}
                        >
                          Ignore URL
                        </button>
                      </li>
                    ))}
                  </ul>
                </div>
              )}
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
            <div>
              <span>Throughput</span>
              <strong>{throughput ? `${throughput.toFixed(1)}/s` : '—'}</strong>
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
            <div>
              <span>Avg response</span>
              <strong>{responseStats ? `${responseStats.averageMs} ms` : '—'}</strong>
            </div>
            <div>
              <span>p95 response</span>
              <strong>{responseStats ? `${responseStats.p95Ms} ms` : '—'}</strong>
            </div>
            {indexStats &&
              (
                [
                  ['indexable', 'Indexable'],
                  ['noindex', 'Noindex'],
                  ['blocked', 'Blocked'],
                ] as const
              ).map(([key, label]) => (
                <button
                  key={key}
                  type="button"
                  className={`overview-card${indexFilter === key ? ' active' : ''}`}
                  onClick={() => {
                    const next = indexFilter === key ? '' : key;
                    setIndexFilter(next);
                    setPage(1);
                    if (crawl) void loadPage(crawl.id, 1, activeIssue);
                  }}
                >
                  <span>{label}</span>
                  <strong>{indexStats[key]}</strong>
                </button>
              ))}
            {(['0', '1', '2', '3+'] as const).map((label, index) => (
              <button
                key={`depth-${label}`}
                type="button"
                className={`overview-card${
                  maxDepthFilter === (label === '3+' ? '3' : label) ? ' active' : ''
                }`}
                title={`Show URLs at depth ≤ ${label === '3+' ? '3' : label}`}
                onClick={() => {
                  const next = label === '3+' ? '3' : label;
                  setMaxDepthFilter(maxDepthFilter === next ? '' : next);
                  setPage(1);
                  if (crawl) void loadPage(crawl.id, 1, activeIssue);
                }}
              >
                <span>Depth {label}</span>
                <strong>{depthBuckets[index] ?? 0}</strong>
              </button>
            ))}
            {contentStats &&
              (
                [
                  ['html', 'HTML'],
                  ['xml', 'XML'],
                  ['image', 'Images'],
                  ['other', 'Other'],
                ] as const
              ).map(([key, label]) => (
                <button
                  key={key}
                  type="button"
                  className={`overview-card${contentFilter === key ? ' active' : ''}`}
                  onClick={() => {
                    const next = contentFilter === key ? '' : key;
                    setContentFilter(next);
                    setPage(1);
                    if (crawl) void loadPage(crawl.id, 1, activeIssue);
                  }}
                >
                  <span>{label}</span>
                  <strong>{contentStats[key]}</strong>
                </button>
              ))}
            {responseBuckets.length > 0 && (
              <div className="overview-bars" style={{ flexBasis: '100%' }}>
                {responseBuckets.map((bucket) => {
                  const max = Math.max(1, ...responseBuckets.map((entry) => entry.count));
                  return (
                    <div className="bar-row" key={bucket.label}>
                      <span>{bucket.label}</span>
                      <div className="bar-track">
                        <div
                          className="bar-fill"
                          style={{ width: `${Math.round((bucket.count / max) * 100)}%` }}
                        />
                      </div>
                      <strong>{bucket.count}</strong>
                    </div>
                  );
                })}
              </div>
            )}
            {depthBuckets.some((value) => value > 0) && (
              <div className="overview-bars" style={{ flexBasis: '100%' }}>
                {depthBuckets.map((count, index) => {
                  const max = Math.max(1, ...depthBuckets);
                  return (
                    <div className="bar-row" key={`depth-bar-${index}`}>
                      <span>{index === 3 ? 'Depth 3+' : `Depth ${index}`}</span>
                      <div className="bar-track">
                        <div
                          className="bar-fill"
                          style={{ width: `${Math.round((count / max) * 100)}%` }}
                        />
                      </div>
                      <strong>{count}</strong>
                    </div>
                  );
                })}
              </div>
            )}
            {segments.length > 0 && (
              <div className="overview-bars" style={{ flexBasis: '100%' }}>
                <h3>Top segments</h3>
                {segments.slice(0, 8).map((row) => {
                  const max = Math.max(1, ...segments.map((entry) => entry.count));
                  return (
                    <div
                      className="bar-row"
                      key={`segment-${row.segment}`}
                      role="button"
                      tabIndex={0}
                      title={
                        row.segment === '(root)'
                          ? 'Clear URL filter'
                          : `Filter URLs to /${row.segment}`
                      }
                      style={{ cursor: 'pointer' }}
                      onClick={() =>
                        setQueryInput(row.segment === '(root)' ? '' : `/${row.segment}/`)
                      }
                      onKeyDown={(event) => {
                        if (event.key === 'Enter' || event.key === ' ') {
                          setQueryInput(row.segment === '(root)' ? '' : `/${row.segment}/`);
                        }
                      }}
                    >
                      <span>{row.segment}</span>
                      <div className="bar-track">
                        <div
                          className="bar-fill"
                          style={{ width: `${Math.round((row.count / max) * 100)}%` }}
                        />
                      </div>
                      <strong>{row.count}</strong>
                    </div>
                  );
                })}
              </div>
            )}
            <div className="overview-bars" style={{ flexBasis: '100%' }}>
              <h3>
                Duplicates{' '}
                <select
                  value={duplicateKind}
                  onChange={(event) =>
                    setDuplicateKind(event.target.value as 'title' | 'meta_description' | 'h1')
                  }
                  aria-label="Duplicate field"
                >
                  <option value="title">Title</option>
                  <option value="meta_description">Meta description</option>
                  <option value="h1">H1</option>
                </select>
              </h3>
              {duplicateGroups.length === 0 && <span className="muted">No duplicate groups.</span>}
              {duplicateGroups.slice(0, 5).map((group) => (
                <div key={group.value} className="duplicate-group">
                  <div>
                    <strong>{group.count}×</strong>{' '}
                    {group.value.length > 90 ? `${group.value.slice(0, 90)}…` : group.value}
                  </div>
                  <ul>
                    {group.urls.slice(0, 3).map((entry) => (
                      <li key={entry.id}>
                        <button
                          type="button"
                          className="link-button"
                          onClick={() => void openDetailById(entry.id)}
                        >
                          {entry.url}
                        </button>
                      </li>
                    ))}
                  </ul>
                </div>
              ))}
            </div>
            {timeline.length > 0 && (
              <div className="overview-bars" style={{ flexBasis: '100%' }}>
                <h3>Throughput timeline</h3>
                {timeline.slice(-30).map((row) => {
                  const max = Math.max(1, ...timeline.map((entry) => entry.count));
                  return (
                    <div className="bar-row" key={`timeline-${row.second}`}>
                      <span>+{row.second}s</span>
                      <div className="bar-track">
                        <div
                          className="bar-fill"
                          style={{ width: `${Math.round((row.count / max) * 100)}%` }}
                        />
                      </div>
                      <strong>{row.count}</strong>
                    </div>
                  );
                })}
              </div>
            )}
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
                  <option value="anchors">Anchor texts</option>
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
                  <option value="overview">Crawl overview</option>
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
                          onClick={() => {
                            if (
                              !['url', 'status', 'depth', 'words', 'time', 'size'].includes(
                                column.key,
                              ) ||
                              !crawl
                            ) {
                              return;
                            }
                            const nextDesc = sortKey === column.key ? !sortDesc : false;
                            setSortKey(column.key);
                            setSortDesc(nextDesc);
                            setPage(1);
                            void loadPage(crawl.id, 1, activeIssue, pageSize, {
                              key: column.key,
                              desc: nextDesc,
                            });
                          }}
                          style={{ cursor: 'pointer' }}
                        >
                          {column.label}
                          {sortKey === column.key ? (sortDesc ? ' ↓' : ' ↑') : ''}
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
                <label className="page-size">
                  Rows
                  <select
                    value={pageSize}
                    aria-label="Rows per page"
                    onChange={(event) => {
                      const next = Number(event.target.value);
                      setPageSize(next);
                      setPage(1);
                      if (crawl) void loadPage(crawl.id, 1, activeIssue, next);
                    }}
                  >
                    {[25, 50, 100, 200].map((size) => (
                      <option key={size} value={size}>
                        {size}
                      </option>
                    ))}
                  </select>
                </label>
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
            Data: {appInfo?.dataDir ?? '—'} · Browser: {appInfo?.browser ?? 'not found'}
          </p>
        </main>
      </div>
    </div>
  );
}






















































