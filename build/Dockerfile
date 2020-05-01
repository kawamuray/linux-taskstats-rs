FROM rust:buster
MAINTAINER Yuto Kawamura

RUN apt-get update && apt-get install -y curl llvm-dev libclang-dev gcc

WORKDIR /taskstats

CMD cargo test
