use color_eyre::eyre::{Report, Result};
use jiff::{
    Span, Zoned,
    civil::{Date, DateTime, Time},
};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    ops::RangeInclusive,
    sync::{
        LazyLock,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};
use ureq::http::Uri;

pub static VERBOSE: AtomicBool = AtomicBool::new(false);
static DATE_RE: LazyLock<Vec<Regex>> = LazyLock::new(|| {
    vec![
        //  * YYYYMMDD
        Regex::new(r#"(?<year>\d{4})(?<month>\d{2})(?<day>\d{2})"#)
            .expect("This shouldn't panic at runtime."),
        // YYYY-MM-DD
        Regex::new(r#"(?<year>\d{4})[[:punct:]]{1}(?<month>\d{1,2})[[:punct:]]{1}(?<day>\d{1,2})"#)
            .expect("This shouldn't panic at runtime."),
        //  * DDMMYYYY
        Regex::new(r#"(?<day>\d{2})(?<month>\d{2})(?<year>\d{4})"#)
            .expect("This shouldn't panic at runtime."),
        //  * DD-MM-YYYY
        Regex::new(r#"(?<day>\d{1,2})[[:punct:]]{1}(?<month>\d{1,2})[[:punct:]]{1}(?<year>\d{4})"#)
            .expect("This shouldn't panic at runtime."),
        //  * YYYYDDMM
        Regex::new(r#"(?<year>\d{4})(?<day>\d{2})(?<month>\d{2})"#)
            .expect("This shouldn't panic at runtime."),
        //  * YYYY-DD-MM
        Regex::new(r#"(?<year>\d{4})[[:punct:]]{1}(?<day>\d{1,2})[[:punct:]]{1}(?<month>\d{1,2})"#)
            .expect("This shouldn't panic at runtime."),
        //  * MMDDYYYY
        Regex::new(r#"(?<month>\d{1,2})(?<day>\d{1,2})(?<year>\d{4})"#)
            .expect("This shouldn't panic at runtime."),
        //  * MM-DD-YYYY
        Regex::new(r#"(?<month>\d{2})[[:punct:]]{1}(?<day>\d{2})[[:punct:]]{1}(?<year>\d{4})"#)
            .expect("This shouldn't panic at runtime."),
    ]
});

static TIME_RE: LazyLock<Vec<Regex>> = LazyLock::new(|| {
    vec![
        //  HHMMSS
        Regex::new(r#"(?<hour>\d{2})(?<minute>\d{2})(?<second>\d{2})"#)
            .expect("This shouldn't panic at runtime."),
        // HH-MM-SS
        Regex::new(r#"(?<hour>\d{2})[[:punct:]]{1}(?<minute>\d{2})[[:punct:]]{1}(?<second>\d{2})"#)
            .expect("This shouldn't panic at runtime."),
    ]
});

static SKIP_RE: LazyLock<Vec<Regex>> = LazyLock::new(|| {
    vec![
        Regex::new(
            r#"[[:alnum:]]{8}-[[:alnum:]]{4}-[[:alnum:]]{4}-[[:alnum:]]{4}-[[:alnum:]]{12}"#,
        )
        .expect("This shouldn't panic at runtime."),
    ]
});

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

    /// set the creation date of an asset, only if that asset doesn't have a creation date already.
    #[argh(switch)]
    pub if_unset: bool,

    /// set the creation date of an asset, even if that asset has a creation date already. It sets --if-unset.
    #[argh(switch)]
    pub overwrite: bool,

    /// maximum allowed time difference when deciding to update an asset's creation date.
    /// Provide a `jiff::Span` string such as `30m`, `1h`, `2d`, or `1h30m`.
    #[argh(option)]
    pub threshold: Option<String>,

    /// explain what is being done
    #[argh(switch, short = 'v')]
    pub verbose: bool,

    /// the earliest year to include in the search, defaults to 2000
    #[argh(option, default = "2000")]
    pub from_year: u16,

    /// the latest year to include in the search, defaults to the current year
    #[argh(option, default = "current_year()")]
    pub until_year: u16,

    /// the maximum time in seconds for a single HTTP request against Immich's HTTP API
    #[argh(option, default = "5")]
    pub timeout: u64,
}

pub struct App {
    client: Client,
    mode: Mode,

    year_range: RangeInclusive<u16>,
}

impl App {
    pub fn builder(host: Uri, api_key: String) -> Builder {
        Builder::new(host, api_key)
    }

    pub fn run(self) -> Result<()> {
        for year in self.year_range {
            let assets = get_assets_by_filename(&format!("{year}"), &self.client)?;
            debug(format!(
                "Found {} assets that have {} in their file name.",
                assets.len(),
                year
            ));

            for asset in assets {
                let Some(datetime) = extract_datetime(&asset.original_file_name) else {
                    debug(format!(
                        "Skipping {}, file name does not include a date.",
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

                let maybe_date_time_original = asset
                    .exif_info
                    .and_then(|exif_info| {
                        exif_info.date_time_original.map(|datetime| {
                            datetime
                                .parse::<DateTime>()
                                .inspect_err(|err| {
                                    eprintln!(
                                        "Failed to extract the datetime from {}: {err:?}",
                                        asset.original_file_name
                                    )
                                })
                                .ok()
                        })
                    })
                    .flatten();

                match (self.mode, maybe_date_time_original) {
                    (Mode::DryRun, None) => {
                        println!(
                            "Date of {} will be set to {}. Call `retrodate` with --if-unset to apply the change.",
                            asset.original_file_name, datetime,
                        );
                    }
                    (Mode::DryRun, Some(date_time_original)) => {
                        println!(
                            "Date of {} will be changed from {} to {}. Call `retrodate` with --overwrite to apply the change.",
                            asset.original_file_name, date_time_original, datetime,
                        );
                    }
                    (Mode::IfUnset, Some(date_time_original)) => {
                        debug(format!(
                            "Skipping {}, the asset has datetime set {}. Call `retrodate` with --overwrite to replace the existing datetime.",
                            asset.original_file_name, date_time_original
                        ));

                        continue;
                    }
                    (Mode::IfUnset | Mode::Overwrite(_), None) => {
                        let _ = set_assets_date_time(&asset.id, &datetime, &self.client)?;
                        debug(format!(
                            "Date of {} set to {}.",
                            asset.original_file_name, datetime
                        ));
                    }
                    (Mode::Overwrite(interval), Some(date_time_original)) => {
                        if (datetime - date_time_original)
                            .abs()
                            .compare((interval, date_time_original))
                            .unwrap()
                            == core::cmp::Ordering::Greater
                        {
                            let _ = set_assets_date_time(&asset.id, &datetime, &self.client)?;
                            debug(format!(
                                "Change date of {} from {} to {}.",
                                asset.original_file_name, date_time_original, datetime
                            ));
                        } else {
                            debug(format!(
                                "Skipping {}, the time difference between the existing datetime ({}) and the datetime extracted from the filename ({}) doesn't surpass the threshold of {:?}",
                                asset.original_file_name, date_time_original, datetime, interval
                            ));
                            continue;
                        }
                    }
                }
            }
        }
        Ok(())
    }
}

#[derive(Debug, Default, Copy, Clone)]
pub enum Mode {
    // Do not mutate assets.
    #[default]
    DryRun,

    // Only mutate assets that are lacking a creation date.
    IfUnset,

    // Mutate all assets, both with and without creation date.
    Overwrite(Span),
}

#[derive(Debug)]
pub struct Builder {
    client: Client,
    from_year: u16,
    until_year: u16,

    mode: Mode,
}

impl Builder {
    pub fn new(host: Uri, api_key: String) -> Self {
        Builder {
            client: Client {
                host,
                api_key,
                timeout: Duration::from_secs(5),
            },
            from_year: 2000,
            until_year: current_year(),
            mode: Mode::DryRun,
        }
    }

    pub fn dry_run(mut self) -> Self {
        self.mode = Mode::DryRun;
        self
    }

    pub fn if_unset(mut self) -> Self {
        self.mode = Mode::IfUnset;
        self
    }

    pub fn overwrite(mut self, interval: Span) -> Self {
        self.mode = Mode::Overwrite(interval);
        self
    }

    pub fn from_year(mut self, year: u16) -> Self {
        self.from_year = year;
        self
    }

    pub fn until_year(mut self, year: u16) -> Self {
        self.until_year = year;
        self
    }

    pub fn http_timeout(mut self, timeout: Duration) -> Self {
        self.client.timeout = timeout;
        self
    }

    pub fn build(self) -> App {
        App {
            client: self.client,
            mode: self.mode,
            year_range: self.from_year..=self.until_year,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Client {
    pub api_key: String,
    pub host: Uri,
    pub timeout: Duration,
}

fn get_assets_by_filename(file_name: &str, client: &Client) -> Result<Vec<Asset>> {
    let url = format!("{}/search/metadata", client.host);

    let mut assets: Vec<Asset> = Vec::new();
    let mut page = String::from("1");

    loop {
        let mut body: Search = ureq::post(&url)
            .header("x-api-key", &client.api_key)
            .config()
            .timeout_global(Some(client.timeout))
            .build()
            .send_json(HashMap::from([
                ("originalFileName", file_name),
                ("page", &page),
            ]))
            .map_err(|error| explain_ureq_error(error, &url))?
            .body_mut()
            .read_json()
            .map_err(|error| explain_ureq_error(error, &url))?;

        assets.append(&mut body.assets.items);
        let Some(next_page) = body.assets.next_page else {
            break;
        };
        page = next_page;
    }

    Ok(assets)
}

pub fn get_asset_by_id(id: &str, client: &Client) -> Result<Asset> {
    let url = format!("{}/assets/{}", client.host, id);

    let asset: Asset = ureq::get(&url)
        .header("x-api-key", &client.api_key)
        .config()
        .timeout_global(Some(client.timeout))
        .build()
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
        .config()
        .timeout_global(Some(client.timeout))
        .build()
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

#[derive(Debug)]
struct DateMatch {
    pub date: Date,
    pub start: usize,
    // Index where the match ends.
    pub end: usize,
}

#[derive(Debug)]
struct TimeMatch {
    pub time: Time,
    pub start: usize,
    // Index where the match ends.
    pub end: usize,
}

fn extract_datetime(file_name: &str) -> Option<DateTime> {
    let mut forbidden: Vec<(usize, usize)> = SKIP_RE
        .iter()
        .filter_map(|re| {
            let _match = re.captures(file_name)?.get(0)?;
            Some((_match.start(), _match.end()))
        })
        .collect();

    let date_match = extract_date(file_name, &forbidden[..])?;
    forbidden.push((date_match.start, date_match.end));

    let time_match = match extract_time(&file_name[date_match.end..], &forbidden[..]) {
        Some(time_match) => Some(time_match),
        None => extract_time(&file_name[..date_match.end], &forbidden),
    };

    let Some(time) = time_match else {
        debug(format!(
            "Extracted date, but failed to extract time from {file_name}, defaulting time to 00:00:00."
        ));
        return Some(date_match.date.at(0, 0, 0, 0));
    };
    Some(date_match.date.to_datetime(time.time))
}

fn extract_date(file_name: &str, forbidden: &[(usize, usize)]) -> Option<DateMatch> {
    if file_name.is_empty() {
        return None;
    }

    for re in &*DATE_RE {
        if let Some(date) = _extract_date(file_name, re) {
            // The first photograph was made in 1826 (or 1827, historians aren't quite sure).
            // I don't think anyone has older pictures on their Immich server.
            //
            // Continue and try to extract the date using a different regex.
            if date.date.year() < 1826 {
                continue;
            }

            if date.date.year() > 2100 {
                continue;
            }

            let overlap = forbidden.iter().any(|(start, end)| {
                if date.start >= *start && date.start <= *end {
                    return true;
                }

                if date.end >= *start && date.end <= *end {
                    return true;
                }
                false
            });
            if !overlap {
                return Some(date);
            }
        }
    }
    None
}

fn _extract_time(file_name: &str, re: &Regex) -> Option<TimeMatch> {
    let caps = re.captures(file_name)?;

    let hour: i8 = caps.name("hour")?.as_str().parse().ok()?;
    let minute: i8 = caps.name("minute")?.as_str().parse().ok()?;
    let second: i8 = caps.name("second")?.as_str().parse().ok()?;

    let time: Time = format!("{hour:02}:{minute:02}:{second:02}").parse().ok()?;

    Some(TimeMatch {
        time,
        start: caps.get(1)?.start(),
        end: caps.get(3)?.end(),
    })
}

fn extract_time(file_name: &str, forbidden: &[(usize, usize)]) -> Option<TimeMatch> {
    if file_name.is_empty() {
        return None;
    }
    println!("time input {}", file_name);

    for re in &*TIME_RE {
        if let Some(time) = _extract_time(file_name, re) {
            let overlap = forbidden.iter().any(|(start, end)| {
                if time.start >= *start && time.start <= *end {
                    return true;
                }

                if time.end >= *start && time.end <= *end {
                    return true;
                }
                false
            });
            if !overlap {
                return Some(time);
            }
        }
    }
    None
}

fn _extract_date(file_name: &str, re: &Regex) -> Option<DateMatch> {
    let caps = re.captures(file_name)?;

    let year: i16 = caps.name("year")?.as_str().parse().ok()?;
    let month: i8 = caps.name("month")?.as_str().parse().ok()?;
    let day: i8 = caps.name("day")?.as_str().parse().ok()?;

    let date: Date = format!("{year}-{month:02}-{day:02}").parse().ok()?;

    Some(DateMatch {
        date,
        start: caps.get(1)?.start(),
        // Whether the date is encoded as YYYY-MM-DD, DD-MM-YYYY or something else,
        // the 3rd capture group is the last group.
        end: caps.get(3)?.end(),
    })
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

    pub next_page: Option<String>,

    /// The total number of assets that match the query.
    pub total: usize,
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
    pub page: Option<String>,
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
    use crate::extract_datetime;
    use jiff::civil::DateTime;

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

    #[test]
    fn test_extracting_datetime() {
        let files: Vec<(&str, DateTime)> = vec![
            (
                "Screenshot_20230927_191315.jpg",
                "2023-09-27 19:13:15".parse().unwrap(),
            ),
            (
                "Screenshot_2023-09-27_191315.jpg",
                "2023-09-27 19:13:15".parse().unwrap(),
            ),
            (
                "Snapchat_2019-08-12_0516c0a9-5402-f3d7-ae27-19671102f52c-main.jpg",
                "2019-08-12 00:00:00".parse().unwrap(),
            ),
            (
                "Screenshot_2023-9-7_191315.jpg",
                "2023-09-07 19:13:15".parse().unwrap(),
            ),
            (
                "Screenshot_2023.09.27T191315.jpg",
                "2023-09-27 19:13:15".parse().unwrap(),
            ),
            (
                "Screenshot_2023-09-27T19:13:15Z.jpg",
                "2023-09-27 19:13:15".parse().unwrap(),
            ),
            (
                "Screenshot_20230927_191315.jpg",
                "2023-09-27 19:13:15".parse().unwrap(),
            ),
            (
                "00003IMG_00003_BURST20180930215746.jpg",
                "2018-09-30 21:57:46".parse().unwrap(),
            ),
            (
                "WhatsApp Image 2024-07-04 at 16.34.09.jpeg",
                "2024-07-04 16:34:09".parse().unwrap(),
            ),
            (
                "20051022_75_1_69dd.jpeg",
                "2005-10-22 00:00:00".parse().unwrap(),
            ),
            (
                "Screenshot - 12242013 - 06_56_20 PM.png",
                "2013-12-24 06:56:20".parse().unwrap(),
            ),
            (
                "IMG-20260309-WA0016.jpg",
                "2026-03-09 00:00:00".parse().unwrap(),
            ),
            ("12252012(001).jpg", "2012-12-25 00:00:00".parse().unwrap()),
            (
                "Screenshot_191315_20230927.jpg",
                "2023-09-27 19:13:15".parse().unwrap(),
            ),
            (
                "20250830_200704.jpg",
                "2025-08-30 20:07:04".parse().unwrap(),
            ),
        ];
        for (file, expected_moment) in files {
            let datetime =
                extract_datetime(file).unwrap_or_else(|| panic!("failed to parse {file}"));
            assert_eq!(datetime, expected_moment, "failed to parse {file}");
        }
    }

    #[test]
    fn inputs_that_must_fail() {
        let inputs = vec![
            "9e8449ee-9f27-440f-949b-69f98a200921.jpg",
            "2016-0101",
            "b71d8e68-1709-4d97-b896-592005e2716a.jpg",
        ];

        for input in inputs {
            assert!(
                extract_datetime(input).is_none(),
                "Invalid {input} was parsed successfully"
            );
        }
    }
}
