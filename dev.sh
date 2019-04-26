#!/bin/bash

docker build --iidfile ./iidfile .

docker run --rm -it \
    --security-opt seccomp="$(pwd)/docker-localbind-seccomp-profile.json" \
    -v "$(pwd):/home/user/docker-localbind" \
    -w /home/user/docker-localbind \
    "$(cat ./iidfile)" \
    /bin/bash
