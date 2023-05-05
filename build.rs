use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    prost_build::compile_protos(&["src/for_flutter.proto"], &["src/"])?;

    Ok(())
}
