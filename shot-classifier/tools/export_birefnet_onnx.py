"""
Export ZhengPeng7/BiRefNet_lite to ONNX for csp_shot_classifier.

    python tools/export_birefnet_onnx.py [SIDE]

SIDE is the square input resolution (default 512). 256 runs in ~0.4 GB
but is too coarse for pale, soft-edged garments on white (the model
returns an empty mask); 512 (~1.5 GB) handles them; 1024 is sharper still
and needs several GB.

Requires: torch, transformers, onnx  (pip install torch transformers onnx)

Opset 20 is used so the decoder's deformable convolutions export as a
single native `DeformConv` op. That needs ONNX Runtime >= ~1.25 at
inference (the bundled onnxruntime.dll is 1.25.0); an older opset makes
torch decompose them into a GatherND graph that blows up memory.
"""
import sys
import torch
from transformers import AutoModelForImageSegmentation

SIDE = int(sys.argv[1]) if len(sys.argv) > 1 else 512
MODEL_ID = "ZhengPeng7/BiRefNet_lite"
OUT = f"birefnet_lite_{SIDE}.onnx"

print(f"loading {MODEL_ID} ...", flush=True)
model = AutoModelForImageSegmentation.from_pretrained(MODEL_ID, trust_remote_code=True)
model = model.float().eval()

dummy = torch.randn(1, 3, SIDE, SIDE, dtype=torch.float32)
print(f"exporting {OUT} at {SIDE}x{SIDE} ...", flush=True)
with torch.no_grad():
    torch.onnx.export(
        model, (dummy,), OUT,
        input_names=["input_image"], output_names=["output_image"],
        opset_version=20,
    )

# torch writes weights to a sidecar .onnx.data file; fold them back in so
# the model is a single portable file.
import onnx
m = onnx.load(OUT)
onnx.save_model(m, OUT, save_as_external_data=False)
print("done:", OUT, flush=True)
