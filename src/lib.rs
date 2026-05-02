use color_eyre::eyre::{Report, Result};
use jiff::{
    Zoned,
    civil::{Date, DateTime},
};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    ops::RangeInclusive,
    sync::atomic::{AtomicBool, Ordering},
};
use ureq::http::Uri;

pub static VERBOSE: AtomicBool = AtomicBool::new(false);

/// Retrodate is a utility to retroactively date images on Immich based on an image's filename.
///
/// This tool queries Immich for all filenames that contain a year, e.g. 2021 in 20210901_115950.jpg.
/// Then, the date (and optional time) is parsed from the filename.
///
/// You can control search using the arguments --from-year and --until-year.
#[derive(argh::FromArgs)]
pub struct Args {
    /// API key providing access to Immich's API. It requires permissions 'asset.read' and 'asset.update'.
    #[argh(option)]
    pub api_key: String,

    /// URL of the immich API
    #[argh(option)]
    pub host: String,

    /// set the creation date of an asset.
    #[argh(switch)]
    pub apply: bool,

    /// explain what is being done
    #[argh(switch, short = 'v')]
    pub verbose: bool,

    /// the earliest year to include in the search, defaults to 2000
    #[argh(option, default = "2000")]
    pub from_year: u16,

    /// the latest year to include in the search, defaults to the current year
    #[argh(option, default = "current_year()")]
    pub until_year: u16,
}

pub struct App {
    client: Client,
    apply: bool,

    year_range: RangeInclusive<u16>,
}

impl App {
    pub fn new(client: Client, from_year: u16, until_year: u16) -> Self {
        Self {
            client,
            apply: false,
            year_range: from_year..=until_year,
        }
    }

    pub fn apply_changes(mut self) -> Self {
        self.apply = true;
        self
    }

    pub fn run(self) -> Result<()> {
        let re = Regex::new(r"(\d{8})(?:[_-](\d{6}))?").unwrap();
        for year in self.year_range {
            let assets = get_assets_by_filename(&format!("{year}"), &self.client)?;
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

                let asset = get_asset_by_id(&asset.id, &self.client)?;

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

                if self.apply {
                    let _ = set_assets_date_time(&asset.id, &datetime, &self.client)?;
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
}

#[derive(Debug, Clone)]
pub struct Client {
    pub api_key: String,
    pub host: Uri,
}

fn get_assets_by_filename(file_name: &str, client: &Client) -> Result<Vec<Asset>> {
    let url = format!("{}/search/metadata", client.host);

    let body: Search = ureq::post(&url)
        .header("x-api-key", &client.api_key)
        .config()
        .build()
        .send_json(HashMap::from([("originalFileName", file_name)]))
        .map_err(|error| explain_ureq_error(error, &url))?
        .body_mut()
        .read_json()
        .map_err(|error| explain_ureq_error(error, &url))?;

    Ok(body.assets.items)
}

pub fn get_asset_by_id(id: &str, client: &Client) -> Result<Asset> {
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
        .send_json(Patch {
            date_time_original: datetime.to_string(),
        })
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

#[derive(Deserialize, Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Search {
    pub assets: Results,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Results {
    #[serde(alias = "assets")]
    pub items: Vec<Asset>,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Asset {
    pub id: String,
    pub original_file_name: String,
    pub exif_info: Option<ExifInfo>,
}

#[derive(Deserialize, Serialize, Debug, Clone, Default, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "camelCase")]
pub struct ExifInfo {
    pub date_time_original: Option<String>,
}
#[derive(Deserialize, Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Query {
    pub original_file_name: String,
}

#[derive(Deserialize, Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Patch {
    pub date_time_original: String,
}

// Return the current year.
fn current_year() -> u16 {
    Zoned::now().year().try_into().unwrap_or(2030)
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
        let _: super::Search = serde_json::from_str(input).unwrap();
    }
}
