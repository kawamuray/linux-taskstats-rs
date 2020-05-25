#!/bin/bash

root_dir=$(cd $(dirname $0)/..; pwd)
exec docker run --rm \
     --network host --cap-add NET_ADMIN \
     -v $root_dir:/taskstats \
     -v $root_dir/docker-build/.cargo:/taskstats/.cargo \
     taskstats-build:latest "$@"
