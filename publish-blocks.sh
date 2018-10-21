#!/bin/bash

set -eu

if [ $# -lt 4 ]; then
    echo "Usage: publish-blocks.sh blocks-inner.json blocks-work.json blocks-signatures.json <RPC URL>" >&2
    exit 1
fi

while IFS='' read -r blockInner <&11 && IFS='' read -r work <&12 && IFS='' read -r signature <&13; do
    rpcCall="$(jq -nc --argjson blockInner "$blockInner" --arg work "$work" --arg signature "$signature" \
        '$blockInner * { "work": $work } * { "signature": $signature }
            | tostring
            | { "action": "process", "block": . }')"
    rpcResult="$(curl -s --show-error "$4" -d "$rpcCall")"
    error="$(jq -er .error <(echo "$rpcResult"))"
    if [ "$error" = "Fork" ]; then
        echo "Encountered a fork for account $(echo "$blockInner" | jq -r .account)" >&2
    elif [ "$error" != "null" ] && [ "$error" != "Old block" ]; then
        echo "Encountered unexpected error '$error' from RPC call $rpcCall" >&2
        exit 2
    fi
    sleep 0.1
done 11<"$1" 12<"$2" 13<"$3"
