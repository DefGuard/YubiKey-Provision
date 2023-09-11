fn main() -> Result<(), Box<dyn std::error::Error>> {
    // compiling protos using path on build time
    let config = prost_build::Config::new();
    tonic_build::configure().compile_with_config(
        config,
        &["proto/worker/worker.proto"],
        &["proto/worker"],
    )?;
    Ok(())
}
