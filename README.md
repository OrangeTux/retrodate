# Retrodate

Utility to retroactively date images on Immich.

## Description

Screenshots and images shared via Whatsapp or other media usually lack metadata;
they don't contain the datetime the images was taken. For "undated" images, Immich uses the upload
time to put the images in your timeline, which is wrong.

Th filename of screenshots and images shared on Whatsapp and social media often include a date and sometimes a time.
It's either the screenshot was taken or the moment the media was shared.

This tool extracts the date (and time, if included) from the filename and uses the Immich API to correct
a pictures exif data.

## License

This project is published under the [MIT license](LICENSE).
