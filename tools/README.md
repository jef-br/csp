# tools

The model pipeline, in order. Run every script **from the repo root** - they
resolve `JB core img batch/` and the `.onnx` files relative to it.

| Script | Does |
|---|---|
| `1. run_inference_stages.py` | Dumps one image's four stages (normalised input, raw logits, sigmoid, thresholded) as jpgs. Reads the side from the model; `BIREFNET_ONNX` picks which one. |
| `2. export_onnx.py` | Exports `ZhengPeng7/BiRefNet_lite` from Hugging Face at a fixed square side. Opset 20, so deformable convs stay a single native `DeformConv`. |
| `3. quantize_int8.py` | Static int8 quantization, calibrated on the garment set. The settings in it are load-bearing and documented in its docstring. |
| `4. compare_models.py` | A/B any two exports: throughput under CSP's real concurrency model, plus mask IoU against the first. |

## Rebuilding the shipped model from scratch

```
python "tools/2. export_onnx.py" 384
python "tools/3. quantize_int8.py" birefnet_lite_384.onnx
python "tools/4. compare_models.py" birefnet_lite_512.onnx birefnet_lite_384_int8.onnx
```

Then point `MODEL_FILE` in `build.rs` at the result. The `.onnx` files are
gitignored - they are build inputs, not source.

## Three things that are easy to get wrong

- **`activation_type=QUInt8`, not `QInt8`.** Signed activations give 1.05x;
  unsigned give 1.20x at identical accuracy, because U8S8 is the fast MLAS
  integer path on x86.
- **`Gemm` must be in `op_types_to_quantize`.** The preprocessing pass fuses
  `MatMul+Add` into `Gemm`, so a list of just `Conv, MatMul` silently skips 96
  projection layers - a quarter of runtime.
- **Nothing else belongs in that list.** Adding `LayerNormalization`/`Softmax`,
  or `Relu`/`Add`/`Concat`, collapses the mask to IoU 0.65 with some outputs
  fully empty, for under 5% speed.

And on the consumer side: the session must run at
`GraphOptimizationLevel::Level3`. QDQ triplets only fuse into real int8 kernels
at Level2 and above; at Level1 a quantized model is *slower* than fp32.
