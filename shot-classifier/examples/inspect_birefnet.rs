//! Prints the BiRefNet ONNX model's input/output tensor names and shapes.
//!
//! Paths (override via env):
//!   ORT_DYLIB_PATH   ONNX Runtime shared library (default: ./onnxruntime.dll)
//!   BIREFNET_ONNX    BiRefNet model (default: ./birefnet_lite.onnx)

use ort::session::Session;

fn main() -> ort::Result<()> {
    let ort_lib = std::env::var("ORT_DYLIB_PATH").unwrap_or_else(|_| "onnxruntime.dll".into());
    let model_path = std::env::var("BIREFNET_ONNX").unwrap_or_else(|_| "birefnet_lite.onnx".into());

    ort::init_from(ort_lib).commit()?;
    let session = Session::builder()?.commit_from_file(model_path)?;
    println!("Inputs:");
    for input in session.inputs.iter() {
        println!("  {} : {:?}", input.name, input.input_type);
    }
    println!("Outputs:");
    for output in session.outputs.iter() {
        println!("  {} : {:?}", output.name, output.output_type);
    }
    Ok(())
}
