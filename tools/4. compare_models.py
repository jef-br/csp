"""
A/B two BiRefNet exports: speed under CSP's real concurrency, and mask agreement.

    python "tools/4. compare_models.py" birefnet_lite_512.onnx birefnet_lite_384_int8.onnx

The first model given is the reference; IoU is measured against its masks. Run
from the repo root - test images come from `JB core img batch/`.

Speed is measured the way CSP actually runs: one shared session driven by N
concurrent single-threaded workers, because `app::batch` fans images across
cores with rayon and the session is built with `with_intra_threads(1)`. Timing a
lone forward pass overstates the gain - this workload is memory-bandwidth bound,
so a model that is 1.43x faster alone is only 1.20x faster under full load.

IoU is reported mean and worst. Worst matters more: aggregate IoU is dominated
by interior pixels on large-subject shots and hides exactly the edge damage you
would notice. Masks are compared after upsampling to a common size, thresholded
in logit space at 0 - the same comparison birefnet.rs makes.

Requires: onnx, onnxruntime, numpy, pillow
"""
import glob
import os
import sys
import threading
import time

import numpy as np
import onnx
import onnxruntime as ort
from PIL import Image

MEAN = np.array([0.485, 0.456, 0.406], dtype=np.float32)
STD = np.array([0.229, 0.224, 0.225], dtype=np.float32)
IMAGE_DIR = "JB core img batch"
WORKERS = 20
COMMON = 1024


def preprocess(path, side):
    img = Image.open(path).convert("RGB").resize((side, side), Image.Resampling.BILINEAR)
    arr = (np.asarray(img, dtype=np.float32) / 255.0 - MEAN) / STD
    return np.ascontiguousarray(arr.transpose(2, 0, 1)[None]).astype(np.float32)


def model_side(path):
    d = onnx.load(path, load_external_data=False).graph.input[0].type.tensor_type.shape.dim
    return d[2].dim_value


def session(path):
    so = ort.SessionOptions()
    # Level3 is mandatory for a quantized model: QDQ only fuses at Level2+.
    so.graph_optimization_level = ort.GraphOptimizationLevel.ORT_ENABLE_ALL
    so.intra_op_num_threads = 1
    so.enable_mem_pattern = False
    return ort.InferenceSession(path, so, providers=["CPUExecutionProvider"])


def throughput(sess, side, total=20):
    xs = [np.random.randn(1, 3, side, side).astype(np.float32) for _ in range(WORKERS)]
    sess.run(None, {"input_image": xs[0]})
    done, lock = [0], threading.Lock()

    def work(i):
        while True:
            with lock:
                if done[0] >= total:
                    return
                done[0] += 1
            sess.run(None, {"input_image": xs[i]})

    t0 = time.perf_counter()
    threads = [threading.Thread(target=work, args=(i,)) for i in range(WORKERS)]
    for t in threads:
        t.start()
    for t in threads:
        t.join()
    return total / (time.perf_counter() - t0)


def masks(sess, side, paths):
    out = []
    for p in paths:
        logits = sess.run(None, {"input_image": preprocess(p, side)})[0][0, 0]
        m = Image.fromarray(((logits > 0.0) * 255).astype(np.uint8))
        out.append(np.asarray(m.resize((COMMON, COMMON), Image.Resampling.NEAREST)) > 127)
    return out


def main():
    if len(sys.argv) < 3:
        sys.exit("usage: compare_models.py REFERENCE.onnx CANDIDATE.onnx [MORE.onnx ...]")
    models = sys.argv[1:]
    paths = sorted(glob.glob(os.path.join(IMAGE_DIR, "*.jpg")))
    if not paths:
        sys.exit(f"no images in {IMAGE_DIR!r} - run from the repo root")

    print(f"{len(paths)} images, {WORKERS} concurrent workers\n")
    print(f"{'model':<34}{'MB':>7}{'img/s':>9}{'speedup':>9}{'meanIoU':>9}{'worstIoU':>10}")
    print("-" * 78)

    ref, base = None, None
    for path in models:
        side = model_side(path)
        sess = session(path)
        rate = throughput(sess, side)
        got = masks(sess, side, paths)
        del sess

        if ref is None:
            ref, base = got, rate
            mean = worst = 1.0
        else:
            ious = [(a & b).sum() / max((a | b).sum(), 1) for a, b in zip(ref, got)]
            mean, worst = float(np.mean(ious)), float(np.min(ious))
        speedup = rate / base if base else 1.0
        print(f"{os.path.basename(path):<34}{os.path.getsize(path) / 2**20:>7.1f}"
              f"{rate:>9.2f}{speedup:>8.2f}x{mean:>9.4f}{worst:>10.4f}")


if __name__ == "__main__":
    main()
