extern crate vergen_git2;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let git = vergen_git2::Git2Builder::all_git()?;
    vergen_git2::Emitter::default()
        .add_instructions(&git)?
        .emit()?;
    Ok(())
}
