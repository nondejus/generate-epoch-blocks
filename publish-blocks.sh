#!/bin/bash

set -eu

if [ $# -lt 4 ]; then
    echo "Usage: publish-blocks.sh blocks-inner.json blocks-work.json blocks-signatures.json <RPC URL>" >&2
    exit 1
fi

jq -rcn --stream \
       --argfile work <(jq -Rsc 'split("\n")' "$2") \
       --argfile signature "$3" '
            def enumerate(i):
                foreach i as $item (
                    [-1, null];
                    [.[0] + 1, $item];
                    .
                );
            enumerate(fromstream(1 | truncate_stream(inputs)))
            | .[1] * { "work": $work[.[0]] } * { "signature": $signature[.[0]] }
            | tostring
            | { "action": "process", "block": . }
       ' < "$1" | \
while IFS='' read -r rpcCall; do
    rpcResult="$(curl -s --show-error "$4" -d "$rpcCall")"
    error="$(jq -er .error <(echo "$rpcResult"))"
    if [ "$error" = "Fork" ]; then
        echo "Encountered a fork for account $(echo "$blockInner" | jq -r .account)" >&2
    elif [ "$error" != "null" ] && [ "$error" != "Old block" ]; then
        echo "Encountered unexpected error '$error' from RPC call $rpcCall" >&2
        exit 2
    fi
done
