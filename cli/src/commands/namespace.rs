use eld_client::facade::ChainClient;
use eld_common::error::EldError;

pub(crate) async fn get_namespace(
    cli: &ChainClient,
    namespace_slug: String,
) -> Result<(), EldError> {
    let lookup = cli.get_namespace(namespace_slug).await?;
    crate::output::print_result(&crate::output::namespace_lookup(&lookup));
    Ok(())
}

pub(crate) async fn add_namespace(
    cli: &ChainClient,
    wallet_name: String,
    namespace_slug: String,
    registration_fee: u128,
    dry_run: bool,
) -> Result<(), EldError> {
    if dry_run {
        crate::output::print_result(&crate::output::dry_run_add_namespace(
            &wallet_name,
            &namespace_slug,
            registration_fee,
        ));
        return Ok(());
    }
    let resp = cli
        .add_namespace(wallet_name, namespace_slug, registration_fee)
        .await?;
    crate::output::print_result(&crate::output::print_registered(&resp));
    Ok(())
}
