/// <reference lib="webworker" />

import init, {
  ZipProcessor,
  type CompatibilityLevel,
  type InspectConfig,
  type InspectedArchive,
  type ZipWarning,
} from "bakezip";

type RequestMessage =
  | {
      id: number;
      type: "initialize";
    }
  | {
      id: number;
      type: "parse";
      file: File | Blob;
    }
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
  | {
      id: number;
      type: "dispose";
      processorId: number;
    };

type SuccessResponseMessage<T> = {
  id: number;
  ok: true;
  result: T;
};

type ErrorResponseMessage = {
  id: number;
  ok: false;
  error: string;
};

type ResponseMessage<T> = SuccessResponseMessage<T> | ErrorResponseMessage;

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
  console.log(`bakezip wasm initialized in ${elapsed.toFixed(2)} ms`);
}

function postOk<T>(id: number, result: T): void {
  const msg: ResponseMessage<T> = { id, ok: true, result };
  (globalThis as unknown as DedicatedWorkerGlobalScope).postMessage(msg);
}

function postErr(id: number, err: unknown): void {
  const message = err instanceof Error ? err.message : String(err);
  const msg: ErrorResponseMessage = { id, ok: false, error: message };
  (globalThis as unknown as DedicatedWorkerGlobalScope).postMessage(msg);
}

(globalThis as unknown as DedicatedWorkerGlobalScope).onmessage = async (
  event,
) => {
  const data = event.data as RequestMessage;

  try {
    switch (data.type) {
      case "initialize": {
        await ensureInit();
        postOk(data.id, { initialized: true });
        return;
      }

      case "parse": {
        await ensureInit();
        const processor = await ZipProcessor.parse(data.file);
        const processorId = nextProcessorId++;
        processors.set(processorId, processor);

        const compatibility: CompatibilityLevel = processor.compatibility;
        const warnings: ZipWarning[] = processor.get_warnings();

        postOk(data.id, { processorId, compatibility, warnings });
        return;
      }

      case "inspect": {
        await ensureInit();
        const processor = processors.get(data.processorId);
        if (!processor) {
          throw new Error("Processor not found (maybe disposed)");
        }
        const inspected: InspectedArchive = processor.inspect(data.config);
        postOk(data.id, { inspected });
        return;
      }

      case "rebuild": {
        await ensureInit();
        const processor = processors.get(data.processorId);
        if (!processor) {
          throw new Error("Processor not found (maybe disposed)");
        }

        const omit = new BigUint64Array(data.omitEntries);
        const blob = processor.rebuild(data.config, omit);
        postOk(data.id, { blob });
        return;
      }

      case "dispose": {
        processors.get(data.processorId)?.free();
        processors.delete(data.processorId);
        postOk(data.id, { disposed: true });
        return;
      }

      default: {
        const exhaustive: never = data;
        throw new Error(`Unknown message type: ${(exhaustive as any).type}`);
      }
    }
  } catch (err) {
    postErr(data.id, err);
  }
};
