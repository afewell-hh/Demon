const { chromium } = require('playwright');
const { connect, StringCodec } = require('nats');

const NATS_URL = process.env.NATS_URL || 'nats://127.0.0.1:4222';
const STREAM_NAME = process.env.RITUAL_STREAM_NAME || 'RITUAL_EVENTS';
const SUBJECT_PREFIX = process.env.RITUAL_SUBJECT_PREFIX || 'demon.ritual.v1';
const BASE_URL = process.env.BASE_URL || 'http://127.0.0.1:3000';
const APPROVER = process.env.APPROVER_EMAIL || 'tester@example.com';

async function ensureStream(jsm) {
  try {
    await jsm.streams.info(STREAM_NAME);
  } catch (err) {
    await jsm.streams.add({
      name: STREAM_NAME,
      subjects: [`${SUBJECT_PREFIX}.>`],
      retention: 'limits',
      discard: 'old',
      storage: 'file',
    });
  }
}

async function seedRun(runId, events) {
  const nc = await connect({ servers: NATS_URL });
  try {
    const jsm = await nc.jetstreamManager();
    await ensureStream(jsm);
    const subject = `${SUBJECT_PREFIX}.playwright-smoke.${runId}.events`;
    try {
      await jsm.streams.purge(STREAM_NAME, { filter: subject });
    } catch (_) {
      // ignore
    }

    const js = await nc.jetstream();
    const sc = StringCodec();
    for (const evt of events) {
      const payload = {
        event: evt.event,
        ts: evt.ts,
        tenantId: 'smoke',
        runId,
        ritualId: 'playwright-smoke',
        ...evt.data,
      };
      await js.publish(subject, sc.encode(JSON.stringify(payload)));
    }
    await nc.flush();
  } finally {
    await nc.close();
  }
}

async function runScenario(runId, action) {
  const browser = await chromium.launch();
  const page = await browser.newPage();
  await page.goto(`${BASE_URL}/runs/${runId}`, { waitUntil: 'domcontentloaded' });
  await page.waitForSelector('#approval-actions');

  const beforeRows = await page.locator('.table tbody tr').count();

  await page.fill('#approver-email', APPROVER);
  await page.fill('#approval-note', action === 'grant' ? 'Manual smoke grant' : 'Manual smoke deny');
  await page.click(action === 'grant' ? '#grant-approval-btn' : '#deny-approval-btn');

  const expectedToast = action === 'grant' ? 'Approval granted successfully' : 'Approval denied successfully';
  await page.waitForFunction(
    (text) => {
      const toast = document.getElementById('approval-toast');
      return toast && toast.textContent && toast.textContent.includes(text);
    },
    expectedToast,
    { timeout: 15000 }
  );
  const expectedStatus = action === 'grant' ? 'Granted' : 'Denied';
  await page.locator('#approval-status', { hasText: expectedStatus }).waitFor({ timeout: 15000 });

  await page
    .waitForFunction(
      (targetCount) => document.querySelectorAll('.table tbody tr').length >= targetCount,
      beforeRows + 1,
      { timeout: 15000 }
    )
    .catch(() => {});
  const afterRows = await page.locator('.table tbody tr').count();

  await browser.close();
  return { beforeRows, afterRows, expectedToast, expectedStatus };
}

(async () => {
  const now = () => new Date().toISOString();
  const baseEvents = gate => ([
    { event: 'ritual.started:v1', ts: now(), data: {} },
    {
      event: 'approval.requested:v1',
      ts: now(),
      data: { gateId: gate, requester: 'alice@example.com', reason: 'Manual smoke' },
    },
  ]);

  const suffix = Date.now().toString(36);
  const grantRun = `manual-grant-${suffix}`;
  await seedRun(grantRun, baseEvents('gate-grant'));
  const grantResult = await runScenario(grantRun, 'grant');

  const denyRun = `manual-deny-${suffix}`;
  await seedRun(denyRun, baseEvents('gate-deny'));
  const denyResult = await runScenario(denyRun, 'deny');

  console.log(JSON.stringify({ grantResult, denyResult }, null, 2));
})();
