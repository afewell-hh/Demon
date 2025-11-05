import { connect, JetStreamManager, StringCodec } from 'nats';

export interface ScaleMetrics {
  queueLag: number;
  p95LatencyMs: number;
  errorRate: number;
  totalProcessed: number;
  totalErrors: number;
}

export interface SeedScaleHintOptions {
  tenantId: string;
  recommendation: 'scale_up' | 'scale_down' | 'steady';
  metrics: ScaleMetrics;
  reason: string;
  traceId?: string;
}

const SCALE_HINT_STREAM_NAME = 'SCALE_HINTS';
const SCALE_HINT_SUBJECT_PREFIX = 'demon.scale.v1';
const DEFAULT_NATS_URL = process.env.NATS_URL || 'nats://127.0.0.1:4222';

async function ensureScaleHintStream(jsm: JetStreamManager) {
  try {
    await jsm.streams.info(SCALE_HINT_STREAM_NAME);
    return;
  } catch (error) {
    // Stream missing; fall through to create
  }

  await jsm.streams.add({
    name: SCALE_HINT_STREAM_NAME,
    subjects: [`${SCALE_HINT_SUBJECT_PREFIX}.*.hints`],
    retention: 'limits',
    discard: 'old',
    storage: 'file',
    max_msgs_per_subject: 10, // Keep last 10 hints per tenant
  });
}

export async function seedScaleHint({
  tenantId,
  recommendation,
  metrics,
  reason,
  traceId,
}: SeedScaleHintOptions) {
  const nc = await connect({ servers: DEFAULT_NATS_URL });
  try {
    const jsm = await nc.jetstreamManager();
    await ensureScaleHintStream(jsm);

    const subject = `${SCALE_HINT_SUBJECT_PREFIX}.${tenantId}.hints`;

    const js = await nc.jetstream();
    const sc = StringCodec();

    const payload = {
      event: 'agent.scale.hint:v1',
      ts: new Date().toISOString(),
      tenantId,
      recommendation,
      metrics: {
        queueLag: metrics.queueLag,
        p95LatencyMs: metrics.p95LatencyMs,
        errorRate: metrics.errorRate,
        totalProcessed: metrics.totalProcessed,
        totalErrors: metrics.totalErrors,
      },
      thresholds: {
        queueLagHigh: 500,
        queueLagLow: 50,
        p95LatencyHighMs: 1000.0,
        p95LatencyLowMs: 100.0,
        errorRateHigh: 0.05,
      },
      hysteresis: {
        currentState: recommendation === 'scale_up' ? 'overload' : recommendation === 'scale_down' ? 'normal' : 'pressure',
        stateChangedAt: new Date().toISOString(),
        consecutiveHighSignals: recommendation === 'scale_up' ? 5 : 0,
        consecutiveLowSignals: recommendation === 'scale_down' ? 3 : 0,
        minSignalsForTransition: 3,
      },
      reason,
      ...(traceId && { traceId }),
    };

    await js.publish(subject, sc.encode(JSON.stringify(payload)));
  } finally {
    await nc.close();
  }
}

export async function clearScaleHints(tenantId: string) {
  const nc = await connect({ servers: DEFAULT_NATS_URL });
  try {
    const jsm = await nc.jetstreamManager();
    const subject = `${SCALE_HINT_SUBJECT_PREFIX}.${tenantId}.hints`;

    try {
      await jsm.streams.purge(SCALE_HINT_STREAM_NAME, { filter: subject });
    } catch (error) {
      // Ignore errors if stream or subject doesn't exist
    }
  } finally {
    await nc.close();
  }
}
