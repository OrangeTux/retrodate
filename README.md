# Retrodate

Retrodate is an utility to retroactively date images on Immich.

Screenshots and images shared via Whatsapp or other media usually lack metadata;
they don't contain the datetime the images was taken. For "undated" images, Immich uses the upload
time to put the images in your timeline, which is wrong.

Th filename of screenshots and images shared on Whatsapp and social media often include a date and sometimes a time.
It's either the screenshot was taken or the moment the media was shared.

This tool extracts the date (and time, if included) from the filename and uses the Immich API to correct
a pictures exif data.

## Usage

First, create an Immich API key with the permission "asset.read" and "asset.update".

Then, run `retrodate` like this:

```bash
./retrodate --api-key "<API_KEY>" --host "http://your-immiche-instance/api" 
Date of IMG_20180301_131212.jpg will be set to 2018-03-01T13:12:12. Run script with --apply to apply the change.
Date of 20190720_130508.jpg will be set to 2019-07-20T13:05:08. Run script with --apply to apply the change.
Date of Screenshot_20201225-162340_Nike Run Club.jpg will be set to 2020-12-25T16:23:40. Run script with --apply to apply the change.
Date of Screenshot_20201202-183706_Spotify.jpg will be set to 2020-12-02T18:37:06. Run script with --apply to apply the change.
```

Run the same command with the flag `--apply` added to apply the changes

```
./retrodate --api-key "<API_KEY>" --host "http://your-immiche-instance/api" --apply
Date of IMG_20180301_131212.jpg set to 2018-03-01T13:12:12.
Date of 20190720_130508.jpg set to 2019-07-20T13:05:08.
Date of Screenshot_20201225-162340_Nike Run Club.jpg set to 2020-12-25T16:23:40.
Date of Screenshot_20201202-183706_Spotify.jpg set to 2020-12-02T18:37:06. 
```

## License

This project is published under the [MIT license](LICENSE).
