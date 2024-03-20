#!/bin/bash
# A script to fetch historic rates from Bitmex between a specifc time window

# BitMEX API base URL
API_URL="https://www.bitmex.com/api/v1"

SYMBOL="XBTUSD"

# Set start and end timestamps for the date range (March 20, 2023, to March 20, 2024)
START_TIMESTAMP="2023-03-20T00:00:00.000Z"
END_TIMESTAMP="2024-03-20T00:00:00.000Z"

# Construct the request URL for hourly data
REQUEST_URL="${API_URL}/trade/bucketed?symbol=${SYMBOL}&binSize=1h&partial=false&count=1000&reverse=false&startTime=${START_TIMESTAMP}&endTime=${END_TIMESTAMP}"

# Fetch the data from BitMEX API
RESPONSE=$(curl -s "${REQUEST_URL}")

# Check if the request was successful
if [ -z "${RESPONSE}" ]; then
    echo "Failed to fetch data from BitMEX API."
    exit 1
fi

# Parse JSON data and extract timestamp, symbol, and open price
JSON_DATA=$(echo "${RESPONSE}" | jq -r '[.[] | {timestamp: .timestamp, symbol: .symbol, open: .open}]')

# Store JSON data to a file
echo "${JSON_DATA}" > bitmex_hourly_rates.json

echo "Hourly rates stored in bitmex_hourly_rates.json"
