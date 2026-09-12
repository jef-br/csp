"""
Quantize a BiRefNet export to int8, the way that actually goes faster.

    python "tools/3. quantize_int8.py" birefnet_lite_384.onnx

Writes `<stem>_int8.onnx` beside the input. Run from the repo root; the
calibration set is read from `JB core img batch/`.

Result on an i7-12700H, 384px: a 37-image CSP batch drops from 17.50s to 8.21s
(2.13x) and csp.exe from 194.3MiB to 79.6MiB, at mask IoU 0.9814 against fp32.

Three settings are load-bearing. Each was measured; none is cosmetic.

  activation_type=QUInt8 (U8S8), not QInt8.
      S8S8 is what ONNX Runtime's docs nominally recommend and it gives only
      1.05x. U8S8 hits the fast MLAS integer path on x86 and gives 1.20x, at
      identical accuracy. This one setting is worth 14%.

  "Gemm" in op_types_to_quantize.
      quant_pre_process fuses MatMul+Add into Gemm before quantization, so a
      list of just Conv/MatMul silently skips 96 projection layers - a quarter
      of total runtime - and you get a model that is barely faster.

  Nothing else in op_types_to_quantize.
      Adding LayerNormalization/Softmax, or Relu/Add/Concat, collapses the mask:
      IoU 0.65, several outputs fully empty, for under 5% speed. Do not.

reduce_range=False is correct on any VNNI-capable CPU (Alder Lake and later);
it exists only to avoid saturation on older AVX2/AVX512 parts.

The consumer must run the session at GraphOptimizationLevel::Level3. The QDQ
triplets this writes only fuse into real int8 kernels at Level2 and above; at
Level1 the graph unpacks int8 to float at every layer and runs *slower* than
fp32. See `src/core/shot_classifier/birefnet.rs`.

Requires: onnx, onnxruntime, numpy, pillow
"""
import glob
import os
import sys
import time

import numpy as np
from PIL import Image
from onnxruntime.quantization import (CalibrationDataReader, CalibrationMethod,
                                      QuantFormat, QuantType, quantize_static)
from onnxruntime.quantization.shape_inference import quant_pre_process

MEAN = np.array([0.485, 0.456, 0.406], dtype=np.float32)
STD = np.array([0.229, 0.224, 0.225], dtype=np.float32)

# Calibration decides the activation ranges baked into the model, so it must be
# drawn from the images this will actually see. Generic photos give worse edges.
CALIB_DIR = "JB core img batch"


def preprocess(path, side):
    """Must match the Rust side exactly - see birefnet.rs::segment."""
    img = Image.open(path).convert("RGB").resize((side, side), Image.Resampling.BILINEAR)
    arr = np.asarray(img, dtype=np.float32) / 255.0
    arr = (arr - MEAN) / STD
    return np.ascontiguousarray(arr.transpose(2, 0, 1)[None]).astype(np.float32)


def model_side(path):
    import onnx
    dims = onnx.load(path, load_external_data=False).graph.input[0].type.tensor_type.shape.dim
    side = dims[2].dim_value
    if side <= 0:
        sys.exit("model has a dynamic input size; re-export at a fixed square side")
    return side


class GarmentCalibration(CalibrationDataReader):
    def __init__(self, paths, side):
        self.paths, self.side = paths, side
        self.it = iter(paths)
        print(f"calibration set: {len(paths)} images at {side}x{side}", flush=True)

    def get_next(self):  # type: ignore[override]  # None signals end-of-data
        path = next(self.it, None)
        return None if path is None else {"input_image": preprocess(path, self.side)}

    def rewind(self):
        self.it = iter(self.paths)


def main():
    if len(sys.argv) < 2:
        sys.exit('usage: quantize_int8.py MODEL.onnx   (run from the repo root)')
    src = sys.argv[1]
    if not os.path.isfile(src):
        sys.exit(f"not found: {src}")
    side = model_side(src)

    paths = sorted(glob.glob(os.path.join(CALIB_DIR, "*.jpg")))
    if not paths:
        sys.exit(f"no calibration images in {CALIB_DIR!r} - run from the repo root")

    stem = os.path.splitext(src)[0]
    prepped, out = f"{stem}_prep.onnx", f"{stem}_int8.onnx"

    print("shape inference ...", flush=True)
    quant_pre_process(src, prepped, skip_symbolic_shape=False, auto_merge=True)

    print("calibrating and quantizing ...", flush=True)
    t0 = time.time()
    quantize_static(
        model_input=prepped,
        model_output=out,
        calibration_data_reader=GarmentCalibration(paths, side),
        quant_format=QuantFormat.QDQ,
        calibrate_method=CalibrationMethod.MinMax,
        activation_type=QuantType.QUInt8,   # U8S8 - see the docstring
        weight_type=QuantType.QInt8,
        per_channel=True,
        reduce_range=False,                 # VNNI CPU, not needed
        op_types_to_quantize=["Conv", "Gemm", "MatMul"],
    )
    os.remove(prepped)

    sidecars = glob.glob(f"{stem}_int8.onnx.data")
    if sidecars:
        sys.exit(f"quantizer emitted external data {sidecars}; build.rs embeds one "
                 f"blob, so fold it back in before shipping this model")

    print(f"\ndone in {time.time() - t0:.0f}s")
    print(f"{out}  {os.path.getsize(out) / 2**20:.1f} MB "
          f"(from {os.path.getsize(src) / 2**20:.1f} MB)")
    print("\nremember: the session must run at GraphOptimizationLevel::Level3")


if __name__ == "__main__":
    main()
