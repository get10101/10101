#!/bin/bash

MIN_XCODE=15
# Pin CocoaPods version to avoid accidental changes in Podfile between PRs
COCOAPODS_VERSION="1.13.0"

set -euo pipefail

# Determine platform using uname
platform=$(uname)

# Run flutter doctor and capture the output
flutter_output=$(flutter doctor -v)


# Check if the platform is macOS, if so, check for Xcode and CocoaPods versions
if [ "$platform" == "Darwin" ]; then

    # Check and extract Xcode version
    if echo "$flutter_output" | grep -q "Xcode"; then
        echo "Xcode is installed!"

        # Extract Xcode version using awk and sed
        xcode_version=$(echo "$flutter_output" | grep "Xcode" | awk -F'(' '{print $2}' | awk -F')' '{print $1}')
        major_version=$(echo "$xcode_version" | awk -F'.' '{print $1}' | sed 's/[^0-9]*//g')

        if [ "$major_version" -ge $MIN_XCODE ]; then
            echo "Xcode version is $xcode_version which is >= $MIN_XCODE. All good!"
        else
            echo "Xcode version is $xcode_version which is < $MIN_XCODE. Please update!"
            exit 1
        fi
    else
        echo "Xcode is not installed! Please install at least Xcode $MIN_XCODE."
        exit 1
    fi

    # Check and extract CocoaPods version
    if echo "$flutter_output" | grep -q "CocoaPods"; then
        echo "CocoaPods is installed!"

        # Extract CocoaPods version using regex and awk
        current_cocoapods_version=$(echo "$flutter_output" | grep "CocoaPods version" | awk '{print $4}')

        if [ "$current_cocoapods_version" == "$COCOAPODS_VERSION" ]; then
            echo "CocoaPods version is $COCOAPODS_VERSION. All good!"
        else
            echo "CocoaPods version is $current_cocoapods_version which is not $COCOAPODS_VERSION. Please install the required version"
            exit 1
        fi
    else
        echo "CocoaPods is not installed!"
        exit 1
    fi
fi
