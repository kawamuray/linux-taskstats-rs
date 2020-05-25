#!/bin/bash
set -e

root_dir=$(cd $(dirname $0)/..; pwd)
run="$root_dir/docker-build/docker-run.sh"

if [ ! -d $root_dir/docker-build/vendor ]; then
    echo "Running cargo vendor ..."
    $run cargo vendor docker-build/vendor
fi

echo "Building ..."
$run cargo build

echo "Running tests ..."
$run cargo test
