# run_inference_stages.py
import os
import sys
import numpy as np
from PIL import Image
import onnxruntime as ort

MODEL_PATH = "birefnet_lite.onnx"
RES = 1024
MEAN = np.array([0.485, 0.456, 0.406], dtype=np.float32)
STD = np.array([0.229, 0.224, 0.225], dtype=np.float32)

# pick the image to run: pass a path as argv[1], or defaults to the first jpg in CiMini
if len(sys.argv) > 1:
    IMG_PATH = sys.argv[1]
else:
    CALIB_DIR = r"C:\Users\JefB\Documents\JBGITROOT\prism\test\datasets\CiMini"
    first_jpg = next(f for f in os.listdir(CALIB_DIR) if f.lower().endswith((".jpg", ".jpeg")))
    IMG_PATH = os.path.join(CALIB_DIR, first_jpg)

base_name = os.path.splitext(os.path.basename(IMG_PATH))[0]
out_dir = os.path.dirname(os.path.abspath(IMG_PATH)) if len(sys.argv) > 1 else os.getcwd()

def save_stage(arr_2d_0to255, suffix):
    img = Image.fromarray(arr_2d_0to255.astype(np.uint8))
    path = os.path.join(os.getcwd(), f"{base_name}--{suffix}.jpg")
    img.save(path)
    print(f"saved {path}")

print(f"Loading: {IMG_PATH}")
img = Image.open(IMG_PATH).convert("RGB").resize((RES, RES))
img_arr = np.array(img, dtype=np.float32) / 255.0  # HWC, 0-1

# --- stage 1: normalized ---
normalized = (img_arr - MEAN) / STD  # HWC, roughly -2..+2, NOT a displayable image as-is
# rescale to 0-255 purely for visualization (min-max stretch)
norm_vis = normalized - normalized.min()
norm_vis = (norm_vis / norm_vis.max()) * 255.0
save_stage(norm_vis.mean(axis=2), "1--normalized")  # mean across channels for a single-image view

# prep model input: NCHW
model_input = normalized.transpose(2, 0, 1)[None, :, :, :].astype(np.float32)

# --- stage 2: forward pass (raw logits) ---
print("Running inference...")
session = ort.InferenceSession(MODEL_PATH, providers=["CPUExecutionProvider"])
input_name = session.get_inputs()[0].name
output_name = session.get_outputs()[0].name
raw_output = session.run([output_name], {input_name: model_input})[0]
logits = raw_output[0, 0]  # HxW, raw values, NOT 0-255

logits_vis = logits - logits.min()
logits_vis = (logits_vis / logits_vis.max()) * 255.0
save_stage(logits_vis, "2--forward_pass")

# --- stage 3: sigmoid ---
sigmoid = 1.0 / (1.0 + np.exp(-logits))  # HxW, 0..1
save_stage(sigmoid * 255.0, "3--sigmoid")

# --- stage 4: thresholded ---
threshold = 0.5
mask = (sigmoid > threshold).astype(np.float32) * 255.0
save_stage(mask, "4--thresholded")

print("Done.")