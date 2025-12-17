# pppocr - WASM Build

High-performance OCR for WebAssembly, powered by ONNX Runtime and PaddleOCR models.

## Installation

```bash
npm install pppocr
# or
pnpm add pppocr
# or
yarn add pppocr
```

## Quick Start

### Basic Usage

```typescript
import init, { OnnxOcrEngine } from "pppocr";
import { InferenceSession } from "onnxruntime-web";

// Initialize the WASM module
await init();

// Create ONNX Runtime sessions for each model
const detectionSession = await InferenceSession.create("path/to/det.onnx");
const rotationSession = await InferenceSession.create("path/to/cls.onnx");
const recognitionSession = await InferenceSession.create("path/to/rec.onnx");

// Create the OCR engine
const engine = new OnnxOcrEngine(
  detectionSession,
  rotationSession,
  recognitionSession,
  "dict.txt", // Character dictionary
  false // host_big_endian
);

// Process an image
const imageData = /* ImageData or Uint8Array */;
const result = await engine.detect(imageData, config);

console.log(result); // OCR detection results
```

### Configuration

The engine accepts a pipeline configuration object:

```typescript
interface WasmPipelineConfig {
  pipeline?: {
    // Text detection configuration
    detection?: {
      box_score_thresh?: number; // Confidence threshold (default: 0.6)
      unclip_ratio?: number; // Unclip ratio for box expansion
      max_candidates?: number; // Maximum box candidates
    };
    // Text rotation classification
    rotation?: {
      score_thresh?: number; // Classification confidence threshold
    };
    // Text recognition configuration
    recognition?: {
      batch_size?: number; // Batch size for recognition
    };
  };
  hook?: {
    // Post-detection hook: filter or modify detected boxes
    onTextBoxes?: (boxes: TextBox[]) => TextBox[];
    // Pre-rotation hook: override rotation for specific boxes
    predefineRotation?: (
      rotation: TextRotation,
      index: number,
      box: TextBox,
    ) => TextRotationWithConfidence;
  };
}
```

### Using with Hooks

```typescript
const config = {
  pipeline: {
    detection: {
      box_score_thresh: 0.5,
    },
  },
  hook: {
    onTextBoxes: (boxes) => {
      // Filter out low-confidence boxes
      return boxes.filter((box) => box.score > 0.7);
    },
    predefineRotation: (rotation, index, box) => {
      // Override rotation if needed
      if (index === 0) {
        return { rotation: "Normal", confidence: 1.0 };
      }
      return { rotation, confidence: 0.9 };
    },
  },
};

const result = await engine.detect(imageData, config);
```

## API Reference

### OnnxOcrEngine

#### Constructor

```typescript
new OnnxOcrEngine(
  detectionSession: InferenceSession,
  rotationSession: InferenceSession,
  recognitionSession: InferenceSession,
  dictionary: string,
  host_big_endian: boolean
)
```

#### Methods

##### `detect(image: ImageSource, config?: WasmPipelineConfig): Promise<OcrOutput>`

Performs OCR detection on the input image.

**Parameters:**

- `image`: Image data (ImageData, Uint8Array, or raw pixel data)
- `config`: Pipeline configuration (optional)

**Returns:** Promise resolving to OCR results containing detected text boxes and recognized text.

### Types

#### ImageSource

```typescript
type ImageSource =
  | ImageData
  | {
      width: number;
      height: number;
      data: Uint8Array;
      channels?: 3 | 4; // RGB or RGBA, default: 3
    };
```

#### TextBox

Detected text box with coordinates and confidence score.

```typescript
interface TextBox {
  points: Point[]; // Bounding box corners
  score: number; // Detection confidence (0-1)
  text: string; // Recognized text
  confidence: number; // Recognition confidence (0-1)
}
```

#### Point

```typescript
interface Point {
  x: number;
  y: number;
}
```

#### TextRotation

```typescript
type TextRotation = "Normal" | "Rotated90" | "Rotated180" | "Rotated270";
```

#### OcrOutput

```typescript
interface OcrOutput {
  text_boxes: TextBox[]; // All detected text boxes
  timings?: {
    detection?: number;
    rotation?: number;
    recognition?: number;
  };
}
```

## Model Files

You'll need to download the PaddleOCR model files. The recommended models are:

- **Detection**: `det.onnx` or `det_v3_mobile.onnx`
- **Classification (Rotation)**: `cls.onnx` or `lcnet_x0_25.onnx`
- **Recognition**: `rec.onnx` or `rec_v5_mobile.onnx`
- **Dictionary**: `ppocrv5_dict.txt`

Models can be found at:

- [PaddleOCR Official Models](https://github.com/PaddlePaddle/PaddleOCR/blob/release/2.7/doc/doc_en/models_list_en.md)
- [ONNX Model Hub](https://github.com/onnx/models)

## Performance Tips

1. **Choose the right model size**: Mobile models (`*_mobile.onnx`) are faster but less accurate than server models
2. **Adjust confidence thresholds**: Lower thresholds detect more text but may include false positives
3. **Batch recognition**: The engine automatically batches character recognition for better performance
4. **Preprocessing**: Ensure input images are properly formatted (RGB or RGBA)

## Browser Support

This package requires:

- WebAssembly support
- Modern JavaScript (ES2020+)
- ONNX Runtime Web for inference

Tested on:

- Chrome/Edge 90+
- Firefox 88+
- Safari 14+

## Build Information

This WASM module is built from the [pppocr](https://github.com/SegaraRai/pppocr) Rust project using `wasm-pack`. The module uses the `wasm-js-full` feature which includes:

- ONNX Runtime Web backend
- Parallel processing (Rayon)
- Zero-copy tensor conversion
- Full TypeScript type definitions

## License

Apache-2.0

## Acknowledgements

- [PaddleOCR](https://www.paddleocr.ai/) for the original OCR models
- [ONNX Runtime](https://onnxruntime.ai/) for the inference engine
- [wasm-pack](https://rustwasm.org/docs/wasm-pack/) for Rust-to-WASM compilation
