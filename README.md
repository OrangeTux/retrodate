# Retrodate

Retrodate is a utility to retroactively date images on Immich.

Screenshots and images shared via Whatsapp or other media usually lack metadata;
they don't contain the datetime the images was taken. For "undated" images, Immich uses the upload
time to put the images in your timeline, which is wrong.

Th filename of screenshots and images shared on Whatsapp and social media often include a date and sometimes a time.
It's either the screenshot was taken or the moment the media was shared.

This tool extracts the date (and time, if included) from the filename and uses the Immich API to correct
a pictures exif data.

## Examples

Obtain `retrodate` from the [releases](https://github.com/OrangeTux/retrodate/releases).

Then, obtain an [Immich API key](https://docs.immich.app/features/command-line-interface#obtain-the-api-key) with the permission "asset.read" and "asset.update".

```bash
$ export API_KEY="<your-key-here>"
$ export IMMICH_API="<url-to-the-immich-api" # For example, https://demo.immich.app/api
```

Then, run `retrodate` like this to list all changes `retrodate` likes to make:

```bash
./retrodate --api-key "${API_KEY}" --host "${IMMICH_API}" 
Date of Screenshot_20230927_191315.jpg will be set to 2023-09-27T19:13:15. Call `retrodate` with --if-unset to apply the change.
Date of Screenshot_20231001.jpg will be set to 2023-10-01T00:00:00. Call `retrodate` with --if-unset to apply the change.
Date of Screenshot_2023-11-02T041328.jpg will be changed from 1970-01-01T00:00:00 to 2023-11-02T04:13:28. Call `retrodate` with --overwrite to apply the change.
```

Run the same command with the flag `--if-unset` added to apply the changes:

```bash
./retrodate --api-key "${API_KEY}" --host "${IMMICH_API}" --if-unset
Date of Screenshot_20230927_191315.jpg set to 2023-09-27T19:13:15.
Date of Screenshot_20231001.jpg set to 2023-10-01T00:00:00.
Skipping of Screenshot_2023-11-02T041328.jpg, the asset has datetime set to 1970-01-01T00:00:00. Call `retrodate` with --overwrite to replace the existing datetime.
```

Alternatively, use `--overwrite` to modify assets that already have a creation date.

```bash
./retrodate --api-key "${API_KEY}" --host "${IMMICH_API}" --overwrite
Date of Screenshot_20230927_191315.jpg set to 2023-09-27T19:13:15.
Date of Screenshot_20231001.jpg set to 2023-10-01T00:00:00.
Change date of Screenshot_2023-11-02T041328.jpg from 1970-01-01T00:00:00 to 2023-11-02T04:13:28.
```

Only touch assets that have 2017, 2018, or 2019 in their file name:

```bash
./retrodate --api-key "${API_KEY}" --host "${IMMICH_API}" --if-unset --from-year 2017 --until-year 2019
Date of IMG_20180301_131212.jpg set to 2018-03-01T13:12:12.
Date of 20190720_130508.jpg set to 2019-07-20T13:05:08.
```

# Usage

```bash
$ ./retrodate --help
Usage: retrodate --api-key <api-key> --host <host> [--if-unset] [--overwrite] [--threshold <threshold>] [-v] [--from-year <from-year>] [--until-year <until-year>] [--timeout <timeout>]

Retrodate is a utility to retroactively date images on Immich based on an image's filename. This tool queries Immich for all filenames that contain a year, e.g. 2021 in 20210901_115950.jpg. Then, the date (and optional time) is parsed from the filename. You can control search using the arguments --from-year and --until-year.

Options:
  --api-key         API key providing access to Immich's API. It requires
                    permissions 'asset.read' and 'asset.update'.
  --host            URL of the immich API
  --if-unset        set the creation date of an asset, only if that asset
                    doesn't have a creation date already.
  --overwrite       set the creation date of an asset, even if that asset has a
                    creation date already. It sets --if-unset.
  --threshold       maximum allowed time difference when deciding to update an
                    asset's creation date. Provide a `jiff::Span` string such as
                    `30m`, `1h`, `2d`, or `1h30m`.
  -v, --verbose     explain what is being done
  --from-year       the earliest year to include in the search, defaults to 2000
  --until-year      the latest year to include in the search, defaults to the
                    current year
  --timeout         the maximum time in seconds for a single HTTP request
                    against Immich's HTTP API
  --help, help      display usage information
```

## License

This project is published under the [MIT license](LICENSE).
