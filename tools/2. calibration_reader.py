# calibration_reader.py
import os
import numpy as np
from PIL import Image
from onnxruntime.quantization import CalibrationDataReader

CALIB_DIR = r"C:\Users\JefB\Documents\JBGITROOT\prism\test\datasets\CiMini"
RES = 1024
MEAN = np.array([0.485, 0.456, 0.406], dtype=np.float32)
STD = np.array([0.229, 0.224, 0.225], dtype=np.float32)

def preprocess(path):
    img = Image.open(path).convert("RGB").resize((RES, RES))
    arr = np.array(img, dtype=np.float32) / 255.0
    arr = (arr - MEAN) / STD
    arr = arr.transpose(2, 0, 1)[None, :, :, :]  # NCHW
    return arr.astype(np.float32)

class BiRefNetCalibrationReader(CalibrationDataReader):
    def __init__(self, calib_dir=CALIB_DIR, max_images=5):
        files = [f for f in os.listdir(calib_dir) if f.lower().endswith((".jpg", ".jpeg"))]
        files = files[:max_images]
        self.paths = [os.path.join(calib_dir, f) for f in files]
        self._iter = iter(self.paths)
        print(f"Calibration set: {len(self.paths)} images")

    def get_next(self):
        path = next(self._iter, None)
        if path is None:
            return None
        return {"input_image": preprocess(path)}



        