#!/usr/bin/env node
import { createRequire } from "node:module";
import { mkdir, stat } from "node:fs/promises";
import { spawnSync } from "node:child_process";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { redactVisibleText } from "./screenshot-redaction.mjs";

const require = createRequire(import.meta.url);
const { chromium } = require("../apps/dashboard/node_modules/playwright");
const repoRoot = join(dirname(fileURLToPath(import.meta.url)), "..");

const dashboardUrl = process.env.AGENT_SHIELD_DASHBOARD_URL ?? "http://127.0.0.1:9999";
const proxyUrl = process.env.AGENT_SHIELD_PROXY_URL ?? "http://127.0.0.1:8888";
const outputDir = process.env.AGENT_SHIELD_SCREENSHOT_DIR ?? join(repoRoot, "screens");
const chromePath = process.env.CHROME_PATH ?? "/usr/bin/google-chrome";
const viewport = { width: 1440, height: 1000 };

const screenshots = [];
const notes = [];

function run(command, args, options = {}) {
  const result = spawnSync(command, args, {
    encoding: "utf8",
    timeout: options.timeout ?? 120_000,
    env: { ...process.env, ...options.env },
  });
  return {
    command: [command, ...args].join(" "),
    status: result.status,
    stdout: result.stdout?.trim() ?? "",
    stderr: result.stderr?.trim() ?? "",
  };
}

async function readJson(path) {
  const response = await fetch(`${dashboardUrl}${path}`);
  if (!response.ok) throw new Error(`${path} returned HTTP ${response.status}`);
  return response.json();
}

async function hasDetailedTraffic() {
  const traffic = await readJson("/api/traffic");
  return traffic.some(
    (entry) =>
      entry.domain?.includes("googleapis.com") &&
      (entry.req_body_file || entry.req_body) &&
      (entry.resp_body_file || entry.resp_body),
  );
}

async function generateTrafficIfNeeded() {
  const curl = run("curl", [
    "-k",
    "-I",
    "-x",
    proxyUrl,
    "https://api.anthropic.com",
    "--max-time",
    "30",
  ]);
  notes.push(`traffic probe: ${curl.command} -> ${curl.status}`);

  if (await hasDetailedTraffic()) return;

  const gemini = run("ash", ["gemini", "-p", "say ok"], {
    env: { AGENT_SHIELD_GEMINI_HOME: "/root" },
    timeout: 180_000,
  });
  notes.push(`fallback LLM probe: ${gemini.command} -> ${gemini.status}`);
}

function pngDimensions(buffer) {
  const signature = "89504e470d0a1a0a";
  if (buffer.subarray(0, 8).toString("hex") !== signature) {
    throw new Error("not a PNG");
  }
  return {
    width: buffer.readUInt32BE(16),
    height: buffer.readUInt32BE(20),
  };
}

async function save(page, fileName) {
  const path = join(outputDir, fileName);
  const buffer = await page.screenshot({ path, fullPage: true });
  const dimensions = pngDimensions(buffer);
  const file = await stat(path);
  screenshots.push({ file: path, ...dimensions, bytes: file.size });
}

async function redactVisibleDetail(page) {
  await redactVisibleText(page, ".detail-body, [data-testid='detail-pane'], [data-testid='packet-body-viewer']");
}

async function clickFirst(locator, label) {
  const count = await locator.count();
  if (!count) throw new Error(`Missing ${label}`);
  await locator.first().scrollIntoViewIfNeeded();
  await locator.first().click();
}

function firstExisting(page, selectors) {
  return page.locator(selectors.join(", "));
}

function detailTab(page, key) {
  const aliases = {
    overview: ["detail-tab-overview"],
    reqh: ["detail-tab-reqh", "detail-tab-request-headers"],
    resph: ["detail-tab-resph", "detail-tab-response-headers"],
    req: ["detail-tab-req", "detail-tab-request-body"],
    resp: ["detail-tab-resp", "detail-tab-response-body"],
  }[key];
  return page.locator(aliases.map((id) => `[data-testid="${id}"]`).join(", "));
}

function legacyDetailTab(page, label) {
  return page.locator(".detail-tab", { hasText: label });
}

async function clickTabIfPresent(page, key, fileName) {
  const labels = {
    overview: "Overview",
    reqh: "Request Headers",
    resph: "Response Headers",
    req: "Request Body",
    resp: "Response Body",
  };
  const tab = detailTab(page, key);
  const locator = (await tab.count()) ? tab : legacyDetailTab(page, labels[key]);
  if (!(await locator.count())) {
    notes.push(`skipped ${fileName}: tab not present`);
    return;
  }
  await clickFirst(locator, `detail tab ${key}`);
  await page.waitForTimeout(700);
  await redactVisibleDetail(page);
  await save(page, fileName);
}

async function chooseDetailedTrafficRow(page) {
  const rows = firstExisting(page, ['[data-testid="traffic-row"]', "tbody tr"]);
  await rows.first().waitFor({ timeout: 20_000 });

  const chosen = await rows.evaluateAll((allRows) => {
    const score = (row) => {
      const text = row.textContent ?? "";
      let value = 0;
      if (text.includes("cloudcode-pa.googleapis.com")) value += 50;
      if (text.includes("chatgpt.com")) value += 30;
      if (row.querySelector('[data-testid="open-request-body"]')) value += 20;
      if (row.querySelector('[data-testid="open-response-body"]')) value += 20;
      if (text.includes("POST")) value += 10;
      if (text.includes("response")) value += 5;
      return value;
    };
    let bestIndex = 0;
    let bestScore = -1;
    allRows.forEach((row, index) => {
      const rowScore = score(row);
      if (rowScore > bestScore) {
        bestIndex = index;
        bestScore = rowScore;
      }
    });
    return bestIndex;
  });

  const row = rows.nth(chosen);
  await row.scrollIntoViewIfNeeded();
  await row.click();
}

async function main() {
  await mkdir(outputDir, { recursive: true });
  const beforeStats = await readJson("/api/stats");
  await generateTrafficIfNeeded();
  const browser = await chromium.launch({
    channel: "chrome",
    executablePath: chromePath,
    headless: true,
  });

  try {
    const page = await browser.newPage({ viewport });
    await page.goto(dashboardUrl, { waitUntil: "networkidle" });
    await firstExisting(page, ['[data-testid="dashboard-root"]', "h1"]).first().waitFor({ timeout: 20_000 });
    await firstExisting(page, ['[data-testid="stats-bar"]', ".stats"]).first().waitFor({ timeout: 20_000 });
    await firstExisting(page, ['[data-testid="traffic-table"]', "table"]).first().waitFor({ timeout: 20_000 });
    await firstExisting(page, ['[data-testid="traffic-row"]', "tbody tr"]).first().waitFor({ timeout: 20_000 });

    await save(page, "demo-01-overview-stats.png");
    await save(page, "demo-02-traffic-table.png");

    await chooseDetailedTrafficRow(page);
    await firstExisting(page, ['[data-testid="detail-pane"]', ".detail-pane.open"]).first().waitFor({ timeout: 10_000 });
    await firstExisting(page, ['[data-testid="detail-overview"]', ".detail-body"]).first().waitFor({ timeout: 10_000 });
    await redactVisibleDetail(page);
    await save(page, "demo-03-row-detail-overview.png");

    await clickTabIfPresent(page, "reqh", "demo-04-request-headers.png");
    await clickTabIfPresent(page, "resph", "demo-05-response-headers.png");
    await clickTabIfPresent(page, "req", "demo-06-request-body.png");
    await clickTabIfPresent(page, "resp", "demo-07-response-body.png");

    const packetTab = (await page.getByTestId("tab-packets").count())
      ? page.getByTestId("tab-packets")
      : page.locator(".tab", { hasText: "Packets" });
    await clickFirst(packetTab, "Packets tab");
    await firstExisting(page, ['[data-testid="packet-body-list"]', ".body-item"]).first().waitFor({ timeout: 10_000 });
    await clickFirst(firstExisting(page, ['[data-testid="packet-body-list-item"]', ".body-item"]), "packet body list item");
    await firstExisting(page, ['[data-testid="packet-body-viewer"]', ".detail-body"]).first().waitFor({ timeout: 10_000 });
    await redactVisibleDetail(page);
    await save(page, "demo-08-packets-body-viewer.png");
  } finally {
    await browser.close();
  }

  const afterStats = await readJson("/api/stats");
  const bodies = await readJson("/api/bodies");
  console.log(
    JSON.stringify(
      {
        ok: true,
        dashboardUrl,
        outputDir,
        beforeStats,
        afterStats,
        bodiesCount: bodies.length,
        screenshots,
        notes,
      },
      null,
      2,
    ),
  );
}

main().catch((error) => {
  console.error(error.stack ?? error.message);
  process.exit(1);
});
