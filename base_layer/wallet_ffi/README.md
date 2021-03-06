# Tari Wallet FFI

Foreign Function interface for the Tari Android and Tari iOS Wallets.

This crate is part of the [Tari Cryptocurrency](https://tari.com) project.

# Build setup (Mac)

## Homebrew

Install Brew
```Shell Script
/usr/bin/ruby -e "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/master/install)"
```

Run the following to install the needed bottles
```Shell Script
brew install pkgconfig
brew install git
brew install make
brew install cmake
brew install autoconf
brew install automake
brew install libtool
```

## iOS Dependencies

Install [XCode](https://apps.apple.com/za/app/xcode/id497799835?mt=12) and then the XCode Command Line Tools with the following command
```Shell Script
xcode-select --install
```

For macOS Mojave additional headers need to be installed, run
```Shell Script
open /Library/Developer/CommandLineTools/Packages/macOS_SDK_headers_for_macOS_10.14.pkg
```
and follow the prompts

## Android Dependencies

Download the [Android NDK Bundle](https://developer.android.com/ndk/downloads)

## Enable Hidden Files

Run the following to show hidden files and folders
```Shell Script
defaults write com.apple.finder AppleShowAllFiles -bool YES
killall Finder
```
## The Code

Clone the following git repositories
1. [Tari](https://github.com/tari-project/tari.git)
2. [Wallet-Android](https://github.com/tari-project/wallet-android.git)
3. [Wallet-iOS](https://github.com/tari-project/wallet-ios.git)

Afterwards ```cd``` into the Tari repository and run the following
```Shell Script
git submodule init
git config submodule.recurse true
git submodule update --recursive --remote
```

## Rust
Install [Rust](https://www.rust-lang.org/tools/install)

Install the following tools and system images
```Shell Script
rustup toolchain add nightly-2019-10-04
rustup default nightly-2019-10-04
rustup component add rustfmt --toolchain nightly
rustup component add clippy
```

## Build Configuration

To configure the build, ```cd``` to the Tari repository and then 
```Shell Script
cd base_layer/wallet_ffi
open build.sample.config
```

Which will present you with the file contents as follows
```text
BUILD_ANDROID=1
BUILD_IOS=1
CARGO_CLEAN=1
SQLITE_SOURCE=https://www.sqlite.org/snapshot/sqlite-snapshot-201911192122.tar.gz
NDK_PATH=$HOME/android-ndk-r20
PKG_PATH=
ANDROID_WALLET_PATH=$HOME/wallet-android
IOS_WALLET_PATH=$HOME/wallet-ios
TARI_REPO_PATH=$HOME/tari-main
```
The following changes need to be made to the file
1. ```NDK_PATH``` needs to be changed to the directory of the Android NDK Bundle.
1. ```ANDROID_WALLET``` needs to be changed to the path of the Android-Wallet repository
1. ```IOS_WALLET_PATH``` needs to be changed to the path of the Wallet-iOS repository
1. ```CARGO_CLEAN``` if set to 1, the cargo clean command will be run before the build
1. ```TARI_REPO_PATH``` needs to be changed to the path of the Tari repository (Optional - defaults to current repo)
1. ```BUILD_ANDROID``` can be set to ```0``` to disable Android library build
1. ```BUILD_IOS``` can be set to ```0``` to disable iOS library build

Save the file and rename it to ```build.config```

## Building the Libraries

To build the libraries, ```cd``` to the Tari repository and then 
```Shell Script
cd base_layer/wallet_ffi
sh mobile_build.sh
```

The relevant libraries will then be built and placed in the appropriate directories of the Wallet-iOS and Wallet-Android repositories. 

# Setup (Windows)

## Test

1. Download SQL Lite (https://www.sqlite.org/index.html - 64bit) and unzip
2. `sqlite3.dll` must be accessible via the session path

## Build

ToDo -  Android only
