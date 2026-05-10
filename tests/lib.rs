use std::{
    sync::atomic::Ordering,
    time::{Duration, Instant},
};

use env::Immich;
use jiff::Span;
use retrodate::{App, Asset, Client, ExifInfo, VERBOSE, get_asset_by_id};
use ureq::http::Uri;

mod env;

fn default_assets() -> Vec<Asset> {
    vec![
        // Asset contains a valid date and time.
        Asset {
            id: String::from("1"),
            original_file_name: String::from("Screenshot_20230927_191315.jpg"),
            exif_info: None,
        },
        // Asset does not contain a date and time.
        Asset {
            id: String::from("2"),
            original_file_name: String::from("lolcat.jpg"),
            exif_info: None,
        },
        // Asset contains a date but not a time.
        Asset {
            id: String::from("3"),
            original_file_name: String::from("Screenshot_20230927.jpg"),
            exif_info: None,
        },
        // Asset contains a date that is outside the range the App is looking for.
        // It only looks for assets with a year of 2000 or later in their filename.
        Asset {
            id: String::from("4"),
            original_file_name: String::from("Screenshot_19980927_200215.jpg"),
            exif_info: None,
        },
        // Asset contains a date but not a time.
        Asset {
            id: String::from("5"),
            original_file_name: String::from("Screenshot_2023-09-27T191315.jpg"),
            exif_info: None,
        },
        // Asset contains valid date and time. But it already a value for dateTimeOriginal.
        Asset {
            id: String::from("6"),
            original_file_name: String::from("Screenshot_2023-09-27T191315.jpg"),
            exif_info: Some(ExifInfo {
                date_time_original: Some(String::from("1970-01-01T00:00:00")),
            }),
        },
        // Asset contains valid date and time. But it already a value for dateTimeOriginal.
        Asset {
            id: String::from("7"),
            original_file_name: String::from("Screenshot_2023-09-27T191315.jpg"),
            exif_info: Some(ExifInfo {
                date_time_original: Some(String::from("2023-09-27T20:00:00")),
            }),
        },
    ]
}
/// Test the business logic against an Immich fake.
/// First, this test creates an App instance and verifies that the date of the asset is _not_ set.
/// Then, an App instance is created with "if_unset()" and the test verifies that the date of the asset _is_ set.
#[test]
fn test_app_created_with_if_unset() {
    let immich = Immich::with_assets(default_assets());
    let addr = immich.listening_address();
    let host: Uri = format!("http://{}/api", addr).parse().unwrap();

    VERBOSE.store(true, Ordering::Relaxed);
    let app = App::builder(host.clone(), String::from("api-key")).build();

    let _handle = immich.spawn();
    app.run().unwrap();

    let client = Client {
        host: host.clone(),
        api_key: String::from("api-key"),
        timeout: Duration::from_secs(1),
    };
    let asset = get_asset_by_id("1", &client).unwrap();
    assert_eq!(asset.exif_info, None);

    let app = App::builder(host, String::from("api-key"))
        .if_unset()
        .build();

    app.run().unwrap();
    let asset = get_asset_by_id("1", &client).unwrap();
    assert_eq!(
        asset.exif_info,
        Some(ExifInfo {
            date_time_original: Some(String::from("2023-09-27T19:13:15"))
        })
    );
    let asset = get_asset_by_id("2", &client).unwrap();
    assert!(asset.exif_info.is_none());
    let asset = get_asset_by_id("3", &client).unwrap();
    assert_eq!(
        asset.exif_info,
        Some(ExifInfo {
            date_time_original: Some(String::from("2023-09-27T00:00:00"))
        })
    );

    let asset = get_asset_by_id("4", &client).unwrap();
    assert!(asset.exif_info.is_none());
    let asset = get_asset_by_id("5", &client).unwrap();
    assert_eq!(
        asset.exif_info,
        Some(ExifInfo {
            date_time_original: Some(String::from("2023-09-27T19:13:15"))
        })
    );
    let asset = get_asset_by_id("6", &client).unwrap();
    assert_eq!(
        asset.exif_info,
        Some(ExifInfo {
            date_time_original: Some(String::from("1970-01-01T00:00:00"))
        })
    );
}

#[test]
fn test_app_created_with_overwrite() {
    let immich = Immich::with_assets(default_assets());
    let addr = immich.listening_address();
    let host: Uri = format!("http://{}/api", addr).parse().unwrap();

    VERBOSE.store(true, Ordering::Relaxed);
    let app = App::builder(host.clone(), String::from("api-key"))
        .overwrite(Span::new().days(1))
        .build();

    let _handle = immich.spawn();
    app.run().unwrap();

    let client = Client {
        host: host.clone(),
        api_key: String::from("api-key"),
        timeout: Duration::from_secs(1),
    };
    let asset = get_asset_by_id("1", &client).unwrap();
    assert_eq!(
        asset.exif_info,
        Some(ExifInfo {
            date_time_original: Some(String::from("2023-09-27T19:13:15"))
        })
    );
    let asset = get_asset_by_id("6", &client).unwrap();
    assert_eq!(
        asset.exif_info,
        Some(ExifInfo {
            date_time_original: Some(String::from("2023-09-27T19:13:15"))
        })
    );
    let asset = get_asset_by_id("7", &client).unwrap();
    assert_eq!(
        asset.exif_info,
        Some(ExifInfo {
            date_time_original: Some(String::from("2023-09-27T20:00:00"))
        })
    );
}
// Verify configuration the HTTP timeout works correctly.
#[test]
fn verify_http_timeout() {
    let immich = Immich::with_assets(vec![]);
    let addr = immich.listening_address();

    let host: Uri = format!("http://{}/api", addr).parse().unwrap();

    let app = App::builder(host.clone(), String::from("api-key"))
        .http_timeout(Duration::from_secs(2))
        .build();
    let then = Instant::now();
    assert!(app.run().is_err());
    let now = Instant::now();
    assert_eq!((now - then).as_secs(), 2);
}
