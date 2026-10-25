# Releasing the Android app on Google Play

Done by hand. Package `io.lucapf.aequalis` (permanent once created in Play Console).

## One time

### 1. The upload key

```
keytool -genkeypair -v -keystore ~/aequalis-upload.jks -alias upload \
  -keyalg RSA -keysize 2048 -validity 10000
```

Back up the `.jks` and its passwords outside the repository (`*.jks` and `keystore.properties` are in
`.gitignore`). Play App Signing keeps the real signing key; if the upload key is lost, Play support can
reset it.

### 2. `apps/android/keystore.properties`

```
storeFile=/home/user/aequalis-upload.jks
storePassword=...
keyAlias=upload
keyPassword=...
```

The same four values can be given as environment variables instead (for CI): `AEQUALIS_STOREFILE`,
`AEQUALIS_STOREPASSWORD`, `AEQUALIS_KEYALIAS`, `AEQUALIS_KEYPASSWORD`.

### 3. The app in Play Console

All apps › Create app: name, default language, app, free. Then fill in, under *Dashboard*:

- **Store listing**: short (80 characters) and full description, icon 512×512, feature graphic
  1024×500, at least 2 phone screenshots.
- **Privacy policy**: a URL (a page in the GitHub repository is enough). Required: the app has the
  INTERNET permission.
- **Data safety**: no personal data collected or shared. The app only downloads exchange rates and the
  city archive.
- **Content rating** questionnaire, **target audience** (not for children), **ads** (none).

### 4. A personal account made after November 2023

Google requires a **closed test** with at least 12 testers opted in for 14 days in a row before
*Apply for production*. Organization accounts are exempt. Start it as soon as the first build is up:
the 14 days cannot be shortened.

## Each release

### 1. Choose the version

`AEQUALIS_VERSION=major.minor.patch`. The version code is `major*10000 + minor*100 + patch`
(`0.1.0` → 100, `1.2.3` → 10203). Play remembers the highest code uploaded and never takes a lower
one, so the version only goes up.

### 2. Build the Rust libraries and the bundle

```
scripts/build-android.sh
cd apps/android
AEQUALIS_VERSION=0.1.0 ./gradlew bundleRelease
```

`build-android.sh` needs the NDK and the three Rust targets (see its first lines); it also ships the
current exchange rates and city archive, so build it the day of the release. `bundleRelease` stops with
an error when there is no upload key.

The bundle: `apps/android/app/build/outputs/bundle/release/app-release.aab`.

### 3. Check it

```
jarsigner -verify -verbose -certs apps/android/app/build/outputs/bundle/release/app-release.aab | grep CN= | head -3
```

The certificate must be yours, not *Android Debug*. Optionally, try the bundle on a phone with
[bundletool](https://developer.android.com/tools/bundletool) (`build-apks`, `install-apks`).

### 4. Upload

Play Console › the app › *Testing › Internal testing* (or *Closed testing*) › *Create new release* ›
upload the `.aab`, write the release notes › *Save* › *Review release* › *Start rollout*. At the first
upload, accept **Play App Signing**.

Read the *Pre-launch report* of the release: crashes and warnings found on Google's devices.

### 5. Production

*Production › Create new release*, **Add from library** (the build already tested), release notes,
*Rollout*. A first review takes a few days; later ones usually hours. A staged rollout (for instance
20%) lets you stop at a problem.

## If something goes wrong

- *"You uploaded a bundle signed with the wrong key"*: the bundle was signed with the debug key or
  another key. Check with `jarsigner` above.
- *"Version code N has already been used"*: raise `AEQUALIS_VERSION`.
- *The app stops at start with R8 on*: see the comment above `optimization` in
  `apps/android/app/build.gradle.kts`.
- The debug-key build for daily use, `scripts/install-android.sh`, needs no key, and installs over the
  debug build, not over the one from Play (different signature): uninstall one to install the other.
