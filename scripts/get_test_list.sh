#!/bin/bash

TEST_PREFIX=e2e
# Find all e2e tests

TEST_BIN=$(ls ./target/debug/deps/${TEST_PREFIX}* | grep -v '\.d\|\.o')

# Initialize an empty array
declare -a TEST_BIN_ARRAY

# Read the output of TEST_BIN into the array
while IFS= read -r line; do
    TEST_BIN_ARRAY+=("$line")
done <<< "$TEST_BIN"

# Convert the array to JSON using jq
json_array=$(printf '%s\n' "${TEST_BIN_ARRAY[@]}" | jq -R . | jq -s .)

# Remove newlines from json_array
json_array=$(echo "$json_array" | tr -d '\n')

# Print the JSON array
echo "$json_array"

