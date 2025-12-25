import type {
  CompatibilityLevel,
  InspectConfig,
  InspectedArchive,
  ZipWarning,
} from "bakezip";

type WorkerRequest =
  | { id: number; type: "parse"; file: File | Blob }
  | {
      id: number;
      type: "inspect";
      processorId: number;
      config: InspectConfig;
    }
  | {
      id: number;
      type: "rebuild";
      processorId: number;
      config: InspectConfig;
      omitEntries: bigint[];
    }
  | { id: number; type: "dispose"; processorId: number };

type WorkerRequestNoId =
  | { type: "initialize" }
  | { type: "parse"; file: File | Blob }
  | { type: "inspect"; processorId: number; config: InspectConfig }
  | {
      type: "rebuild";
      processorId: number;
      config: InspectConfig;
      omitEntries: bigint[];
    }
  | { type: "dispose"; processorId: number };

type WorkerSuccess<T> = { id: number; ok: true; result: T };
type WorkerError = { id: number; ok: false; error: string };

type WorkerResponse<T> = WorkerSuccess<T> | WorkerError;

type ParseResult = {
  processorId: number;
  compatibility: CompatibilityLevel;
  warnings: ZipWarning[];
};

type InspectResult = { inspected: InspectedArchive };

type RebuildResult = { blob: Blob };

let workerSingleton: Worker | null = null;
let nextRequestId = 1;
const pending = new Map<
  number,
  { resolve: (value: any) => void; reject: (reason?: any) => void }
>();

function getWorker(): Worker {
  if (workerSingleton) {
    return workerSingleton;
  }

  const worker = new Worker(
    new URL("../workers/bakezip.worker.ts", import.meta.url),
    {
      type: "module",
    },
  );

  worker.onmessage = (event: MessageEvent) => {
    const msg = event.data as WorkerResponse<any>;
    const handler = pending.get(msg.id);
    if (!handler) return;
    pending.delete(msg.id);

    if (msg.ok) {
      handler.resolve(msg.result);
    } else {
      handler.reject(new Error(msg.error));
    }
  };

  worker.onerror = (event) => {
    for (const { reject } of pending.values()) {
      reject(new Error(event.message));
    }
    pending.clear();
  };

  workerSingleton = worker;
  return worker;
}

async function callWorker<T>(message: WorkerRequestNoId): Promise<T> {
  const id = nextRequestId++;
  const worker = getWorker();
  const payload = { id, ...message } as WorkerRequest;

  return new Promise<T>((resolve, reject) => {
    pending.set(id, { resolve, reject });
    worker.postMessage(payload);
  });
}

export type ZipProcessorProxy = {
  readonly compatibility: CompatibilityLevel;
  readonly warnings: readonly ZipWarning[];

  inspect(config: InspectConfig): Promise<InspectedArchive>;
  rebuild(config: InspectConfig, omitEntries: BigUint64Array): Promise<Blob>;
  dispose(): Promise<void>;
};

class ZipProcessorWorkerProxyImpl implements ZipProcessorProxy {
  readonly #compatibility: CompatibilityLevel;
  readonly #warnings: readonly ZipWarning[];
  readonly #processorId: number;

  constructor(args: ParseResult) {
    this.#processorId = args.processorId;
    this.#compatibility = args.compatibility;
    this.#warnings = args.warnings;
  }

  get compatibility(): CompatibilityLevel {
    return this.#compatibility;
  }

  get warnings(): readonly ZipWarning[] {
    return this.#warnings;
  }

  async inspect(config: InspectConfig): Promise<InspectedArchive> {
    const result = await callWorker<InspectResult>({
      type: "inspect",
      processorId: this.#processorId,
      config,
    });
    return result.inspected;
  }

  async rebuild(
    config: InspectConfig,
    omitEntries: BigUint64Array,
  ): Promise<Blob> {
    const omit = Array.from(omitEntries);
    const result = await callWorker<RebuildResult>({
      type: "rebuild",
      processorId: this.#processorId,
      config,
      omitEntries: omit,
    });
    return result.blob;
  }

  async dispose(): Promise<void> {
    await callWorker<{ disposed: true }>({
      type: "dispose",
      processorId: this.#processorId,
    });
  }
}

export async function initializeWorker(): Promise<void> {
  await callWorker<{ initialized: true }>({ type: "initialize" });
}

export async function parseZipInWorker(
  file: File | Blob,
): Promise<ZipProcessorProxy> {
  const result = await callWorker<ParseResult>({ type: "parse", file });
  return new ZipProcessorWorkerProxyImpl(result);
}
