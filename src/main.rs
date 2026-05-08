use color_eyre::eyre::{Result, WrapErr};
use jiff::Span;
use retrodate::{App, Args, VERBOSE};
use std::{sync::atomic::Ordering, time::Duration};

fn main() -> Result<()> {
    color_eyre::install()?;
    let mut args: Args = argh::from_env();
    VERBOSE.store(args.verbose, Ordering::Relaxed);

    let host =  args.host.parse().wrap_err_with(|| {
            format!("failed to parse URL of the Immich HTTP API '{}', it should have the format of http://192.168.178.1:2283/api or https://example.com/api", args.host)
        })?;

    // Be forgiving if a user swapped the values from --from-year and --until-year.
    if args.from_year > args.until_year {
        std::mem::swap(&mut args.from_year, &mut args.until_year);
    }

    let mut builder = App::builder(host, args.api_key)
        .from_year(args.from_year)
        .until_year(args.until_year)
        .http_timeout(Duration::from_secs(args.timeout));

    if args.if_unset {
        builder = builder.if_unset();
    }

    let threshold = args
        .threshold
        .map(|value| value.parse::<Span>())
        .transpose()
        .unwrap();

    let builder = match (args.overwrite, threshold) {
        (false, _) => builder,
        (true, None) => builder.overwrite(Span::new()),
        (true, Some(interval)) => builder.overwrite(interval),
    };

    let app = builder.build();
    app.run()
}
