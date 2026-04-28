#!/usr/bin/env node
import { createRequire } from "node:module";
import { mkdir, rm, stat } from "node:fs/promises";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { redactVisibleText } from "./screenshot-redaction.mjs";

const require = createRequire(import.meta.url);
const { chromium } = require("../apps/dashboard/node_modules/playwright");
const repoRoot = join(dirname(fileURLToPath(import.meta.url)), "..");

const listenerUrl = process.env.AGENT_SHIELD_DASHBOARD_LISTENER_URL ?? "http://127.0.0.1:18081";
const outputDir = process.env.AGENT_SHIELD_DL_SCREENSHOT_DIR ?? join(repoRoot, "docs/screens/dl");
const chromePath = process.env.CHROME_PATH ?? "/usr/bin/google-chrome";
const viewport = { width: 1440, height: 1000 };

const screenshots = [];

function pngDimensions(buffer) {
  if (buffer.subarray(0, 8).toString("hex") !== "89504e470d0a1a0a") {
    throw new Error("not a PNG");
  }
  return {
    width: buffer.readUInt32BE(16),
    height: buffer.readUInt32BE(20),
  };
}

async function redactVisibleSecrets(page) {
  await redactVisibleText(page, "#detail-pane, #detail-content, pre, [data-copy]");
}

async function save(page, fileName) {
  await redactVisibleSecrets(page);
  const path = join(outputDir, fileName);
  const buffer = await page.screenshot({ path, fullPage: false });
  const dimensions = pngDimensions(buffer);
  const file = await stat(path);
  screenshots.push({ file: path, ...dimensions, bytes: file.size });
}

async function loadDashboard(page) {
  await page.goto(listenerUrl, { waitUntil: "networkidle" });
  await page.locator("#traffic-filters").waitFor({ timeout: 20_000 });
  await page.locator("#stats").waitFor({ timeout: 20_000 });
  await page.locator("#traffic-table tr[data-listener-id]").first().waitFor({ timeout: 20_000 });
}

async function clearFilters(page) {
  for (const name of ["q", "domain", "method", "phase"]) {
    const input = page.locator(`#traffic-filters input[name="${name}"]`);
    await input.fill("");
    await input.dispatchEvent("input");
    await input.dispatchEvent("change");
  }
  await page.waitForTimeout(900);
}

async function clickRow(page, selector, label) {
  const row = page.locator(selector).first();
  if (!(await row.count())) throw new Error(`Missing listener row: ${label}`);
  await row.scrollIntoViewIfNeeded();
  await row.click();
  await page.locator("#detail-content").waitFor({ timeout: 10_000 });
  await page.waitForTimeout(700);
}

async function clickDetailTab(page, label) {
  const tab = page.locator("#detail-content button", { hasText: label }).first();
  if (!(await tab.count())) throw new Error(`Missing detail tab: ${label}`);
  await tab.click();
  await page.waitForTimeout(700);
}

async function fillFilter(page, name, value) {
  const input = page.locator(`#traffic-filters input[name="${name}"]`);
  await input.fill(value);
  await input.dispatchEvent("input");
  await input.dispatchEvent("change");
  await page.waitForTimeout(700);
}

async function applyColumnDemoState(page) {
  await page.locator("#columns-menu summary").click();
  const flow = page.locator('.column-toggle[data-column="flow"]');
  if ((await flow.count()) && (await flow.isChecked())) {
    await flow.setChecked(false);
    await flow.dispatchEvent("change");
  }

  const resizer = page.locator('.column-resizer[data-column="url"]').first();
  const box = await resizer.boundingBox();
  if (box) {
    await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2);
    await page.mouse.down();
    await page.mouse.move(box.x - 120, box.y + box.height / 2, { steps: 8 });
    await page.mouse.up();
  }
  await page.waitForTimeout(500);
}

async function main() {
  await rm(outputDir, { recursive: true, force: true });
  await mkdir(outputDir, { recursive: true });

  const browser = await chromium.launch({
    channel: "chrome",
    executablePath: chromePath,
    headless: true,
  });

  try {
    const page = await browser.newPage({ viewport });
    await page.addInitScript(() => {
      localStorage.removeItem("agentShieldDashboardState");
    });

    await loadDashboard(page);
    await save(page, "dl-01-listener-overview.png");

    await fillFilter(page, "q", "cloudcode");
    await fillFilter(page, "phase", "sse.event.in");
    await save(page, "dl-02-search-filter-form.png");

    await loadDashboard(page);
    await clearFilters(page);
    await applyColumnDemoState(page);
    await save(page, "dl-03-columns-menu.png");

    await loadDashboard(page);
    await clearFilters(page);
    await page.locator('th[data-col="domain"] button').click();
    await page.locator('th[data-col="domain"] button', { hasText: /Domain.*↑/ }).waitFor({ timeout: 10_000 });
    await page.waitForTimeout(500);
    await save(page, "dl-04-sorted-table.png");

    await loadDashboard(page);
    await clearFilters(page);
    await clickRow(page, 'tr[data-listener-id]:has-text("api.anthropic.com"):has-text("/v1/messages"):has-text("request")', "anthropic request");
    await save(page, "dl-05-row-detail-right-pane.png");

    await clickDetailTab(page, "Request Headers");
    await save(page, "dl-06-request-headers.png");

    await clickDetailTab(page, "Request Body");
    await save(page, "dl-07-request-body.png");

    await loadDashboard(page);
    await clearFilters(page);
    const respButton = page
      .locator('tr[data-listener-id]:has-text("cloudcode-pa.googleapis.com"):has-text("response") button', { hasText: "RESP" })
      .first();
    if (await respButton.count()) {
      await respButton.click();
      await page.locator("#detail-content button", { hasText: "Response Body" }).waitFor({ timeout: 10_000 });
      await page.waitForTimeout(700);
    } else {
      await clickRow(page, 'tr[data-listener-id]:has-text("api.anthropic.com"):has-text("/v1/messages"):has-text("response")', "anthropic response");
      await clickDetailTab(page, "Response Body");
    }
    await save(page, "dl-08-response-body.png");
  } finally {
    await browser.close();
  }

  console.log(
    JSON.stringify(
      {
        ok: true,
        listenerUrl,
        outputDir,
        screenshots,
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
