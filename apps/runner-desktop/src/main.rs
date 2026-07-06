use anyhow::Result;
use baudbound_core::RunnerCore;

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let core = RunnerCore::default();
    println!("{} desktop shell scaffold", core.name);
    println!("Desktop-only tray and approval UI will call shared runner core services.");

    Ok(())
}
