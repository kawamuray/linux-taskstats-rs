#!/bin/bash
exec docker build $(dirname $0) -t taskstats-build:latest
