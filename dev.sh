#!/bin/bash

docker build --iidfile ./iidfile .

docker run --rm -it -v "$(pwd):/home/user/docker-localbind" --security-opt seccomp=unconfined -w /home/user/docker-localbind $(cat ./iidfile) /bin/bash


