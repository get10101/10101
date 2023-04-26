#!/bin/bash

while true; do
  curl --data-binary '{"jsonrpc": "1.0", "id":"curltest", "method": "generatetoaddress", "params": [1, "bcrt1qaqhnv6fzqptnhwjr5vpxwcqs8q3mx9kqtxql54"] }' -H 'content-type: text/plain;' http://localhost:8080/bitcoin
  sleep 60
done
