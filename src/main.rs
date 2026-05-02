use color_eyre::eyre::{Result, WrapErr};
use retrodate::{App, Args, Client, VERBOSE};
use std::sync::atomic::Ordering;

fn main() -> Result<()> {
    color_eyre::install()?;
    let args: Args = argh::from_env();
    VERBOSE.store(args.verbose, Ordering::Relaxed);

    let client = Client {
        host: args.host.parse().wrap_err_with(|| {
            format!("failed to parse URL of the Immich HTTP API '{}', it should have the format of http://192.168.178.1:2283/api or https://example.com/api", args.host)
        })?,
        api_key: args.api_key,
    };
    let mut app = App::new(client, args.from_year, args.until_year as u16);
    if args.apply {
        app = app.apply_changes();
    }

    app.run()
}
