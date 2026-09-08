# quantize_static_int8.py
from onnxruntime.quantization import quantize_static, QuantType, QuantFormat
from calibration_reader import BiRefNetCalibrationReader

quantize_static(
    model_input="birefnet_lite.onnx",
    model_output="birefnet_lite_int8.onnx",
    calibration_data_reader=BiRefNetCalibrationReader(),
    quant_format=QuantFormat.QDQ,
    weight_type=QuantType.QInt8,
    activation_type=QuantType.QInt8,
)