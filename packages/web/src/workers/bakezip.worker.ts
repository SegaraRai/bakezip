/// <reference lib="webworker" />

import init, {
  ZipProcessor,
  type CompatibilityLevel,
  type InspectConfig,
  type InspectedArchive,
  type ZipWarning,
} from "bakezip";

export type WorkerRPC = {
  initialize: {
    Request: {};
    Response: {
      initialized: true;
    };
  };
  parse: {
    Request: {
      file: File | Blob;
    };
    Response: {
      processorId: number;
      compatibility: CompatibilityLevel;
      warnings: readonly ZipWarning[];
    };
  };
  inspect: {
    Request: {
      processorId: number;
      config: InspectConfig;
    };
    Response: {
      inspected: InspectedArchive;
    };
  };
  rebuild: {
    Request: {
      processorId: number;
      config: InspectConfig;
      omitEntries: bigint[];
    };
    Response: {
      blob: Blob;
    };
  };
  dispose: {
    Request: {
      processorId: number;
    };
    Response: {
      disposed: true;
    };
  };
};

export type SuccessResponseMessage<T> = {
  id: string | number;
  ok: true;
  payload: T;
};

export type ErrorResponseMessage = {
  id: string | number;
  ok: false;
  error: string;
};

export type ResponseMessage<T> =
  | SuccessResponseMessage<T>
  | ErrorResponseMessage;

export type RequestMessage<T> = {
  id: string | number;
  type: keyof WorkerRPC;
  payload: T;
};

const processors = new Map<number, ZipProcessor>();
let nextProcessorId = 1;

let initialized = false;
async function ensureInit(): Promise<void> {
  if (initialized) {
    return;
  }

  const ts = performance.now();
  await init();
  initialized = true;

  const elapsed = performance.now() - ts;
  console.log(`Worker initialized in ${elapsed.toFixed(2)} ms`);
}

const handler = {
  async initialize(): Promise<{ initialized: true }> {
    await ensureInit();
    return { initialized: true };
  },

  async parse(payload: { file: File | Blob }): Promise<{
    processorId: number;
    compatibility: CompatibilityLevel;
    warnings: readonly ZipWarning[];
  }> {
    await ensureInit();
    const processor = await ZipProcessor.parse(payload.file);
    const processorId = nextProcessorId++;
    processors.set(processorId, processor);

    const compatibility: CompatibilityLevel = processor.compatibility;
    const warnings: ZipWarning[] = processor.get_warnings();

    return { processorId, compatibility, warnings };
  },

  async inspect(payload: {
    processorId: number;
    config: InspectConfig;
  }): Promise<{ inspected: InspectedArchive }> {
    await ensureInit();
    const processor = processors.get(payload.processorId);
    if (!processor) {
      throw new Error("Processor not found (maybe disposed)");
    }
    const inspected: InspectedArchive = processor.inspect(payload.config);
    return { inspected };
  },

  async rebuild(payload: {
    processorId: number;
    config: InspectConfig;
    omitEntries: bigint[];
  }): Promise<{ blob: Blob }> {
    await ensureInit();
    const processor = processors.get(payload.processorId);
    if (!processor) {
      throw new Error("Processor not found (maybe disposed)");
    }

    const omit = new BigUint64Array(payload.omitEntries);
    const blob = processor.rebuild(payload.config, omit);
    return { blob };
  },

  async dispose(payload: { processorId: number }): Promise<{ disposed: true }> {
    processors.get(payload.processorId)?.free();
    processors.delete(payload.processorId);
    return { disposed: true };
  },
} satisfies {
  [K in keyof WorkerRPC]: (
    payload: WorkerRPC[K]["Request"],
  ) => Promise<WorkerRPC[K]["Response"]>;
};

(globalThis as unknown as DedicatedWorkerGlobalScope).onmessage = async (
  event,
) => {
  const data = event.data as RequestMessage<any>;

  try {
    const func = handler[data.type];
    if (!func) {
      throw new Error(`Unknown message type: ${data.type}`);
    }

    const payload = await func(data.payload);

    const msg: ResponseMessage<typeof payload> = {
      id: data.id,
      ok: true,
      payload,
    };
    (globalThis as unknown as DedicatedWorkerGlobalScope).postMessage(msg);
  } catch (err) {
    const message = err instanceof Error ? err.message : String(err);
    const msg: ErrorResponseMessage = {
      id: data.id,
      ok: false,
      error: message,
    };
    (globalThis as unknown as DedicatedWorkerGlobalScope).postMessage(msg);
  }
};
