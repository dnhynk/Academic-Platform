//! Native shell entry point; enabled only in the desktop runtime lane.
fn main() -> Result<(), Box<dyn std::error::Error>> {
    academic_desktop::runtime::run()?;
    Ok(())
}
