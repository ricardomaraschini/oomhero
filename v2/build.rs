extern crate vergen;

use vergen::EmitBuilder;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    EmitBuilder::builder()
        .git_sha(true)
        .git_dirty(true)
        .git_commit_date()
        .emit()?;
    Ok(())
}
