#!/bin/bash

opts=""
while [[ "$1" == '-'* ]]; do
    opts="$opts $1"
    shift
done

root_dir=$(cd $(dirname $0)/..; pwd)
exec docker run --rm \
     --network host --cap-add NET_ADMIN \
     -v $root_dir:/taskstats \
     -v $root_dir/docker-build/.cargo:/taskstats/.cargo \
     $opts \
     taskstats-build:latest "$@"
