import { connect, JetStreamManager, StringCodec } from 'nats';

export interface SeedEvent {
  event: string;
  ts: string;
  data?: Record<string, unknown>;
  stateFrom?: string;
  stateTo?: string;
}

export interface SeedRunOptions {
  runId: string;
  ritualId?: string;
  tenantId?: string;
  events: SeedEvent[];
}

const DEFAULT_STREAM_NAME = process.env.RITUAL_STREAM_NAME || 'RITUAL_EVENTS';
const DEFAULT_SUBJECT_PREFIX = process.env.RITUAL_SUBJECT_PREFIX || 'demon.ritual.v1';
const DEFAULT_NATS_URL = process.env.NATS_URL || 'nats://127.0.0.1:4222';

async function ensureStream(jsm: JetStreamManager, stream: string, subjectPrefix: string) {
  try {
    await jsm.streams.info(stream);
    return;
  } catch (error) {
    // Stream missing; fall through to create
  }

  await jsm.streams.add({
    name: stream,
    subjects: [`${subjectPrefix}.>`],
    retention: 'limits',
    discard: 'old',
    storage: 'file',
    max_msgs_per_subject: -1,
  });
}

export async function seedRun({
  runId,
  ritualId = 'playwright-ritual',
  tenantId = 'default',
  events,
}: SeedRunOptions) {
  await publishEvents({ runId, ritualId, tenantId, events }, { purge: true });
}

export async function appendEvents({
  runId,
  ritualId = 'playwright-ritual',
  tenantId = 'default',
  events,
}: SeedRunOptions) {
  await publishEvents({ runId, ritualId, tenantId, events }, { purge: false });
}

async function publishEvents(
  {
    runId,
    ritualId,
    tenantId,
    events,
  }: Required<Omit<SeedRunOptions, 'events'>> & { events: SeedEvent[] },
  opts: { purge: boolean },
) {
  if (!events.length) {
    throw new Error('seedRun requires at least one event');
  }

  const nc = await connect({ servers: DEFAULT_NATS_URL });
  try {
    const jsm = await nc.jetstreamManager();
    await ensureStream(jsm, DEFAULT_STREAM_NAME, DEFAULT_SUBJECT_PREFIX);

    const subject = `${DEFAULT_SUBJECT_PREFIX}.${ritualId}.${runId}.events`;

    if (opts.purge) {
      // Purge prior subject data for determinism (ignore errors when subject missing)
      try {
        await jsm.streams.purge(DEFAULT_STREAM_NAME, { filter: subject });
      } catch (error) {
        // Subject missing is fine for cold start
      }
    }

    const js = await nc.jetstream();
    const sc = StringCodec();

    for (const evt of events) {
      const payload: Record<string, unknown> = {
        event: evt.event,
        ts: evt.ts,
        tenantId,
        runId,
        ritualId,
        ...(evt.data || {}),
      };
      if (evt.stateFrom) {
        payload.stateFrom = evt.stateFrom;
      }
      if (evt.stateTo) {
        payload.stateTo = evt.stateTo;
      }

      const data = sc.encode(JSON.stringify(payload));
      await js.publish(subject, data);
    }

    await nc.flush();
  } finally {
    await nc.close();
  }
}

export async function waitForOperateUI(baseUrl: string, timeoutMs = 30000) {
  const deadline = Date.now() + timeoutMs;
  let lastError: unknown;

  while (Date.now() < deadline) {
    try {
      const response = await fetch(`${baseUrl}/health`);
      if (response.ok || response.status >= 400) {
        return;
      }
    } catch (error) {
      lastError = error;
    }
    await new Promise(resolve => setTimeout(resolve, 500));
  }

  throw new Error(
    `Operate UI did not become ready within ${timeoutMs}ms${
      lastError ? ` (last error: ${String(lastError)})` : ''
    }`
  );
}

export function iso(ts: string | number | Date) {
  if (typeof ts === 'string') return ts;
  if (typeof ts === 'number') return new Date(ts).toISOString();
  return ts.toISOString();
}
