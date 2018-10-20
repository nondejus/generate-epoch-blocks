#!/bin/bash

set -eu

if [ $# -lt 3 ]; then
    echo "Usage: validate-work.sh blocks-inner.json blocks-work.json <RPC URL>" >&2
    exit 1
fi

i=0
while IFS='' read -r blockInner <&11 && IFS='' read -r work <&12; do
    rpcCall="$(jq -nc --argjson blockInner "$blockInner" --arg work "$work" \
        '$blockInner * { "work": $work, "signature": "0" }
            | tostring
            | { "action": "process", "block": . }')"
    rpcResult="$(curl -s --show-error "$3" -d "$rpcCall")"
    error="$(jq -er .error <(echo "$rpcResult"))"
    if [[ "$error" == *work* ]]; then
        echo "Error: work for block $i invalid" >&2
        exit 1
    fi
    i=$(($i + 1))
done 11<"$1" 12<"$2"
