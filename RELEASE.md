# How to release

## Docker

Docker images are being released up on push to `main` at the moment.

As of now, there is no other way to release a different version!

## Testflight

To deploy to testflight you will need to have an apple developer account configured locally.

1. Build everything

First, we need to build all files to be able to bundle the release into an ipa file.

This command is not necessary to be run on every release.
It is only needed if you did not set up the project before.

```bash
just deps
```

```bash
just gen
just ios
```

If everything works, we go ahead and bundle an IPA (OS and iPadOS application archive)

```bash
just build-ipa
```

To publish to testflight you will need to have your Apple ID set up correctly.
We make use of environment variables which are parsed by our `just`-file automatically.
Make a copy of `.env.sample` and call it `.env` and fill out the required fields.

Once done, you can publish to testflight:

```bash
just publish-testflight
```

For a single command to execute all this in one go, you can use:

```bash
just release-testflight
```

Once uploaded, log into `appstoreconnect.apple.com/apps/` and approve testing the new version.

## Using fastlane

Make sure that all environment variables are set in the `.env` file.

You will also need to install [`fastlane`](https://fastlane.tools/).

1. We need to build the IPA file first without code signing because fastlane does not support `--dart-define`.
   Building the IPA file adds some overheads but ensures that these variables are set.

```bash
just build-ipa-no-codesign
```

2. Execute fastlane to build a signed IPA file and upload to Testflight

```bash
just publish-testflight-fastlane
```
