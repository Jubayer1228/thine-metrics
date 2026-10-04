#!/usr/bin/env node
/**
 * Crawl Datadog Observability (18) docs with Playwright.
 * Extracts titles, H2/H3 outlines, and key capability bullets.
 */
const { chromium } = require('/tmp/node_modules/playwright');
const fs = require('fs');

const PAGES = [
  // Infrastructure (8 from screenshot)
  { id: 'infrastructure', url: 'https://docs.datadoghq.com/infrastructure/' },
  { id: 'metrics', url: 'https://docs.datadoghq.com/metrics/' },
  { id: 'containers', url: 'https://docs.datadoghq.com/containers/' },
  { id: 'serverless', url: 'https://docs.datadoghq.com/serverless/' },
  { id: 'network', url: 'https://docs.datadoghq.com/network_monitoring/' },
  { id: 'cloud_cost', url: 'https://docs.datadoghq.com/cloud_cost_management/' },
  { id: 'cloudcraft', url: 'https://docs.datadoghq.com/datadog_cloudcraft/' },
  { id: 'storage', url: 'https://docs.datadoghq.com/infrastructure/storage_management/' },
  // Applications (3)
  { id: 'apm', url: 'https://docs.datadoghq.com/tracing/' },
  { id: 'usm', url: 'https://docs.datadoghq.com/universal_service_monitoring/' },
  { id: 'profiler', url: 'https://docs.datadoghq.com/profiler/' },
  // Data (3)
  { id: 'dbm', url: 'https://docs.datadoghq.com/database_monitoring/' },
  { id: 'dsm', url: 'https://docs.datadoghq.com/data_streams_monitoring/' },
  { id: 'data_obs', url: 'https://docs.datadoghq.com/data_observability_overview/' },
  // Logs (4)
  { id: 'logs', url: 'https://docs.datadoghq.com/logs/' },
  { id: 'sds', url: 'https://docs.datadoghq.com/sensitive_data_scanner/' },
  { id: 'obs_pipelines', url: 'https://docs.datadoghq.com/observability_pipelines/' },
  { id: 'error_tracking', url: 'https://docs.datadoghq.com/error_tracking/' },
];

async function scrape(page, item) {
  const out = { id: item.id, url: item.url, ok: false };
  try {
    const resp = await page.goto(item.url, { waitUntil: 'domcontentloaded', timeout: 45000 });
    out.status = resp ? resp.status() : 0;
    await page.waitForTimeout(800);
    out.title = await page.title();
    out.h1 = await page.locator('h1').first().textContent().catch(() => null);
    out.headings = await page.locator('h2, h3').allTextContents().then(xs =>
      xs.map(s => s.trim()).filter(Boolean).slice(0, 40)
    );
    // capability-ish bullets / paragraphs under main
    out.bullets = await page.locator('main li, article li, .doc-content li').allTextContents().then(xs =>
      xs.map(s => s.trim().replace(/\s+/g, ' ')).filter(s => s.length > 20 && s.length < 220).slice(0, 25)
    );
    out.summary = await page.locator('main p, article p').first().textContent().catch(() => null);
    if (out.summary) out.summary = out.summary.trim().replace(/\s+/g, ' ').slice(0, 400);
    out.ok = out.status >= 200 && out.status < 400;
  } catch (e) {
    out.error = String(e.message || e);
  }
  return out;
}

(async () => {
  const browser = await chromium.launch({ headless: true });
  const page = await browser.newPage();
  const results = [];
  for (const item of PAGES) {
    process.stderr.write(`crawl ${item.id}...\n`);
    const r = await scrape(page, item);
    results.push(r);
    process.stderr.write(`  -> ${r.ok ? 'ok' : 'FAIL'} ${r.status || ''} ${r.title || r.error || ''}\n`);
  }
  await browser.close();
  const outPath = '/Users/jubayer_1228/Documents/Projects/potential-project/thine-metrics/docs/datadog_obs18_crawl.json';
  fs.writeFileSync(outPath, JSON.stringify({ crawled_at: new Date().toISOString(), results }, null, 2));
  console.log(JSON.stringify({ outPath, count: results.length, ok: results.filter(r => r.ok).length }, null, 2));
})().catch(e => { console.error(e); process.exit(1); });
