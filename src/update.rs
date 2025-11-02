use self_update::cargo_crate_version;
use std::error::Error;

/// Runs the self-update process to update dtop to the latest version
pub fn run_update() -> Result<(), Box<dyn Error>> {
    println!("Checking for updates...");

    let status = self_update::backends::github::Update::configure()
        .repo_owner("amir20")
        .repo_name("dtop")
        .bin_name("dtop")
        .show_download_progress(true)
        .current_version(cargo_crate_version!())
        .build()?
        .update()?;

    match status {
        self_update::Status::UpToDate(version) => {
            println!("Already up to date (v{})", version);
        }
        self_update::Status::Updated(version) => {
            println!("Successfully updated to v{}", version);
            println!("Please restart dtop to use the new version.");
        }
    }

    Ok(())
}
