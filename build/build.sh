#!/bin/bash
set -e

root_dir=$(cd $(dirname $0)/..; pwd)

if [ ! -d $root_dir/build/vendor ]; then
    echo "Running cargo vendor ..."
    docker run --rm -v $root_dir:/taskstats -v $root_dir/build/.cargo:/taskstats/.cargo taskstats-build:latest cargo vendor build/vendor
fi

echo "Building ..."
docker run --rm -v $root_dir:/taskstats -v $root_dir/build/.cargo:/taskstats/.cargo taskstats-build:latest cargo build

echo "Running tests ..."
docker run --rm --network host --cap-add NET_ADMIN -v $root_dir:/taskstats -v $root_dir/build/.cargo:/taskstats/.cargo taskstats-build:latest cargo test
