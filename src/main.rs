use color_eyre::eyre::{Report, Result, WrapErr};
use jiff::civil::{Date, DateTime};
use regex::Regex;
use std::{
    collections::HashMap,
    sync::atomic::{AtomicBool, Ordering},
};
use ureq::http::Uri;

static VERBOSE: AtomicBool = AtomicBool::new(false);

/// Find assets on Immich which have a date in their file name.
#[derive(argh::FromArgs)]
struct Args {
    /// API key providing access to Immich's API. It requires permissions 'asset.read' and 'asset.update'.
    #[argh(option)]
    api_key: String,

    /// URL of the immich API
    #[argh(option)]
    host: String,

    /// set the creation date of an asset.
    #[argh(switch)]
    apply: bool,

    /// explain what is being done
    #[argh(switch, short = 'v')]
    verbose: bool,
}

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

    let re = Regex::new(r"(\d{8})(?:[_-](\d{6}))?").unwrap();
    for year in 2000..2027 {
        let assets = get_assets_by_filename(&format!("{year}"), &client)?;
        debug(format!(
            "Found {} assets that have {} in their file name.",
            assets.len(),
            year
        ));

        for asset in assets {
            let Some(datetime) = extract_datetime_from_asset(&asset, &re) else {
                debug(format!(
                    "Skipping {}, file name does not include a date(time).",
                    asset.original_file_name
                ));
                continue;
            };

            // Sometimes the needle doesn't match for the year, but for other parts of the timestamp.
            // For example, the needle "2002" matches the time 20:02 in this file name "201704220-2002.jpg".
            // We'll skip then.
            if datetime.year() != year as i16 {
                continue;
            }

            let asset = get_asset_by_id(&asset.id, &client)?;

            if let Some(exif_info) = asset.exif_info
                && let Some(original_date_time) = exif_info.date_time_original
            {
                let original_date_time = original_date_time.parse::<DateTime>().ok();
                if let Some(original_date_time) = &original_date_time {
                    if (*original_date_time - datetime).get_days() <= 1 {
                        continue;
                    };
                    debug(format!(
                        "Date of {} will be updated from {:?} to {:?}",
                        asset.original_file_name, original_date_time, datetime,
                    ));
                }
            }

            if args.apply {
                let _ = set_assets_date_time(&asset.id, &datetime, &client)?;
                println!("Date of {} set to {:?}", asset.original_file_name, datetime,);
            } else {
                println!(
                    "Date of {} will be set to {:?}. Run script with --apply to apply the change.",
                    asset.original_file_name, datetime,
                );
            }
        }
    }

    Ok(())
}

fn extract_datetime_from_asset(asset: &Asset, re: &Regex) -> Option<DateTime> {
    let caps = re.captures(&asset.original_file_name)?;

    let date: Date = caps
        .get(1)?
        .as_str()
        .parse()
        .inspect_err(|err| {
            debug(format!(
                "Failed to parse the match '{}' as a date: {err:?}",
                caps.get(1).unwrap().as_str()
            ))
        })
        .ok()?;

    // If file name doesn't include a time, default to midnight.
    let Some(time) = caps.get(2) else {
        return Some(date.at(0, 0, 0, 0));
    };

    let time = time.as_str();

    // These lookups should be safe, since the regex matches on exactly 6 characters.
    let hour: i8 = time[0..2].parse().ok()?;
    let minutes: i8 = time[2..4].parse().ok()?;
    let seconds: i8 = time[4..6].parse().ok()?;
    Some(date.at(hour, minutes, seconds, 0))
}

#[derive(serde::Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct Read {
    assets: Results,
}

#[derive(serde::Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct Results {
    #[serde(alias = "assets")]
    items: Vec<Asset>,
}

#[derive(serde::Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct Asset {
    id: String,
    original_file_name: String,
    exif_info: Option<ExifInfo>,
}

#[derive(serde::Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
struct ExifInfo {
    date_time_original: Option<String>,
}

#[derive(Debug)]
struct Client {
    api_key: String,
    host: Uri,
}

fn get_assets_by_filename(file_name: &str, client: &Client) -> Result<Vec<Asset>> {
    let url = format!("{}/search/metadata", client.host);

    let body: Read = ureq::post(&url)
        .header("x-api-key", &client.api_key)
        .send_json(HashMap::from([("originalFileName", file_name)]))
        .map_err(|error| explain_ureq_error(error, &url))?
        .body_mut()
        .read_json()
        .map_err(|error| explain_ureq_error(error, &url))?;

    Ok(body.assets.items)
}

fn get_asset_by_id(id: &str, client: &Client) -> Result<Asset> {
    let url = format!("{}/assets/{}", client.host, id);

    let asset: Asset = ureq::get(&url)
        .header("x-api-key", &client.api_key)
        .call()
        .map_err(|error| explain_ureq_error(error, &url))?
        .body_mut()
        .read_json()
        .map_err(|error| explain_ureq_error(error, &url))?;

    Ok(asset)
}

fn set_assets_date_time(id: &str, datetime: &DateTime, client: &Client) -> Result<Asset> {
    let url = format!("{}/assets/{}", client.host, id);

    let asset = ureq::put(&url)
        .header("x-api-key", &client.api_key)
        .send_json(HashMap::from([(
            "dateTimeOriginal",
            format!("{}", datetime),
        )]))
        .map_err(|error| explain_ureq_error(error, &url))?
        .body_mut()
        .read_json()
        .map_err(|error| explain_ureq_error(error, &url))?;

    Ok(asset)
}

// Add some context errors that occur when interacting with Immich's HTTP API.
fn explain_ureq_error(error: ureq::Error, url: &str) -> Report {
    let explanation = match error {
        ureq::Error::StatusCode(401) => "not authorized, please verify the API key",
        ureq::Error::StatusCode(403) => {
            "the API key doesn't have correct permissions, make sure to enable the permissions asset.read and asset.update"
        }
        ureq::Error::StatusCode(404) => "are you sure the location of the Immich API is correct?",
        ureq::Error::Json(_) => "failed to parse the JSON received from endpoint",
        _ => return Report::new(error).wrap_err(format!("interaction with {url} failed")),
    };

    Report::new(error).wrap_err(format!("interaction with {url} failed: {explanation}"))
}

fn debug(message: String) {
    if VERBOSE.load(Ordering::Relaxed) {
        println!("{message}");
    }
}

#[cfg(test)]
mod test {
    #[test]
    fn test() {
        let input = r#"{
  "albums": {
    "count": 0,
    "facets": [],
    "items": [],
    "total": 0
  },
  "assets": {
    "count": 1,
    "facets": [],
    "items": [
      {
        "checksum": "R2FK8qQmyVoNLRshw/HBf2eimWY=",
        "createdAt": "2025-12-08T15:51:25.852Z",
        "deviceAssetId": "Screenshot_20220521-213034_DBNavigator.jpg-597069",
        "deviceId": "CLI",
        "duplicateId": "f0553e4d-90b2-469d-a8cf-652295204758",
        "duration": "0:00:00.00000",
        "fileCreatedAt": "2022-05-21T19:30:34.743Z",
        "fileModifiedAt": "2024-11-09T18:27:01.000Z",
        "hasMetadata": true,
        "height": 2400,
        "id": "8b9b29ed-5857-4a24-bcfb-c49dc2aef5a2",
        "isArchived": false,
        "isEdited": false,
        "isFavorite": false,
        "isOffline": false,
        "isTrashed": false,
        "libraryId": null,
        "livePhotoVideoId": null,
        "localDateTime": "2022-05-21T21:30:34.743Z",
        "originalFileName": "Screenshot_20220521-213034_DB Navigator.jpg",
        "originalMimeType": "image/jpeg",
        "originalPath": "/data/upload/d56371b5-41f5-4588-86ee-65d182fef53d/50/68/50684a19-a9b7-4d4d-a8ba-8f04e3928d64.jpg",
        "ownerId": "d56371b5-41f5-4588-86ee-65d182fef53d",
        "people": [],
        "resized": true,
        "thumbhash": "LRgSCwSniHh3f4NZaWCeBuY=",
        "type": "IMAGE",
        "updatedAt": "2026-03-20T07:29:25.349Z",
        "visibility": "timeline",
        "width": 1080
      }
    ],
    "nextPage": null,
    "total": 1
  }
}"#;
        let _: super::Read = serde_json::from_str(input).unwrap();
    }
}
