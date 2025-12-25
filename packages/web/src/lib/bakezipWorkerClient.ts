import type {
  CompatibilityLevel,
  InspectConfig,
  InspectedArchive,
  ZipWarning,
} from "bakezip";
import type {
  RequestMessage,
  ResponseMessage,
  WorkerRPC,
} from "../workers/bakezip.worker";

let workerSingleton: Worker | null = null;
const pending = new Map<
  string | number,
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
    const msg = event.data as ResponseMessage<any>;
    const handler = pending.get(msg.id);
    if (!handler) {
      return;
    }
    pending.delete(msg.id);

    if (msg.ok) {
      handler.resolve(msg.payload);
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

async function callWorker<K extends keyof WorkerRPC>(
  type: K,
  payload: WorkerRPC[K]["Request"],
): Promise<WorkerRPC[K]["Response"]> {
  const id = crypto.randomUUID();
  const worker = getWorker();
  const message = { id, type, payload } as RequestMessage<
    WorkerRPC[K]["Request"]
  >;

  return new Promise<WorkerRPC[K]["Response"]>((resolve, reject) => {
    pending.set(id, { resolve, reject });
    worker.postMessage(message);
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

  constructor(payload: WorkerRPC["parse"]["Response"]) {
    this.#processorId = payload.processorId;
    this.#compatibility = payload.compatibility;
    this.#warnings = payload.warnings;
  }

  get compatibility(): CompatibilityLevel {
    return this.#compatibility;
  }

  get warnings(): readonly ZipWarning[] {
    return this.#warnings;
  }

  async inspect(config: InspectConfig): Promise<InspectedArchive> {
    const result = await callWorker("inspect", {
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
    const result = await callWorker("rebuild", {
      processorId: this.#processorId,
      config,
      omitEntries: omit,
    });
    return result.blob;
  }

  async dispose(): Promise<void> {
    await callWorker("dispose", {
      processorId: this.#processorId,
    });
  }
}

export async function initializeWorker(): Promise<void> {
  await callWorker("initialize", {});
}

export async function parseZipInWorker(
  file: File | Blob,
): Promise<ZipProcessorProxy> {
  const result = await callWorker("parse", { file });
  return new ZipProcessorWorkerProxyImpl(result);
}
