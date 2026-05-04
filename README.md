# Retrodate

Retrodate is an utility to retroactively date images on Immich.

Screenshots and images shared via Whatsapp or other media usually lack metadata;
they don't contain the datetime the images was taken. For "undated" images, Immich uses the upload
time to put the images in your timeline, which is wrong.

Th filename of screenshots and images shared on Whatsapp and social media often include a date and sometimes a time.
It's either the screenshot was taken or the moment the media was shared.

This tool extracts the date (and time, if included) from the filename and uses the Immich API to correct
a pictures exif data.

> [!NOTE]  
> At the moment, `retrodate` only supports "2021-02-13", "2021.02.13 17:13", "20210213171312", and some
variations on this format. Inputs starting with the day or month, e.g. 13-02-2021, are not yet supported.

## Usage

Obtain `retrodate` from the [releases](https://github.com/OrangeTux/retrodate/releases).

Then, obtain an [Immich API key](https://docs.immich.app/features/command-line-interface#obtain-the-api-key) with the permission "asset.read" and "asset.update".

```bash
$ export API_KEY="<your-key-here>"
$ export IMMICH_API="<url-to-the-immich-api" # For example, https://demo.immich.app/api
```

Then, run `retrodate` like this to list all changes `retrodate` likes to make:

```bash
./retrodate --api-key "${API_KEY}" --host "${IMMICH_API}" 
Date of IMG_20180301_131212.jpg will be set to 2018-03-01T13:12:12. Run script with --apply to apply the change.
Date of 20190720_130508.jpg will be set to 2019-07-20T13:05:08. Run script with --apply to apply the change.
Date of Screenshot_20201225-162340_Nike Run Club.jpg will be set to 2020-12-25T16:23:40. Run script with --apply to apply the change.
Date of Screenshot_20201202-183706_Spotify.jpg will be set to 2020-12-02T18:37:06. Run script with --apply to apply the change.
```

Run the same command with the flag `--apply` added to apply the changes:

```bash
./retrodate --api-key "${API_KEY}" --host "${IMMICH_API}" --apply
Date of IMG_20180301_131212.jpg set to 2018-03-01T13:12:12.
Date of 20190720_130508.jpg set to 2019-07-20T13:05:08.
Date of Screenshot_20201225-162340_Nike Run Club.jpg set to 2020-12-25T16:23:40.
Date of Screenshot_20201202-183706_Spotify.jpg set to 2020-12-02T18:37:06. 
```

Only touch assets that have 2017, 2018, or 2019 in their file name:

```bash
./retrodate --api-key "${API_KEY}" --host "${IMMICH_API}" --apply --from-year 2017 --until-year 2019
Date of IMG_20180301_131212.jpg set to 2018-03-01T13:12:12.
Date of 20190720_130508.jpg set to 2019-07-20T13:05:08.
```

## 
## License

This project is published under the [MIT license](LICENSE).
